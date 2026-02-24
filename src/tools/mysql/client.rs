use crate::config_loader::Environment;
use crate::config_loader::process_detector;
use crate::error::{CliError, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct MySQLTool;

/// Output mode for mysql CLI
#[derive(Copy, Clone)]
enum OutputMode {
    /// Normal formatted output (suitable for \G and table output)
    Standard,
    /// Raw, no headers, batch, no pretty formatting (-N -B -r -A)
    Raw,
}

struct TempMySqlDefaultsFile {
    path: PathBuf,
}

impl TempMySqlDefaultsFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn create(host: &str, port: u16, user: &str, password: &str) -> Result<Self> {
        let random: [u8; 16] = rand::random();
        let suffix = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let filename = format!("cloud-cli-mysql-{suffix}.cnf");
        let path = std::env::temp_dir().join(filename);

        let mut open_options = fs::OpenOptions::new();
        open_options.write(true).create_new(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open_options.mode(0o600);
        }

        let mut file = open_options.open(&path).map_err(|e| {
            CliError::ToolExecutionFailed(format!(
                "Failed to create mysql defaults file at {}: {e}",
                path.display()
            ))
        })?;

        let user = escape_mysql_option_value(user);
        let host = escape_mysql_option_value(host);
        let password = escape_mysql_option_value(password);

        writeln!(file, "[client]").map_err(CliError::IoError)?;
        writeln!(file, "user=\"{user}\"").map_err(CliError::IoError)?;
        writeln!(file, "host=\"{host}\"").map_err(CliError::IoError)?;
        writeln!(file, "port={port}").map_err(CliError::IoError)?;
        writeln!(file, "protocol=tcp").map_err(CliError::IoError)?;
        if !password.is_empty() {
            writeln!(file, "password=\"{password}\"").map_err(CliError::IoError)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)
                .map_err(CliError::IoError)?
                .permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms).map_err(CliError::IoError)?;
        }

        Ok(Self { path })
    }
}

impl Drop for TempMySqlDefaultsFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn escape_mysql_option_value(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

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

    /// Executes a MySQL query using credentials from the configuration (standard output mode).
    pub fn query_sql_with_config(
        config: &crate::config_loader::DorisConfig,
        query: &str,
    ) -> Result<String> {
        Self::execute_query_with_config(config, query, OutputMode::Standard)
    }

    /// Executes a MySQL query and returns raw output without headers or formatting (-N -B -r -A)
    pub fn query_sql_raw_with_config(
        config: &crate::config_loader::DorisConfig,
        query: &str,
    ) -> Result<String> {
        Self::execute_query_with_config(config, query, OutputMode::Raw)
    }

    /// Shared implementation for executing a query with selected output mode
    fn execute_query_with_config(
        config: &crate::config_loader::DorisConfig,
        query: &str,
        mode: OutputMode,
    ) -> Result<String> {
        let mysql_cfg = config.mysql.as_ref().ok_or_else(|| {
            CliError::ConfigError("MySQL credentials not found in config".to_string())
        })?;

        let cred_mgr = crate::tools::mysql::CredentialManager::new()?;
        let user = &mysql_cfg.user;
        let password = cred_mgr.decrypt_password(&mysql_cfg.password)?;
        let (host, port) = Self::get_connection_params()?;

        let output = Self::run_mysql_command(&host, port, user, &password, query, mode)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Access denied for user") || stderr.contains("ERROR 1045") {
                Err(CliError::MySQLAccessDenied(
                    "Access denied. Please update MySQL credentials.".into(),
                ))
            } else if stderr.contains("Unknown database") || stderr.contains("ERROR 1049") {
                Err(CliError::ToolExecutionFailed(
                    "Unknown database. Please verify the database name.".into(),
                ))
            } else if stderr.contains("Can't connect")
                || stderr.contains("Connection refused")
                || stderr.contains("ERROR 2003")
            {
                Err(CliError::ToolExecutionFailed(format!(
                    "Cannot connect to MySQL at {host}:{port}. Check host/port and service status."
                )))
            } else {
                Err(CliError::ToolExecutionFailed(
                    "MySQL query failed. Please try again.".into(),
                ))
            }
        } else {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
    }

    /// Runs a MySQL command with credentials in the specified output mode
    fn run_mysql_command(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        query: &str,
        mode: OutputMode,
    ) -> Result<std::process::Output> {
        let (mut command, _defaults_file) =
            Self::build_mysql_command(host, port, user, password, query, mode)?;

        command
            .output()
            .map_err(|e| CliError::ToolExecutionFailed(format!("Failed to execute mysql: {e}")))
    }

    fn build_mysql_command(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        query: &str,
        mode: OutputMode,
    ) -> Result<(Command, TempMySqlDefaultsFile)> {
        let defaults_file = TempMySqlDefaultsFile::create(host, port, user, password)?;

        let mut command = Command::new("mysql");
        // Must be the first argument to ensure MySQL reads only this file and does not
        // merge/override with system/user option files such as ~/.my.cnf.
        command.arg(format!(
            "--defaults-file={}",
            defaults_file.path().display()
        ));

        match mode {
            OutputMode::Standard => {
                command.arg("-A");
            }
            OutputMode::Raw => {
                command.args(["-N", "-B", "-r", "-A"]);
            }
        }

        command.arg("-e").arg(query);

        // Prevent mysql from prompting for a password interactively.
        command.stdin(std::process::Stdio::null());

        Ok((command, defaults_file))
    }

    /// Lists databases (excluding system databases) using raw mysql output
    pub fn list_databases(config: &crate::config_loader::DorisConfig) -> Result<Vec<String>> {
        let output = Self::query_sql_raw_with_config(config, "SHOW DATABASES;")?;
        let mut dbs: Vec<String> = output
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| !crate::tools::mysql::SYSTEM_DATABASES.contains(&s.as_str()))
            .collect();
        dbs.sort();
        Ok(dbs)
    }

    /// Lists tables for a given database using raw mysql output
    pub fn list_tables(
        config: &crate::config_loader::DorisConfig,
        database: &str,
    ) -> Result<Vec<String>> {
        let sql = format!("USE `{}`; SHOW TABLES;", database);
        let output = Self::query_sql_raw_with_config(config, &sql)?;
        let mut tables: Vec<String> = output
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        tables.sort();
        Ok(tables)
    }

    /// Gets the connection parameters for MySQL, with a clear priority:
    pub fn get_connection_params() -> Result<(String, u16)> {
        if let Some((host, port)) = std::env::var("MYSQL_HOST")
            .ok()
            .and_then(|h| std::env::var("MYSQL_PORT").ok().map(|p| (h, p)))
            .and_then(|(h, p_str)| p_str.parse::<u16>().ok().map(|p| (h, p)))
        {
            return Ok((host, port));
        }

        let config = crate::config_loader::load_config()?;
        if let Some(selected_fe_host) =
            crate::tools::common::host_selection::get_selected_host(false)
        {
            return Ok((selected_fe_host, config.query_port.unwrap_or(9030)));
        }

        if let Some(port) = config.query_port {
            return Ok(("127.0.0.1".to_string(), port));
        }

        // Fallback to default value.
        Ok(("127.0.0.1".to_string(), 9030))
    }
}
