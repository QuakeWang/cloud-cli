pub mod config;
pub mod config_loader;
pub mod core;
pub mod error;
pub mod executor;
pub mod process;
pub mod tools;
pub mod ui;

use config::Config;
use config_loader::persist_configuration;
use dialoguer::Confirm;
use error::Result;
use tools::Tool;
use tools::mysql::CredentialManager;
use ui::*;

/// Main CLI application runner
pub fn run_cli() -> Result<()> {
    let mut app_state = crate::core::AppState::new()?;

    if let Err(e) = app_state.config.validate() {
        ui::print_error(&format!("Config warning: {e}"));
    }

    let fe_process_exists =
        config_loader::process_detector::get_pid_by_env(config_loader::Environment::FE).is_ok();
    let has_mysql = app_state.doris_config.mysql.is_some();

    let cred_mgr = CredentialManager::new()?;
    if fe_process_exists
        && !has_mysql
        && Confirm::new()
            .with_prompt("MySQL credentials not detected. Configure now?")
            .default(true)
            .interact()?
    {
        match cred_mgr.prompt_credentials_with_connection_test() {
            Ok((user, password)) => {
                let mysql_config = cred_mgr.encrypt_credentials(&user, &password)?;
                app_state.doris_config.mysql = Some(mysql_config);
                persist_configuration(&app_state.doris_config);

                match tools::mysql::MySQLTool.query_cluster_info(&app_state.doris_config) {
                    Ok(cluster_info) => {
                        if let Err(e) = cluster_info.save_to_file() {
                            ui::print_warning(&format!("Failed to save cluster info: {e}"));
                        }
                    }
                    Err(e) => {
                        ui::print_warning(&format!("Failed to collect cluster info: {e}"));
                    }
                }
            }
            Err(e) => {
                ui::print_warning(&format!("MySQL credential setup failed: {e}"));
                ui::print_warning("You can configure it later in the settings.");
            }
        }
    }

    // Collect cluster info asynchronously in the background
    app_state.spawn_background_tasks_if_needed();

    let mut current_config = app_state.config.clone();

    loop {
        match show_main_menu()? {
            MainMenuAction::Fe => {
                if let Err(e) =
                    ui::handle_service_loop(&current_config, "FE", app_state.registry.fe_tools())
                {
                    print_error(&format!("FE service error: {e}"));
                    if !ask_continue("Would you like to return to the main menu?")? {
                        break;
                    }
                }
            }
            MainMenuAction::Be => {
                if let Err(e) =
                    ui::handle_service_loop(&current_config, "BE", app_state.registry.be_tools())
                {
                    print_error(&format!("BE service error: {e}"));
                    if !ask_continue("Would you like to return to the main menu?")? {
                        break;
                    }
                }
            }
            MainMenuAction::Exit => break,
        }

        app_state.reset_runtime_config();
        current_config = app_state.config.clone();
    }

    app_state.cleanup();

    ui::print_goodbye();
    Ok(())
}

fn execute_tool_enhanced(config: &Config, tool: &dyn Tool, service_name: &str) -> Result<()> {
    ui::tool_executor::execute_tool_enhanced(config, tool, service_name)
}
