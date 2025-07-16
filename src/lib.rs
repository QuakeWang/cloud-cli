pub mod config;
pub mod error;
pub mod executor;
pub mod process;
pub mod tools;
pub mod ui;

use config::Config;
use error::Result;
use process::ProcessManager;
use tools::{Tool, ToolRegistry};
use ui::*;

/// Main CLI application runner
pub fn run_cli(config: Config) -> Result<()> {
    let registry = ToolRegistry::new();
    loop {
        match show_main_menu()? {
            MainMenuAction::Fe => {
                if let Err(e) = handle_service_loop(&config, "FE", registry.fe_tools(), |pm| {
                    pm.detect_fe_processes()
                }) {
                    print_error(&format!("FE service error: {e}"));
                    if !ask_continue("Would you like to return to the main menu?")? {
                        break;
                    }
                }
            }
            MainMenuAction::Be => {
                if let Err(e) = handle_service_loop(&config, "BE", registry.be_tools(), |pm| {
                    pm.detect_be_processes()
                }) {
                    print_error(&format!("BE service error: {e}"));
                    if !ask_continue("Would you like to return to the main menu?")? {
                        break;
                    }
                }
            }
            MainMenuAction::Exit => break,
        }
    }

    ui::print_goodbye();
    Ok(())
}

/// Generic loop for handling a service type (FE or BE).
fn handle_service_loop(
    config: &Config,
    service_name: &str,
    tools: &[Box<dyn Tool>],
    process_detector: impl Fn(&ProcessManager) -> Result<Vec<process::Process>>,
) -> Result<()> {
    let process_manager = ProcessManager;
    loop {
        match show_tool_selection_menu(2, &format!("Select {service_name} tool"), tools)? {
            Some(tool) => {
                if let Err(e) = execute_tool(
                    config,
                    tool,
                    &process_manager,
                    &process_detector,
                    service_name,
                ) {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }

                // After a tool runs (or gracefully exits), show the post-execution menu
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

/// Generic tool execution function.
fn execute_tool(
    config: &Config,
    tool: &dyn Tool,
    process_manager: &ProcessManager,
    process_detector: &impl Fn(&ProcessManager) -> Result<Vec<process::Process>>,
    service_name: &str,
) -> Result<()> {
    let pid = if tool.requires_pid() {
        let processes = process_detector(process_manager)?;
        if processes.is_empty() {
            print_error(&format!("No {} processes found.", tool.name()));
            return Ok(());
        }
        let selected_process = process_manager.select_process(&processes, service_name)?;
        selected_process.pid
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
        Err(e) => handle_tool_execution_error(config, &e).map(|_| ()),
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
                print_error(&format!("Path does not exist: {}", new_path.display()));
                return Ok(None);
            }

            let jmap_path = new_path.join("bin/jmap");
            let jstack_path = new_path.join("bin/jstack");

            if !jmap_path.exists() || !jstack_path.exists() {
                print_error("Required JDK tools (jmap/jstack) not found in the specified path");
                return Ok(None);
            }

            let fixed_config = config.clone().with_jdk_path(new_path);
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
            print_success("Output directory updated successfully!");
            Ok(Some(fixed_config))
        }
        2 => Ok(None),
        _ => Err(error::CliError::InvalidInput(
            "Invalid selection".to_string(),
        )),
    }
}
