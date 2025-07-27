use crate::config_loader::Environment;
use crate::config_loader::process_detector;
use crate::error::{CliError, Result};
use std::process::Command;

pub struct MySQLTool;

impl MySQLTool {
    pub fn detect_fe_process() -> Result<u32> {
        process_detector::get_pid_by_env(Environment::FE)
    }

    pub fn test_connection(host: &str, port: u16, user: &str, password: &str) -> Result<bool> {
        let output = Command::new("mysql")
            .args([
                "-h",
                host,
                "-P",
                &port.to_string(),
                "-u",
                user,
                &format!("-p{password}"),
                "-A",
                "-e",
                "SELECT 1;",
            ])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    Ok(true)
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    if error_msg.contains("Access denied") {
                        Err(CliError::InvalidInput(format!(
                            "MySQL connection failed: Incorrect username or password.
Error: {}",
                            error_msg.trim()
                        )))
                    } else {
                        Err(CliError::ProcessExecutionFailed(format!(
                            "MySQL connection failed: {}
Error: {}",
                            output.status,
                            error_msg.trim()
                        )))
                    }
                }
            }
            Err(e) => Err(CliError::ProcessExecutionFailed(format!(
                "Failed to execute mysql command: {e}\nPlease ensure the mysql client is installed and in the environment."
            ))),
        }
    }

    pub fn get_connection_params() -> Result<(String, u16)> {
        if let (Ok(host), Ok(port_str)) = (std::env::var("MYSQL_HOST"), std::env::var("MYSQL_PORT"))
        {
            if let Ok(port) = port_str.parse::<u16>() {
                return Ok((host, port));
            }
        }

        let config = crate::config_loader::load_config()?;

        if let Some(port) = config.query_port {
            return Ok(("127.0.0.1".to_string(), port));
        }

        let host = "127.0.0.1".to_string();
        let port = 9030;

        Ok((host, port))
    }

    pub fn query_sql(_user: &str, _password: &str, _query: &str) -> Result<String> {
        Ok(String::new())
    }
}
