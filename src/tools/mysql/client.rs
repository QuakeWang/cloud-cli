use crate::config_loader::Environment;
use crate::config_loader::process_detector;
use crate::error::{CliError, Result};
use std::process::Command;

pub struct MySQLTool;

impl MySQLTool {
    pub fn detect_fe_process() -> Result<u32> {
        process_detector::get_pid_by_env(Environment::FE)
    }

    /// Queries the full cluster information, including frontends and backends.
    pub fn query_cluster_info(
        &self,
        config: &crate::config_loader::DorisConfig,
    ) -> Result<crate::tools::mysql::ClusterInfo> {
        let frontends_output =
            Self::query_sql_with_config(config, "SHOW FRONTENDS \\G").map_err(|e| {
                crate::error::CliError::ConfigError(format!("Failed to query frontends: {e}"))
            })?;
        let frontends = crate::tools::mysql::parse_frontends(&frontends_output);

        let backends_output =
            Self::query_sql_with_config(config, "SHOW BACKENDS \\G").map_err(|e| {
                crate::error::CliError::ConfigError(format!("Failed to query backends: {e}"))
            })?;
        let backends = crate::tools::mysql::parse_backends(&backends_output);

        Ok(crate::tools::mysql::ClusterInfo {
            frontends,
            backends,
        })
    }

    /// Executes a MySQL query using credentials from the configuration.
    pub fn query_sql_with_config(
        config: &crate::config_loader::DorisConfig,
        query: &str,
    ) -> Result<String> {
        let mysql_cfg = config.mysql.as_ref().ok_or_else(|| {
            CliError::ConfigError("MySQL credentials not found in config".to_string())
        })?;

        let cred_mgr = crate::tools::mysql::CredentialManager::new()?;
        let user = &mysql_cfg.user;
        let password = cred_mgr.decrypt_password(&mysql_cfg.password)?;
        let (host, port) = Self::get_connection_params()?;

        let output = Self::run_mysql_command_with_credentials(&host, port, user, &password, query)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Access denied for user") || stderr.contains("ERROR 1045") {
                return Err(CliError::MySQLAccessDenied(stderr.to_string()));
            } else {
                return Err(CliError::ToolExecutionFailed(format!(
                    "mysql query failed: {stderr}"
                )));
            }
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Runs a MySQL command with explicit credentials.
    fn run_mysql_command_with_credentials(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        query: &str,
    ) -> Result<std::process::Output> {
        let mut command = Command::new("mysql");
        command.arg("-h").arg(host);
        command.arg("-P").arg(port.to_string());
        command.arg("-u").arg(user);

        if !password.is_empty() {
            command.arg(format!("-p{password}"));
        }

        command.arg("-A");
        command.arg("-e").arg(query);

        // Prevent mysql from prompting for a password interactively
        command.stdin(std::process::Stdio::null());

        command
            .output()
            .map_err(|e| CliError::ToolExecutionFailed(format!("Failed to execute mysql: {e}")))
    }

    /// Gets the connection parameters for MySQL, with a clear priority:
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

        // Fallback to default value.
        Ok(("127.0.0.1".to_string(), 9030))
    }
}
