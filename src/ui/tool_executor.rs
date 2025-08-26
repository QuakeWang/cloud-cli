use crate::config::Config;
use crate::config_loader;
use crate::error::{self, Result};
use crate::process;
use crate::tools::Tool;
use crate::ui::{print_error, print_info, print_success};
use std::path::Path;

pub fn execute_tool_enhanced(config: &Config, tool: &dyn Tool, service_name: &str) -> Result<()> {
    let pid = match resolve_pid_if_required(tool) {
        Some(pid) => pid,
        None => return Ok(()),
    };

    print_info(&format!("Executing {}...", tool.name()));

    match tool.execute(config, pid) {
        Ok(result) => {
            print_success(&result.message);
            maybe_print_output_path(&result.output_path);
            Ok(())
        }
        Err(error::CliError::GracefulExit) => Ok(()),
        Err(e) => {
            match crate::ui::error_handlers::handle_tool_execution_error(
                config,
                &e,
                service_name,
                tool.name(),
            )? {
                Some(updated_config) => execute_tool_enhanced(&updated_config, tool, service_name),
                None => Ok(()),
            }
        }
    }
}

fn resolve_pid_if_required(tool: &dyn Tool) -> Option<u32> {
    if !tool.requires_pid() {
        return Some(0);
    }

    if let Some(pid) = config_loader::get_current_pid() {
        return Some(pid);
    }

    match process::select_process_interactively() {
        Ok(pid) => Some(pid),
        Err(_) => {
            let tool_name = tool.name();
            print_error(&format!("No {tool_name} processes found."));
            None
        }
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
