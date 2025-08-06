use crate::config::Config;
use crate::error::{CliError, Result};
use crate::executor;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use dialoguer::Input;
use std::env;
use std::process::Command;

pub struct FeProfilerTool;

impl FeProfilerTool {
    /// Prompt user for profile duration and return the duration value
    /// This method can be called before tool execution to get user input
    pub fn prompt_duration() -> Result<u32> {
        let input: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Enter collection duration in seconds")
            .with_initial_text("10")
            .interact_text()
            .map_err(|e| CliError::InvalidInput(format!("Duration input failed: {e}")))?;

        let duration_str = if input.trim().is_empty() {
            "10"
        } else {
            input.trim()
        };

        match duration_str.parse::<u32>() {
            Ok(val) if val > 0 && val <= 300 => Ok(val),
            Ok(_) => {
                ui::print_warning("Duration must be between 1 and 300 seconds!");
                ui::print_info("Hint: Enter a number between 1-300, e.g., 25");
                Err(CliError::GracefulExit)
            }
            Err(_) => {
                ui::print_warning("Please enter a valid number!");
                ui::print_info("Hint: Enter a number between 1-300, e.g., 25");
                Err(CliError::GracefulExit)
            }
        }
    }

    /// Execute the profiler with a specific duration
    pub fn execute_with_duration(&self, config: &Config, duration: u32) -> Result<ExecutionResult> {
        let doris_config = crate::config_loader::load_config()?;

        let fe_install_dir = doris_config
            .fe_install_dir
            .as_ref()
            .or(Some(&doris_config.install_dir))
            .ok_or_else(|| CliError::ConfigError("FE install directory not found".to_string()))?;

        let profile_script = fe_install_dir.join("bin").join("profile_fe.sh");

        if !profile_script.exists() {
            return Err(CliError::ConfigError(format!(
                "profile_fe.sh not found at {}. Please ensure Doris version is 2.1.4+",
                profile_script.display()
            )));
        }

        let mut command = Command::new("bash");
        command.arg(&profile_script);
        command.env("PROFILE_SECONDS", duration.to_string());

        executor::execute_command_with_timeout(&mut command, self.name(), config)?;

        let message = format!(
            "Flame graph generated successfully (duration: {duration}s)."
        );

        Ok(ExecutionResult {
            output_path: std::path::PathBuf::new(),
            message,
        })
    }
}

impl Tool for FeProfilerTool {
    fn name(&self) -> &str {
        "fe-profiler"
    }

    fn description(&self) -> &str {
        "Generate flame graph for FE performance analysis using async-profiler"
    }

    fn execute(&self, config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let profile_seconds = if env::var("PROFILE_SECONDS").is_ok() {
            env::var("PROFILE_SECONDS")
                .unwrap()
                .parse::<u32>()
                .unwrap_or(10)
        } else {
            Self::prompt_duration()?
        };

        self.execute_with_duration(config, profile_seconds)
    }

    fn requires_pid(&self) -> bool {
        false // FE profiler doesn't need PID as it uses profile_fe.sh script
    }
}
