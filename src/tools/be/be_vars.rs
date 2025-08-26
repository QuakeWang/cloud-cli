use super::BeResponseHandler;
use super::be_http_client;
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use dialoguer::{Input, theme::ColorfulTheme};

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

        let result = be_http_client::request_be_webserver_port("/varz", Some(&variable_name));

        let handler = BeResponseHandler {
            success_message: "Query completed!",
            empty_warning: "No variables found matching '{}'.",
            error_context: "Failed to query BE",
            tips: "Ensure the BE service is running and accessible.",
        };

        handler.handle_console_result(result, &variable_name)
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
        Ok("".to_string())
    } else {
        Ok(input)
    }
}
