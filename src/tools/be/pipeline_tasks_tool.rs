use crate::config::Config;
use crate::error::Result;
use crate::tools::common::be_webserver;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

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

        config.ensure_output_dir()?;

        match be_webserver::request_be_webserver_port("/api/running_pipeline_tasks", None) {
            Ok(result) => {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let filename = format!("pipeline_tasks_{}.json", timestamp);
                let output_path = config.output_dir.join(filename);

                fs::write(&output_path, &result)?;

                ui::print_success("Pipeline tasks fetched successfully");
                println!("\n{}", get_summary(&result));

                let message = format!("Pipeline tasks saved to {}", output_path.display());

                Ok(ExecutionResult {
                    output_path,
                    message,
                })
            }
            Err(e) => Err(e),
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
