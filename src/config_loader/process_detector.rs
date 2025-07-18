use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use crate::config_loader::Environment;
use crate::error::{CliError, Result};

/// Detect whether the current environment is FE or BE
pub fn detect_environment() -> Result<Environment> {
    // Check BE process first
    if check_be_process()? {
        return Ok(Environment::BE);
    }

    // Then check FE process
    if check_fe_process()? {
        return Ok(Environment::FE);
    }

    // If neither is detected, return Unknown
    Ok(Environment::Unknown)
}

/// Execute a shell command and return its output
pub fn execute_command(cmd: &str) -> Result<String> {
    execute_command_internal(cmd)
}

fn execute_command_internal(cmd: &str) -> Result<String> {
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

/// Check if BE process exists
fn check_be_process() -> Result<bool> {
    let output = execute_command("pgrep -f doris_be")?;
    Ok(!output.is_empty())
}

/// Check if FE process exists
fn check_fe_process() -> Result<bool> {
    let output = execute_command("pgrep -f \"(fe.*java|DorisFE)\"")?;
    Ok(!output.is_empty())
}

/// Get process ID of the first matching process
fn get_pid(pattern: &str) -> Result<String> {
    let output = execute_command(&format!("pgrep -f \"{pattern}\""))?;

    let pid = output
        .lines()
        .next()
        .ok_or_else(|| CliError::ConfigError(format!("No PID found for pattern: {pattern}")))?
        .trim()
        .to_string();

    Ok(pid)
}

/// Read environment variables from /proc/$pid/environ
fn read_proc_environ(pid: &str, grep_pattern: &str) -> Result<String> {
    let cmd = format!("cat /proc/{pid}/environ | tr '\\0' '\\n' | grep -E '{grep_pattern}'");
    execute_command(&cmd)
}

/// Extract environment variable value from a string like KEY=VALUE
pub fn extract_env_var(environ_output: &str, key: &str) -> Option<String> {
    extract_env_var_internal(environ_output, key)
}

fn extract_env_var_internal(environ_output: &str, key: &str) -> Option<String> {
    environ_output
        .lines()
        .find(|line| line.starts_with(&format!("{key}=")))
        .map(|line| line[key.len() + 1..].to_string())
}

/// Get BE paths including installation path and JDK path
pub fn get_be_paths() -> Result<(PathBuf, PathBuf)> {
    let pid = get_pid("doris_be")?;

    // Get environment variables related to BE
    let environ = read_proc_environ(&pid, "DORIS|BE|HOME|JAVA_HOME")?;

    // Extract DORIS_HOME and JAVA_HOME
    let doris_home = extract_env_var(&environ, "DORIS_HOME").ok_or_else(|| {
        CliError::ConfigError("DORIS_HOME not found in BE process environment".to_string())
    })?;

    let java_home =
        extract_env_var(&environ, "JAVA_HOME").unwrap_or_else(|| "/opt/jdk".to_string());

    let install_path = PathBuf::from(doris_home);
    let jdk_path = PathBuf::from(java_home);

    Ok((install_path, jdk_path))
}

/// Get BE config path
pub fn get_be_config_path() -> Result<PathBuf> {
    let (install_path, _) = get_be_paths()?;
    Ok(install_path.join("conf").join("be.conf"))
}

/// Get FE paths including installation path and JDK path
pub fn get_fe_paths() -> Result<(PathBuf, PathBuf)> {
    let pid = get_pid("(fe.*java|DorisFE)")?;

    // Get environment variables related to FE
    let environ = read_proc_environ(&pid, "DORIS|FE|HOME|JAVA_HOME")?;

    // Extract DORIS_HOME and JAVA_HOME
    let doris_home = extract_env_var(&environ, "DORIS_HOME").ok_or_else(|| {
        CliError::ConfigError("DORIS_HOME not found in FE process environment".to_string())
    })?;

    let java_home =
        extract_env_var(&environ, "JAVA_HOME").unwrap_or_else(|| "/opt/jdk".to_string());

    let install_path = PathBuf::from(doris_home);
    let jdk_path = PathBuf::from(java_home);

    Ok((install_path, jdk_path))
}

/// Get FE config path
pub fn get_fe_config_path() -> Result<PathBuf> {
    let (install_path, _) = get_fe_paths()?;
    Ok(install_path.join("conf").join("fe.conf"))
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
