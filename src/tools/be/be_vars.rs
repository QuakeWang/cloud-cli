use crate::config::Config;
use crate::error::{CliError, Result};
use crate::executor;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use dialoguer::{Input, theme::ColorfulTheme};
use std::path::PathBuf;
use std::process::Command;

const BE_DEFAULT_IP: &str = "127.0.0.1";
const BE_HTTP_PORTS: &[u16] = &[8040, 8041];

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

        let query_result = query_be_vars(&variable_name);
        handle_query_result(&variable_name, query_result);

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

/// Queries the BE's /varz endpoint for a given pattern.
fn query_be_vars(pattern: &str) -> Result<String> {
    for &port in BE_HTTP_PORTS {
        let url = format!("http://{BE_DEFAULT_IP}:{port}/varz");
        let mut curl_cmd = Command::new("curl");
        curl_cmd.args(["-sS", &url]);

        if let Ok(output) = executor::execute_command(&mut curl_cmd, "curl") {
            let varz_content = String::from_utf8_lossy(&output.stdout);
            let filtered_lines: Vec<&str> = varz_content
                .lines()
                .filter(|line| line.contains(pattern))
                .collect();
            return Ok(filtered_lines.join("\n"));
        }
    }

    Err(CliError::ToolExecutionFailed(
        "Could not connect to any BE http port (8040, 8041). Check if BE is running.".to_string(),
    ))
}
