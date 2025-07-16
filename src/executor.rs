use crate::config::Config;
use crate::error::{CliError, Result};
use std::process::{Command, Output};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Executes a command with standardized error handling
pub fn execute_command(command: &mut Command, tool_name: &str) -> Result<Output> {
    let output = command.output().map_err(|e| {
        CliError::ToolExecutionFailed(format!("Failed to execute {tool_name}: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = if !stderr.is_empty() {
            stderr.to_string()
        } else if !stdout.is_empty() {
            stdout.to_string()
        } else {
            format!(
                "Command failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            )
        };
        return Err(CliError::ToolExecutionFailed(format!(
            "{tool_name} failed: {error_msg}"
        )));
    }

    Ok(output)
}

/// Executes a command with timeout based on configuration
pub fn execute_command_with_timeout(
    command: &mut Command,
    tool_name: &str,
    config: &Config,
) -> Result<Output> {
    let mut child = command
        .spawn()
        .map_err(|e| CliError::ToolExecutionFailed(format!("Failed to start {tool_name}: {e}")))?;

    let timeout = Duration::from_millis(config.get_timeout_millis());

    match child.wait_timeout(timeout).map_err(|e| {
        CliError::ToolExecutionFailed(format!("Error waiting for {tool_name} process: {e}"))
    })? {
        // Process completed within timeout
        Some(status) => {
            if !status.success() {
                return Err(CliError::ToolExecutionFailed(format!(
                    "{tool_name} failed with exit code: {}",
                    status.code().unwrap_or(-1)
                )));
            }

            Ok(Output {
                status,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
        None => {
            // Kill the process
            let _ = child.kill();

            Err(CliError::ToolExecutionFailed(format!(
                "{tool_name} timed out after {} seconds",
                config.timeout_seconds
            )))
        }
    }
}
