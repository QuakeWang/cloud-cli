pub mod config;
pub mod config_loader;
pub mod error;
pub mod executor;
pub mod process;
pub mod tools;
pub mod ui;

use config::Config;
use config_loader::{load_config, persist_configuration};
use dialoguer::Confirm;
use error::Result;
use std::thread;
use tools::mysql::CredentialManager;
use tools::{Tool, ToolRegistry};
use ui::*;

/// Main CLI application runner
pub fn run_cli() -> Result<()> {
    let mut doris_config = load_config()?;

    let config = config_loader::to_app_config(doris_config.clone());
    if let Err(e) = config.validate() {
        ui::print_error(&format!("Config warning: {e}"));
    }

    let fe_process_exists =
        config_loader::process_detector::get_pid_by_env(config_loader::Environment::FE).is_ok();
    let has_mysql = doris_config.mysql.is_some();

    let cred_mgr = CredentialManager::new()?;
    if fe_process_exists
        && !has_mysql
        && Confirm::new()
            .with_prompt("MySQL credentials not detected. Configure now?")
            .default(true)
            .interact()?
    {
        let mut success = false;
        for _ in 0..3 {
            match cred_mgr.prompt_credentials_with_connection_test() {
                Ok((user, password)) => {
                    let mysql_config = cred_mgr.encrypt_credentials(&user, &password)?;
                    doris_config.mysql = Some(mysql_config);
                    persist_configuration(&doris_config);

                    match tools::mysql::MySQLTool.query_cluster_info(&doris_config) {
                        Ok(cluster_info) => {
                            if let Err(e) = cluster_info.save_to_file() {
                                ui::print_warning(&format!("Failed to save cluster info: {e}"));
                            }
                        }
                        Err(e) => {
                            ui::print_warning(&format!("Failed to collect cluster info: {e}"));
                        }
                    }

                    success = true;
                    break;
                }
                Err(e) => {
                    ui::print_warning(&format!("MySQL credential setup failed: {e}"));
                }
            }
        }
        if !success {
            ui::print_warning(
                "MySQL credential setup failed after 3 attempts. You can configure it later in the settings.",
            );
        }
    }

    // Collect cluster info asynchronously in the background
    let background_handle = if fe_process_exists && has_mysql {
        Some(spawn_cluster_info_collector(doris_config.clone()))
    } else {
        None
    };

    let registry = ToolRegistry::new();
    let mut current_config = config;

    loop {
        match show_main_menu()? {
            MainMenuAction::Fe => {
                if let Err(e) = handle_service_loop(&current_config, "FE", registry.fe_tools()) {
                    print_error(&format!("FE service error: {e}"));
                    if !ask_continue("Would you like to return to the main menu?")? {
                        break;
                    }
                }
            }
            MainMenuAction::Be => {
                if let Err(e) = handle_service_loop(&current_config, "BE", registry.be_tools()) {
                    print_error(&format!("BE service error: {e}"));
                    if !ask_continue("Would you like to return to the main menu?")? {
                        break;
                    }
                }
            }
            MainMenuAction::Exit => break,
        }

        current_config = Config::new();
    }

    // Wait for background task to complete
    if let Some(handle) = background_handle {
        let _ = handle.join();
    }

    ui::print_goodbye();
    Ok(())
}

/// Collect cluster info asynchronously in the background
fn spawn_cluster_info_collector(
    doris_config: crate::config_loader::DorisConfig,
) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        // Delay a short time to avoid blocking main program startup
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Check if cluster info needs to be updated
        if should_update_cluster_info() {
            collect_cluster_info_with_retry(&doris_config);
        }
    })
}

/// Collect cluster info with retry mechanism and timeout
fn collect_cluster_info_with_retry(doris_config: &crate::config_loader::DorisConfig) {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_SECS: u64 = 2;
    const TIMEOUT_SECS: u64 = 30;

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(TIMEOUT_SECS);
    let mut retry_count = 0;

    while retry_count < MAX_RETRIES && start.elapsed() < timeout {
        match collect_cluster_info_background(doris_config) {
            Ok(_) => {
                // Successfully collected, exit retry loop
                break;
            }
            Err(e) => {
                retry_count += 1;

                // Don't retry on authentication errors
                if let crate::error::CliError::MySQLAccessDenied(_) = e {
                    break;
                }

                // Don't retry on configuration errors
                if let crate::error::CliError::ConfigError(_) = e {
                    break;
                }

                if retry_count >= MAX_RETRIES || start.elapsed() >= timeout {
                    // Only log in debug mode and avoid excessive output
                    if std::env::var("CLOUD_CLI_DEBUG").is_ok() {
                        eprintln!(
                            "Background cluster info collection failed after {retry_count} attempts: {e}"
                        );
                    }
                    break;
                } else {
                    // Wait before retrying
                    std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS));
                }
            }
        }
    }
}

/// Check if cluster info needs to be updated
fn should_update_cluster_info() -> bool {
    let clusters_file = match dirs::home_dir() {
        Some(home) => home.join(".config").join("cloud-cli").join("clusters.toml"),
        None => return true, // Unable to determine path, default to update
    };

    if !clusters_file.exists() {
        return true;
    }

    let metadata = match std::fs::metadata(&clusters_file) {
        Ok(m) => m,
        Err(_) => return true, // Unable to get metadata, default to update
    };

    if metadata.len() < 100 {
        return true;
    }

    let modified = match metadata.modified() {
        Ok(m) => m,
        Err(_) => return true, // Unable to get modification time, default to update
    };

    let duration = match std::time::SystemTime::now().duration_since(modified) {
        Ok(d) => d,
        Err(_) => return true, // Time error, default to update
    };

    duration.as_secs() > 300 // 5 minutes
}

/// Implementation for collecting cluster info in the background
fn collect_cluster_info_background(doris_config: &crate::config_loader::DorisConfig) -> Result<()> {
    if doris_config.mysql.is_none() {
        return Ok(());
    }
    let mysql_tool = tools::mysql::MySQLTool;
    let cluster_info = mysql_tool.query_cluster_info(doris_config)?;
    cluster_info.save_to_file()?;
    Ok(())
}

/// Generic loop for handling a service type (FE or BE).
fn handle_service_loop(config: &Config, service_name: &str, tools: &[Box<dyn Tool>]) -> Result<()> {
    loop {
        match show_tool_selection_menu(2, &format!("Select {service_name} tool"), tools)? {
            Some(tool) => {
                if let Err(e) = execute_tool_enhanced(config, tool, service_name) {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }

                match show_post_execution_menu(tool.name())? {
                    PostExecutionAction::Continue => continue,
                    PostExecutionAction::BackToMain => return Ok(()),
                    PostExecutionAction::Exit => {
                        ui::print_goodbye();
                        std::process::exit(0);
                    }
                }
            }
            None => return Ok(()), // "Back" was selected
        }
    }
}

/// Enhanced tool execution function that uses the new configuration system
fn execute_tool_enhanced(config: &Config, tool: &dyn Tool, _service_name: &str) -> Result<()> {
    let pid = if tool.requires_pid() {
        // Try to get PID from configuration first
        match config_loader::get_current_pid() {
            Some(pid) => pid,
            None => {
                // Fallback: try to detect and select process interactively
                match process::select_process_interactively() {
                    Ok(pid) => pid,
                    Err(_) => {
                        let tool_name = tool.name();
                        print_error(&format!("No {tool_name} processes found."));
                        return Ok(());
                    }
                }
            }
        }
    } else {
        0 // PID is not required, provide a dummy value
    };

    print_info(&format!("Executing {}...", tool.name()));

    match tool.execute(config, pid) {
        Ok(result) => {
            print_success(&result.message);
            if result.output_path.to_str() != Some("console_output") {
                print_info(&format!(
                    "Output saved to: {}",
                    result.output_path.display()
                ));
            }
            Ok(())
        }
        Err(error::CliError::GracefulExit) => Ok(()), // Simply return to the menu
        Err(e) => {
            // Handle the error and get the potentially updated config
            match handle_tool_execution_error(config, &e)? {
                Some(updated_config) => {
                    // Try executing the tool again with the updated config
                    execute_tool_enhanced(&updated_config, tool, _service_name)
                }
                None => Ok(()),
            }
        }
    }
}

fn handle_tool_execution_error(config: &Config, error: &error::CliError) -> Result<Option<Config>> {
    println!();
    print_warning("Tool execution failed due to configuration issues.");
    print_error(&format!("Error: {error}"));

    println!();
    print_info("Would you like to:");

    let options = vec![
        "Fix JDK path and retry".to_string(),
        "Fix output directory and retry".to_string(),
        "Cancel and return to menu".to_string(),
    ];

    let selection = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Choose an option")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| error::CliError::InvalidInput(format!("Error fix selection failed: {e}")))?;

    match selection {
        0 => {
            // Fix JDK path
            let new_path: String =
                dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Enter the correct JDK path")
                    .with_initial_text(config.jdk_path.to_string_lossy().to_string())
                    .interact_text()
                    .map_err(|e| {
                        error::CliError::InvalidInput(format!("JDK path input failed: {e}"))
                    })?;

            let new_path = std::path::PathBuf::from(new_path);

            // Validate the new path
            if !new_path.exists() {
                let path_display = new_path.display();
                print_error(&format!("Path does not exist: {path_display}"));
                return Ok(None);
            }

            let jmap_path = new_path.join("bin/jmap");
            let jstack_path = new_path.join("bin/jstack");

            if !jmap_path.exists() || !jstack_path.exists() {
                print_error("Required JDK tools (jmap/jstack) not found in the specified path");
                return Ok(None);
            }

            let fixed_config = config.clone().with_jdk_path(new_path);

            // Persist the updated configuration
            if let Err(e) = persist_updated_config(&fixed_config) {
                print_warning(&format!("Failed to persist configuration: {e}"));
            }

            print_success("JDK path updated successfully!");
            Ok(Some(fixed_config))
        }
        1 => {
            // Fix output directory
            let new_path: String =
                dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Enter the output directory path")
                    .with_initial_text(config.output_dir.to_string_lossy().to_string())
                    .interact_text()
                    .map_err(|e| {
                        error::CliError::InvalidInput(format!("Output dir input failed: {e}"))
                    })?;

            let new_path = std::path::PathBuf::from(new_path);

            // Test creating the directory
            if let Err(e) = std::fs::create_dir_all(&new_path) {
                print_error(&format!("Cannot create directory: {e}"));
                return Ok(None);
            }

            let fixed_config = config.clone().with_output_dir(new_path);

            // Persist the updated configuration
            if let Err(e) = persist_updated_config(&fixed_config) {
                print_warning(&format!("Failed to persist configuration: {e}"));
            }

            print_success("Output directory updated successfully!");
            Ok(Some(fixed_config))
        }
        2 => Ok(None),
        _ => Err(error::CliError::InvalidInput(
            "Invalid selection".to_string(),
        )),
    }
}

/// Persist updated configuration to disk
fn persist_updated_config(config: &Config) -> Result<()> {
    let mut doris_config = config_loader::load_config()?;
    doris_config = doris_config.with_app_config(config);
    match config_loader::config_persister::persist_config(&doris_config) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
