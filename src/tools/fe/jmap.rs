use crate::config::Config;
use crate::error::{CliError, Result};
use crate::executor;
use crate::tools::{ExecutionResult, Tool};
use chrono::Utc;
use std::process::Command;

pub struct JmapDumpTool;
pub struct JmapHistoTool;

impl Tool for JmapDumpTool {
    fn name(&self) -> &str {
        "jmap-dump"
    }

    fn description(&self) -> &str {
        "Generate heap dump (.hprof)"
    }

    fn execute(&self, config: &Config, pid: u32) -> Result<ExecutionResult> {
        config.ensure_output_dir()?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("jmap_dump_{pid}_{timestamp}.hprof");
        let output_path = config.output_dir.join(filename);

        let jmap_path = config.get_jmap_path();
        let dump_arg = format!("live,file={}", output_path.display());

        let mut command = Command::new(&jmap_path);
        command.args([format!("-dump:{dump_arg}"), pid.to_string()]);

        executor::execute_command_with_timeout(&mut command, self.name(), config)?;

        Ok(ExecutionResult {
            output_path,
            message: format!(
                "Heap dump completed successfully (timeout: {}s)",
                config.timeout_seconds
            ),
        })
    }
}

impl Tool for JmapHistoTool {
    fn name(&self) -> &str {
        "jmap-histo"
    }

    fn description(&self) -> &str {
        "Generate histogram (.log)"
    }

    fn execute(&self, config: &Config, pid: u32) -> Result<ExecutionResult> {
        config.ensure_output_dir()?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("jmap_histo_{pid}_{timestamp}.log");
        let output_path = config.output_dir.join(filename);

        let jmap_path = config.get_jmap_path();

        let mut command = Command::new(&jmap_path);
        command.args(["-histo:live", &pid.to_string()]);

        // Use regular execution for histogram as it's typically fast
        let output = executor::execute_command(&mut command, self.name())?;

        std::fs::write(&output_path, &output.stdout).map_err(CliError::IoError)?;

        Ok(ExecutionResult {
            output_path,
            message: "Histogram completed successfully".to_string(),
        })
    }
}
