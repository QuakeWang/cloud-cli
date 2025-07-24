use super::BeResponseHandler;
use super::be_http_client;
use crate::config::Config;
use crate::error::Result;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;

/// Tool to fetch running pipeline tasks from BE node
pub struct PipelineTasksTool;

impl Tool for PipelineTasksTool {
    fn name(&self) -> &str {
        "pipeline-tasks"
    }

    fn description(&self) -> &str {
        "Get running pipeline tasks from BE node"
    }

    fn execute(&self, config: &Config, _pid: u32) -> Result<ExecutionResult> {
        ui::print_info("Fetching running pipeline tasks from BE...");

        let result = be_http_client::request_be_webserver_port("/api/running_pipeline_tasks", None);

        let handler = BeResponseHandler {
            success_message: "Pipeline tasks fetched successfully!",
            empty_warning: "No running pipeline tasks found.",
            error_context: "Failed to fetch pipeline tasks",
            tips: "Ensure the BE service is running and accessible.",
        };

        // First check if we have a result
        match &result {
            Ok(output) => {
                if output.len() < 100 || output.lines().count() <= 3 {
                    return handler.handle_console_result(result, "pipeline tasks");
                }

                // Otherwise save to file
                config.ensure_output_dir()?;
                handler.handle_file_result(config, result, "pipeline_tasks", get_summary)
            }
            Err(_) => {
                // For errors, just use the standard error handling
                handler.handle_console_result(result, "pipeline tasks")
            }
        }
    }

    fn requires_pid(&self) -> bool {
        false
    }
}

/// Get a summary of the response data for display in the console
fn get_summary(data: &str) -> String {
    if data.trim().is_empty() {
        return "No running pipeline tasks found.".to_string();
    }

    // Simple summary: show first few lines
    let preview_lines: Vec<&str> = data.lines().take(10).collect();
    let preview = preview_lines.join("\n");

    if data.lines().count() > 10 {
        format!("{}\n... (more content in output file)", preview)
    } else {
        preview
    }
}
