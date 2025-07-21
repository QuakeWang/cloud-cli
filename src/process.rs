use crate::config_loader;
use crate::error::{CliError, Result};
use crate::ui;
use dialoguer::{Select, theme::ColorfulTheme};
use std::process::Command;

/// Represents a Java process that can be targeted by diagnostic tools
#[derive(Debug, Clone)]
pub struct Process {
    pub pid: u32,
    pub command: String,
}

/// Manager for detecting and selecting Java processes
pub struct ProcessManager;

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    /// Creates a new ProcessManager instance
    pub fn new() -> Self {
        Self
    }

    /// Fallback detection when config PID is not available
    fn detect_processes_fallback(
        &self,
        env_type: config_loader::Environment,
    ) -> Result<Vec<Process>> {
        use crate::config_loader::process_detector;

        if env_type == config_loader::Environment::Unknown {
            return Ok(vec![]);
        }

        process_detector::detect_current_process()
            .map(|result| {
                if result.environment == env_type {
                    vec![Process {
                        pid: result.pid,
                        command: result.command,
                    }]
                } else {
                    vec![]
                }
            })
            .or_else(|_| Ok(vec![]))
    }

    /// Detect processes for specific environment - uses config PID if available, fallback to detection
    pub fn detect_processes(&self, env_type: config_loader::Environment) -> Result<Vec<Process>> {
        if let Ok(config) = config_loader::get_current_config() {
            if config.environment == env_type {
                if let (Some(pid), Some(command)) = (config.get_valid_pid(), config.process_command)
                {
                    return Ok(vec![Process { pid, command }]);
                }
            }
        }
        self.detect_processes_fallback(env_type)
    }

    /// Detect FE processes
    pub fn detect_fe_processes(&self) -> Result<Vec<Process>> {
        self.detect_processes(config_loader::Environment::FE)
    }

    /// Detect BE processes
    pub fn detect_be_processes(&self) -> Result<Vec<Process>> {
        self.detect_processes(config_loader::Environment::BE)
    }

    pub fn select_process<'a>(
        &self,
        processes: &'a [Process],
        service_name: &str,
    ) -> Result<&'a Process> {
        if processes.is_empty() {
            return Err(CliError::ProcessNotFound(format!(
                "No {service_name} processes found"
            )));
        }

        if processes.len() == 1 {
            ui::print_info(format!("Found 1 {service_name} process").as_str());
            ui::print_process_info(processes[0].pid, &processes[0].command);
            return Ok(&processes[0]);
        }

        ui::print_info(&format!(
            "Found {} {service_name} processes",
            processes.len()
        ));

        let items: Vec<String> = processes
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{} PID: {} - {}",
                    console::style(format!("[{}]", i + 1)).dim(),
                    console::style(p.pid.to_string()).green().bold(),
                    console::style(crate::ui::truncate_command(&p.command, 60)).dim()
                )
            })
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Select {service_name} process"))
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| CliError::InvalidInput(format!("Process selection failed: {e}")))?;

        let selected = &processes[selection];
        ui::print_process_info(selected.pid, &selected.command);
        Ok(selected)
    }
}

/// Process utility functions
pub struct ProcessTool;

impl ProcessTool {
    /// Check if a process is still running
    pub fn is_process_alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get process information by PID
    pub fn get_process_info(pid: u32) -> Result<String> {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid,command"])
            .output()
            .map_err(|e| {
                CliError::ProcessExecutionFailed(format!("Failed to get process info: {e}"))
            })?;

        if !output.status.success() {
            return Err(CliError::ProcessExecutionFailed(format!(
                "Failed to get info for PID {}",
                pid
            )));
        }

        String::from_utf8(output.stdout).map_err(|e| {
            CliError::ProcessExecutionFailed(format!("Failed to parse process info: {e}"))
        })
    }
}

/// Select process interactively when config PID is not available
pub fn select_process_interactively() -> Result<u32> {
    // This is only called when config file PID is invalid
    // Use the enhanced process detector for discovery
    match config_loader::process_detector::detect_current_process() {
        Ok(result) => {
            crate::ui::print_info(&format!(
                "Found {} process",
                if result.environment == config_loader::Environment::FE {
                    "FE"
                } else {
                    "BE"
                }
            ));
            crate::ui::print_process_info(result.pid, &result.command);
            Ok(result.pid)
        }
        Err(_) => Err(CliError::ProcessNotFound(
            "No Doris process found".to_string(),
        )),
    }
}
