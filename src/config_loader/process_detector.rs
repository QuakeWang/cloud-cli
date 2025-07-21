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

    if let Ok(be_process) = detect_be_process_detailed() {
        processes.push(be_process);
    }

    if let Ok(fe_process) = detect_fe_process_detailed() {
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
    let (doris_home, java_home) = get_paths_by_pid_with_fallback(env, pid, &command);

    Ok(ProcessDetectionResult {
        pid,
        command,
        environment: env,
        doris_home,
        java_home,
    })
}

/// Detect BE process with detailed information
fn detect_be_process_detailed() -> Result<ProcessDetectionResult> {
    detect_process_detailed(Environment::BE)
}

/// Detect FE process with detailed information
fn detect_fe_process_detailed() -> Result<ProcessDetectionResult> {
    detect_process_detailed(Environment::FE)
}

/// Get process command line by PID with improved error handling
pub fn get_process_command(pid: u32) -> Result<String> {
    // Try different approaches to get the command line

    // Approach 1: Standard ps command
    let output = match Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
    {
        Ok(o) => o,
        Err(_) => {
            // Try alternative method
            return get_process_command_alternative(pid);
        }
    };

    if !output.status.success() {
        // Try alternative method
        return get_process_command_alternative(pid);
    }

    match String::from_utf8(output.stdout) {
        Ok(s) => Ok(s.trim().to_string()),
        Err(_) => {
            // Try alternative method
            get_process_command_alternative(pid)
        }
    }
}

/// Alternative method to get process command when standard method fails
fn get_process_command_alternative(pid: u32) -> Result<String> {
    // Try using /proc/PID/cmdline on Linux
    let proc_cmdline = Path::new("/proc").join(pid.to_string()).join("cmdline");
    if proc_cmdline.exists() {
        if let Ok(content) = std::fs::read_to_string(&proc_cmdline) {
            let command = content.replace('\0', " ").trim().to_string();
            return Ok(command);
        }
    }

    // If all else fails, try a different ps format
    let cmd = format!("ps -p {pid} -o args=");
    match execute_command(&cmd) {
        Ok(output) => Ok(output),
        Err(_) => {
            // Last resort: Just return a placeholder with the PID
            let fallback = format!("unknown_process_{pid}");
            Ok(fallback)
        }
    }
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
            let commands = [
                "jps | grep DorisFE",                                   // Java process scanner
                "ps -ef | grep DorisFE | grep -v grep",                 // Process list
                "ps -ef | grep 'doris.*fe' | grep java | grep -v grep", // Java FE processes
            ];

            for cmd in &commands {
                if let Ok(output) = execute_command(cmd) {
                    if output.trim().is_empty() {
                        continue;
                    }

                    let regex_pattern = if cmd.starts_with("jps") {
                        r"^(\d+)\s+DorisFE"
                    } else {
                        r"^\S+\s+(\d+)"
                    };

                    if let Ok(pid) = extract_pid_from_output(&output, regex_pattern, true) {
                        return Ok(pid);
                    }
                }
            }

            Err(CliError::ProcessNotFound(
                "No FE processes found".to_string(),
            ))
        }
        _ => Err(CliError::ProcessNotFound("Invalid environment".to_string())),
    }
}

/// Get BE process ID with reliable matching (legacy compatibility)
pub fn get_be_pid() -> Result<u32> {
    get_pid_by_env(Environment::BE)
}

/// Get FE process ID with reliable matching (legacy compatibility)
pub fn get_fe_pid() -> Result<u32> {
    get_pid_by_env(Environment::FE)
}

/// Read environment variables from /proc/$pid/environ
fn read_proc_environ(pid: &str, grep_pattern: &str) -> Result<String> {
    let cmd = format!("cat /proc/{pid}/environ | tr '\\0' '\\n' | grep -E '{grep_pattern}'");
    execute_command(&cmd)
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

/// Extract environment variable value from a string like KEY=VALUE
pub fn extract_env_var(environ_output: &str, key: &str) -> Option<String> {
    regex_utils::extract_env_var(environ_output, key)
}

/// Get paths including installation path and JDK path for the specified environment
pub fn get_paths(env: Environment) -> Result<(PathBuf, PathBuf)> {
    let pid = get_pid_by_env(env)?;

    // Get environment variables related to the process
    let environ = read_proc_environ(&pid.to_string(), "DORIS|BE|FE|HOME|JAVA_HOME")?;

    // Extract DORIS_HOME and JAVA_HOME
    let doris_home = extract_env_var(&environ, "DORIS_HOME").ok_or_else(|| {
        CliError::ConfigError(format!("DORIS_HOME not found in {env} process environment"))
    })?;

    let java_home =
        extract_env_var(&environ, "JAVA_HOME").unwrap_or_else(|| "/opt/jdk".to_string());

    let install_path = PathBuf::from(doris_home);
    let jdk_path = PathBuf::from(java_home);

    Ok((install_path, jdk_path))
}

/// Get paths by PID with fallback options for the specified environment
fn get_paths_by_pid_with_fallback(env: Environment, pid: u32, command: &str) -> (PathBuf, PathBuf) {
    let grep_pattern = "DORIS|BE|FE|HOME|JAVA_HOME";
    if let Ok(environ) = read_proc_environ_by_pid(pid, grep_pattern) {
        if let Some(doris_home) = extract_env_var(&environ, "DORIS_HOME") {
            let java_home =
                extract_env_var(&environ, "JAVA_HOME").unwrap_or_else(|| "/opt/jdk".to_string());
            return (PathBuf::from(doris_home), PathBuf::from(java_home));
        }
    }

    match env {
        Environment::BE => {
            if let Some(path) = regex_utils::extract_path_from_command(command, "doris_be") {
                return (path, PathBuf::from("/opt/jdk"));
            }

            if let Some(doris_home) = extract_doris_home_from_path(command) {
                return (doris_home, PathBuf::from("/opt/jdk"));
            }
        }
        Environment::FE => {
            if let Some(doris_home) = extract_fe_doris_home_from_command(command) {
                return (doris_home, PathBuf::from("/opt/jdk"));
            }
        }
        _ => {}
    }

    (PathBuf::from("/opt/selectdb"), PathBuf::from("/opt/jdk"))
}

/// Check if a process with given PID is still running
pub fn is_process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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

/// Extract DORIS_HOME from any absolute path in command (simplified logic)
fn extract_doris_home_from_path(command: &str) -> Option<PathBuf> {
    command
        .split_whitespace()
        .filter(|word| word.starts_with('/') && word.contains("doris_be"))
        .find_map(|word| {
            PathBuf::from(word)
                .parent()?
                .parent()?
                .parent()
                .map(|doris_home| doris_home.to_path_buf())
        })
}

/// Extract FE DORIS_HOME from command parameters (simplified)
fn extract_fe_doris_home_from_command(command: &str) -> Option<PathBuf> {
    // Check for explicit DORIS_HOME parameter
    let re1 = regex::Regex::new(r"-DDORIS_HOME=([^ ]+)").ok()?;
    if let Some(caps) = re1.captures(command) {
        if let Some(doris_home) = caps.get(1) {
            return Some(PathBuf::from(doris_home.as_str()));
        }
    }

    // Check for log4j configuration file path
    let re2 =
        regex::Regex::new(r"-Dlog4j\.configurationFile=file:([^/]+(?:/[^/]+)*?)(?:/conf/)").ok()?;
    if let Some(caps) = re2.captures(command) {
        if let Some(doris_home) = caps.get(1) {
            return Some(PathBuf::from(doris_home.as_str()));
        }
    }

    None
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

/// Get config path for the specified environment
pub fn get_config_path(env: Environment) -> Result<PathBuf> {
    let (install_path, _) = get_paths(env)?;

    let config_file = match env {
        Environment::BE => "be.conf",
        Environment::FE => "fe.conf",
        _ => return Err(CliError::ConfigError("Invalid environment".to_string())),
    };

    Ok(install_path.join("conf").join(config_file))
}

/// Get BE config path
pub fn get_be_config_path() -> Result<PathBuf> {
    get_config_path(Environment::BE)
}

/// Get FE config path
pub fn get_fe_config_path() -> Result<PathBuf> {
    get_config_path(Environment::FE)
}
