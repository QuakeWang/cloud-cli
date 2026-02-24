use crate::config::Config;
use crate::config_loader;
use crate::error::{self, Result};
use crate::process;
use crate::tools::Tool;
use crate::ui::{print_error, print_info, print_success};
use std::path::Path;

pub struct ToolRunStatus {
    pub updated_config: Option<Config>,
    pub completed: bool,
}

impl ToolRunStatus {
    fn completed(updated_config: Option<Config>) -> Self {
        Self {
            updated_config,
            completed: true,
        }
    }

    fn skipped(updated_config: Option<Config>) -> Self {
        Self {
            updated_config,
            completed: false,
        }
    }
}

fn normalize_error_handler_result(result: Result<Option<Config>>) -> Result<Option<Config>> {
    match result {
        // `GracefulExit` from error handler means "stop current tool flow",
        // not "bubble up to parent menu".
        Err(error::CliError::GracefulExit) => Ok(None),
        other => other,
    }
}

pub fn execute_tool_enhanced(
    config: &Config,
    tool: &dyn Tool,
    service_name: &str,
) -> Result<ToolRunStatus> {
    let mut current_config = config.clone();
    let mut latest_updated_config: Option<Config> = None;

    loop {
        let pid = match resolve_pid_if_required(tool, service_name) {
            Some(pid) => pid,
            None => return Ok(ToolRunStatus::skipped(latest_updated_config)),
        };

        print_info(&format!("Executing {}...", tool.name()));

        match tool.execute(&current_config, pid) {
            Ok(result) => {
                print_success(&result.message);
                maybe_print_output_path(&result.output_path);
                return Ok(ToolRunStatus::completed(latest_updated_config));
            }
            Err(error::CliError::GracefulExit) => {
                return Ok(ToolRunStatus::skipped(latest_updated_config));
            }
            Err(e) => {
                let recovery = normalize_error_handler_result(
                    crate::ui::error_handlers::handle_tool_execution_error(
                        &current_config,
                        &e,
                        service_name,
                        tool.name(),
                    ),
                )?;

                match recovery {
                    Some(updated_config) => {
                        latest_updated_config = Some(updated_config.clone());
                        current_config = updated_config;
                    }
                    None => return Ok(ToolRunStatus::skipped(latest_updated_config)),
                }
            }
        }
    }
}

fn resolve_pid_if_required(tool: &dyn Tool, service_name: &str) -> Option<u32> {
    if !tool.requires_pid() {
        return Some(0);
    }

    if let Some(pid) = config_loader::get_pid_by_service(service_name) {
        return Some(pid);
    }

    match service_name {
        "FE" | "BE" => {
            print_error(&format!("No {service_name} process found."));
            None
        }
        _ => match process::select_process_interactively() {
            Ok(pid) => Some(pid),
            Err(_) => {
                let tool_name = tool.name();
                print_error(&format!("No {tool_name} processes found."));
                None
            }
        },
    }
}

fn maybe_print_output_path(output_path: &Path) {
    if output_path
        .to_str()
        .filter(|p| !p.is_empty() && *p != "console_output")
        .is_some()
    {
        print_info(&format!("Output saved to: {}", output_path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CliError;

    #[test]
    fn normalize_error_handler_result_cases() {
        let graceful =
            normalize_error_handler_result(Err(CliError::GracefulExit)).expect("must succeed");
        assert!(
            graceful.is_none(),
            "graceful exit should be normalized to None"
        );

        let invalid = normalize_error_handler_result(Err(CliError::InvalidInput("bad".into())));
        assert!(
            matches!(invalid, Err(CliError::InvalidInput(msg)) if msg == "bad"),
            "non-graceful errors should be preserved"
        );

        let cfg = Config::default();
        let success = normalize_error_handler_result(Ok(Some(cfg))).expect("must succeed");
        assert!(
            success.is_some(),
            "successful recovery value should be preserved"
        );
    }
}
