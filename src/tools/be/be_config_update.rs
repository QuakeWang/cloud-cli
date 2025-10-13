use super::be_http_client;
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::ExecutionResult;
use crate::tools::Tool;
use crate::ui;
use dialoguer::{Confirm, Input, theme::ColorfulTheme};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct ConfigUpdateResult {
    config_name: String,
    status: String,
    msg: String,
}

pub struct BeUpdateConfigTool;

impl Tool for BeUpdateConfigTool {
    fn name(&self) -> &str {
        "set-be-config"
    }

    fn description(&self) -> &str {
        "Update BE configuration variables"
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let key = prompt_input("Enter BE config key to update")?;
        let value = prompt_input(&format!("Enter value for '{key}'"))?;
        let persist = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Persist this configuration?")
            .default(false)
            .interact()
            .map_err(|e| CliError::InvalidInput(format!("Input failed: {e}")))?;

        ui::print_info(&format!(
            "Updating BE config: {key}={value} (persist: {persist})"
        ));

        let endpoint = format!("/api/update_config?{key}={value}&persist={persist}");
        handle_update_result(be_http_client::post_be_endpoint(&endpoint), &key)
    }

    fn requires_pid(&self) -> bool {
        false
    }
}

fn prompt_input(prompt: &str) -> Result<String> {
    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact_text()
        .map_err(|e| CliError::InvalidInput(format!("Input failed: {e}")))?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        ui::print_warning("Input cannot be empty!");
        Err(CliError::GracefulExit)
    } else {
        Ok(trimmed.to_string())
    }
}

fn get_current_value(key: &str) -> Option<String> {
    be_http_client::request_be_webserver_port("/varz", Some(key))
        .ok()?
        .lines()
        .next()?
        .split('=')
        .nth(1)
        .map(|v| v.trim().to_string())
}

fn handle_update_result(result: Result<String>, key: &str) -> Result<ExecutionResult> {
    let json_response = result.map_err(|e| {
        ui::print_error(&format!("Failed to update BE config: {e}."));
        ui::print_info("Tips: Ensure the BE service is running and accessible.");
        e
    })?;

    let results: Vec<ConfigUpdateResult> = serde_json::from_str(&json_response)
        .map_err(|e| CliError::ToolExecutionFailed(format!("Failed to parse response: {e}")))?;

    println!();
    ui::print_info("Results:");

    let all_ok = results.iter().all(|item| {
        if item.status == "OK" {
            match get_current_value(&item.config_name) {
                Some(value) => println!("  ✓ {} = {}", item.config_name, value),
                None => println!("  ✓ {}: OK", item.config_name),
            }
            true
        } else {
            println!("  ✗ {}: FAILED - {}", item.config_name, item.msg);
            false
        }
    });

    if all_ok {
        Ok(ExecutionResult {
            output_path: PathBuf::from("console_output"),
            message: format!("Config '{key}' updated successfully"),
        })
    } else {
        Err(CliError::ToolExecutionFailed(
            "Some configurations failed to update".to_string(),
        ))
    }
}
