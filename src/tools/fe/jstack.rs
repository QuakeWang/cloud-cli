use crate::config::Config;
use crate::error::{CliError, Result};
use crate::executor;
use crate::tools::{ExecutionResult, Tool};
use chrono::Utc;
use std::process::Command;

pub struct JstackTool;

impl Tool for JstackTool {
    fn name(&self) -> &str {
        "jstack"
    }

    fn description(&self) -> &str {
        "Generate thread stack trace (.log)"
    }

    fn execute(&self, config: &Config, pid: u32) -> Result<ExecutionResult> {
        config.ensure_output_dir()?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("jstack_{pid}_{timestamp}.log");
        let output_path = config.output_dir.join(filename);

        let jstack_path = config.get_jstack_path();

        let mut command = Command::new(&jstack_path);
        command.args([&pid.to_string()]);

        let output = executor::execute_command(&mut command, self.name())?;

        std::fs::write(&output_path, &output.stdout).map_err(CliError::IoError)?;

        Ok(ExecutionResult {
            output_path,
            message: "Thread stack trace completed successfully".to_string(),
        })
    }
}
