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

    fn detect_processes_by_pattern<F>(&self, match_fn: F) -> Result<Vec<Process>>
    where
        F: Fn(&str) -> bool,
    {
        let output = Command::new("ps")
            .args(["-ef"])
            .output()
            .map_err(|e| CliError::ProcessExecutionFailed(format!("Failed to execute ps: {e}")))?;

        if !output.status.success() {
            return Err(CliError::ProcessExecutionFailed(format!(
                "ps command failed with exit code: {}",
                output.status
            )));
        }

        let stdout = String::from_utf8(output.stdout).map_err(|e| {
            CliError::ProcessExecutionFailed(format!("Failed to parse ps output: {e}"))
        })?;

        let mut processes = Vec::new();

        for line in stdout.lines() {
            if match_fn(line) && !line.contains("grep") {
                if let Some(process) = self.parse_process_line(line) {
                    processes.push(process);
                }
            }
        }

        Ok(processes)
    }

    pub fn detect_fe_processes(&self) -> Result<Vec<Process>> {
        self.detect_processes_by_pattern(|line| {
            line.contains("fe") && line.contains("org.apache.doris.DorisFE")
        })
    }

    pub fn detect_be_processes(&self) -> Result<Vec<Process>> {
        self.detect_processes_by_pattern(|line| line.contains("doris_be"))
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

    /// Parses a line from `ps` output to extract process information
    fn parse_process_line(&self, line: &str) -> Option<Process> {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 2 {
            return None;
        }

        let pid_str = parts[1];
        let pid = pid_str.parse::<u32>().ok()?;

        Some(Process {
            pid,
            command: line.to_string(),
        })
    }
}
