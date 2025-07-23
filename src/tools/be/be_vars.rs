use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::common::be_webserver;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use dialoguer::{Input, theme::ColorfulTheme};
use std::path::PathBuf;

/// Tool to query BE configuration variables
pub struct BeVarsTool;

impl Tool for BeVarsTool {
    fn name(&self) -> &str {
        "get-be-vars"
    }

    fn description(&self) -> &str {
        "Query BE configuration variables"
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let variable_name = prompt_for_variable_name()?;
        if variable_name.is_empty() {
            return Err(CliError::GracefulExit);
        }

        ui::print_info(&format!(
            "Querying BE for variables matching: '{variable_name}'"
        ));

        let result = be_webserver::request_be_webserver_port("/varz", Some(&variable_name));
        handle_query_result(&variable_name, result);

        Ok(ExecutionResult {
            output_path: PathBuf::from("console_output"),
            message: format!("Variable query completed for: {variable_name}"),
        })
    }

    fn requires_pid(&self) -> bool {
        false
    }
}

fn prompt_for_variable_name() -> Result<String> {
    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter BE variable name to query (or part of it)")
        .interact_text()
        .map_err(|e| CliError::InvalidInput(format!("Variable name input failed: {e}")))?;

    if input.trim().is_empty() {
        ui::print_warning("Variable name cannot be empty!");
        ui::print_info("Hint: e.g., tablet_map_shard_size, or just 'shard' to search.");
        Ok("".to_string())
    } else {
        Ok(input)
    }
}

fn handle_query_result(variable_name: &str, result: Result<String>) {
    match result {
        Ok(output) => {
            ui::print_success("Query completed!");
            println!();
            ui::print_info("Results:");
            if output.is_empty() {
                ui::print_warning(&format!("No variables found matching '{variable_name}'."));
            } else {
                println!("{output}");
            }
        }
        Err(e) => {
            ui::print_error(&format!("Failed to query BE: {e}."));
            ui::print_info("Tips: Ensure the BE service is running and accessible.");
        }
    }
}
