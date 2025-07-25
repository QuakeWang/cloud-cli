use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use crate::config_loader::Environment;
use crate::config_loader::regex_utils;
use crate::error::{CliError, Result};

/// Process detection result with detailed information
#[derive(Debug, Clone)]
pub struct ProcessDetectionResult {
    pub pid: u32,
    pub command: String,
    pub environment: Environment,
    pub doris_home: PathBuf,
    pub java_home: PathBuf,
}

/// Detect all running Doris processes with detailed information
pub fn detect_all_processes() -> Result<Vec<ProcessDetectionResult>> {
    let mut processes = Vec::new();

    if let Ok(be_process) = detect_process_detailed(Environment::BE) {
        processes.push(be_process);
    }

    if let Ok(fe_process) = detect_process_detailed(Environment::FE) {
        processes.push(fe_process);
    }

    if processes.is_empty() {
        Err(CliError::ProcessNotFound(
            "No Doris processes found".to_string(),
        ))
    } else {
        Ok(processes)
    }
}

/// Detect current running process with detailed information (legacy compatibility)
pub fn detect_current_process() -> Result<ProcessDetectionResult> {
    let all_processes = detect_all_processes()?;
    // Return the first process found for backward compatibility
    all_processes
        .into_iter()
        .next()
        .ok_or_else(|| CliError::ProcessNotFound("No Doris process found".to_string()))
}

/// Detect whether the current environment is FE or BE
pub fn detect_environment() -> Result<Environment> {
    match detect_current_process() {
        Ok(result) => Ok(result.environment),
        Err(_) => Ok(Environment::Unknown),
    }
}

/// Execute a shell command and return its output
pub fn execute_command(cmd: &str) -> Result<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| CliError::ConfigError(format!("Failed to execute command: {e}")))?;

    let result = str::from_utf8(&output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|e| CliError::ConfigError(format!("Failed to parse command output: {e}")))?;

    Ok(result)
}

/// Detect process with detailed information based on environment
fn detect_process_detailed(env: Environment) -> Result<ProcessDetectionResult> {
    let pid = get_pid_by_env(env)?;
    let command = get_process_command(pid)?;
    let (doris_home, java_home) = get_paths_by_pid(pid);

    Ok(ProcessDetectionResult {
        pid,
        command,
        environment: env,
        doris_home,
        java_home,
    })
}

/// Get process command line by PID with improved error handling
pub fn get_process_command(pid: u32) -> Result<String> {
    // Try /proc/PID/cmdline on Linux (most direct and reliable when available)
    let proc_cmdline = Path::new("/proc").join(pid.to_string()).join("cmdline");
    if proc_cmdline.exists() {
        if let Ok(content) = std::fs::read_to_string(&proc_cmdline) {
            let command = content.replace('\0', " ").trim().to_string();
            if !command.is_empty() {
                return Ok(command);
            }
        }
    }

    // Try ps command with different output formats
    let ps_formats = ["command=", "args="];
    for format in &ps_formats {
        if let Ok(output) = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", format])
            .output()
        {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let cmd = s.trim().to_string();
                    if !cmd.is_empty() {
                        return Ok(cmd);
                    }
                }
            }
        }
    }

    // Last resort: Return a placeholder with the PID
    Ok(format!("unknown_process_{pid}"))
}

fn extract_pid_from_output(output: &str, regex_pattern: &str, first_only: bool) -> Result<u32> {
    regex_utils::extract_pid_from_output(output, regex_pattern, first_only)
        .ok_or_else(|| CliError::ProcessNotFound("Invalid process info format".to_string()))
}

/// Get process ID based on environment type
pub fn get_pid_by_env(env: Environment) -> Result<u32> {
    match env {
        Environment::BE => {
            let cmd = "ps -ef | grep doris_be | grep -v grep";
            let output = execute_command(cmd)
                .map_err(|_| CliError::ProcessNotFound("No BE processes found".to_string()))?;

            if output.trim().is_empty() {
                return Err(CliError::ProcessNotFound(
                    "No BE processes found".to_string(),
                ));
            }

            extract_pid_from_output(&output, r"^\S+\s+(\d+)", false)
        }
        Environment::FE => {
            let cmd = "ps -ef | grep DorisFE | grep -v grep";
            let output = execute_command(cmd)
                .map_err(|_| CliError::ProcessNotFound("No FE processes found".to_string()))?;

            if output.trim().is_empty() {
                return Err(CliError::ProcessNotFound(
                    "No FE processes found".to_string(),
                ));
            }

            extract_pid_from_output(&output, r"^\S+\s+(\d+)", false)
        }
        _ => Err(CliError::ProcessNotFound("Invalid environment".to_string())),
    }
}

/// Read environment variables by PID for Linux systems
fn read_proc_environ_by_pid(pid: u32, grep_pattern: &str) -> Result<String> {
    // Check if /proc exists (Linux systems)
    let proc_path = Path::new("/proc").join(pid.to_string()).join("environ");

    if proc_path.exists() {
        // Linux system
        let cmd = format!("cat /proc/{pid}/environ | tr '\\0' '\\n' | grep -E '{grep_pattern}'");
        execute_command(&cmd)
    } else {
        // If /proc doesn't exist or we can't access it
        Err(CliError::ConfigError(format!(
            "Cannot access process environment for PID {pid} - /proc filesystem not available"
        )))
    }
}

/// Get paths including installation path and JDK path for the specified environment
pub fn get_paths(env: Environment) -> Result<(PathBuf, PathBuf)> {
    let pid = get_pid_by_env(env)?;

    // Use the simplified function to get paths
    let (install_path, jdk_path) = get_paths_by_pid(pid);

    // Verify that we have a valid DORIS_HOME path
    if install_path == PathBuf::from("/opt/selectdb") {
        return Err(CliError::ConfigError(format!(
            "DORIS_HOME not found in {env} process environment"
        )));
    }

    Ok((install_path, jdk_path))
}

/// Get paths by PID for the specified environment
fn get_paths_by_pid(pid: u32) -> (PathBuf, PathBuf) {
    let grep_pattern = "DORIS_HOME|JAVA_HOME";
    if let Ok(environ) = read_proc_environ_by_pid(pid, grep_pattern) {
        if let Some(doris_home) = regex_utils::extract_env_var(&environ, "DORIS_HOME") {
            let java_home = regex_utils::extract_env_var(&environ, "JAVA_HOME")
                .unwrap_or_else(|| "/opt/jdk".to_string());
            return (PathBuf::from(doris_home), PathBuf::from(java_home));
        }
    }

    // Default paths if environment variables are not available
    (PathBuf::from("/opt/selectdb"), PathBuf::from("/opt/jdk"))
}

/// Verify that a config file exists
pub fn verify_config_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(CliError::ConfigError(format!(
            "Config file does not exist: {}",
            path.display()
        )));
    }
    Ok(())
}

pub fn detect_mixed_deployment(config: &mut crate::config_loader::DorisConfig) -> Result<bool> {
    let mut is_mixed = false;

    let all_processes = detect_all_processes()?;

    let fe_processes: Vec<_> = all_processes
        .iter()
        .filter(|p| p.environment == crate::config_loader::Environment::FE)
        .collect();

    let be_processes: Vec<_> = all_processes
        .iter()
        .filter(|p| p.environment == crate::config_loader::Environment::BE)
        .collect();

    if !fe_processes.is_empty() && !be_processes.is_empty() {
        is_mixed = true;

        if let Some(fe_process) = fe_processes.first() {
            config.fe_process_pid = Some(fe_process.pid);
            config.fe_process_command = Some(fe_process.command.clone());
            config.fe_install_dir = Some(fe_process.doris_home.clone());

            if config.environment != crate::config_loader::Environment::FE {
                if let Ok(fe_config) = crate::config_loader::config_parser::parse_config_from_path(
                    crate::config_loader::Environment::FE,
                    &fe_process.doris_home,
                ) {
                    config.http_port = fe_config.http_port;
                    config.rpc_port = fe_config.rpc_port;
                    config.query_port = fe_config.query_port;
                    config.edit_log_port = fe_config.edit_log_port;
                    config.cloud_http_port = fe_config.cloud_http_port;
                    config.meta_dir = fe_config.meta_dir;
                }
            }
        }

        if let Some(be_process) = be_processes.first() {
            config.be_process_pid = Some(be_process.pid);
            config.be_process_command = Some(be_process.command.clone());
            config.be_install_dir = Some(be_process.doris_home.clone());

            if config.environment != crate::config_loader::Environment::BE {
                if let Ok(be_config) = crate::config_loader::config_parser::parse_config_from_path(
                    crate::config_loader::Environment::BE,
                    &be_process.doris_home,
                ) {
                    config.be_port = be_config.be_port;
                    config.brpc_port = be_config.brpc_port;
                    config.webserver_port = be_config.webserver_port;
                    config.heartbeat_service_port = be_config.heartbeat_service_port;
                }
            }
        }
    }

    Ok(is_mixed)
}
