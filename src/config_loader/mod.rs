use crate::error::Result;
use std::path::PathBuf;

pub mod config_parser;
pub mod config_persister;
pub mod process_detector;
pub mod regex_utils;

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Environment {
    FE,
    BE,
    Mixed,
    Unknown,
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::FE => write!(f, "FE"),
            Environment::BE => write!(f, "BE"),
            Environment::Mixed => write!(f, "FE + BE"),
            Environment::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Doris configuration model with all system settings
#[derive(Debug, Clone)]
pub struct DorisConfig {
    pub environment: Environment,
    pub install_dir: PathBuf,
    pub conf_dir: PathBuf,
    pub log_dir: PathBuf,
    pub jdk_path: PathBuf,
    pub output_dir: PathBuf,
    pub timeout_seconds: u64,
    pub no_progress_animation: bool,

    // Process information
    pub process_pid: Option<u32>,
    pub process_command: Option<String>,
    pub last_detected: Option<chrono::DateTime<chrono::Utc>>,

    // BE specific configurations
    pub be_port: Option<u16>,
    pub brpc_port: Option<u16>,
    pub heartbeat_service_port: Option<u16>,
    pub webserver_port: Option<u16>,

    // BE process information for mixed deployment
    pub be_process_pid: Option<u32>,
    pub be_process_command: Option<String>,
    pub be_install_dir: Option<PathBuf>,

    // FE specific configurations
    pub http_port: Option<u16>,
    pub rpc_port: Option<u16>,
    pub query_port: Option<u16>,
    pub edit_log_port: Option<u16>,
    pub cloud_http_port: Option<u16>,
    pub meta_dir: Option<PathBuf>,

    // FE process information for mixed deployment
    pub fe_process_pid: Option<u32>,
    pub fe_process_command: Option<String>,
    pub fe_install_dir: Option<PathBuf>,

    // Network configurations
    pub priority_networks: Option<String>,
    pub meta_service_endpoint: Option<String>,
}

impl Default for DorisConfig {
    fn default() -> Self {
        Self {
            environment: Environment::Unknown,
            install_dir: PathBuf::from("/opt/selectdb"),
            conf_dir: PathBuf::from("/opt/selectdb/conf"),
            log_dir: PathBuf::from("/opt/selectdb/log"),
            jdk_path: PathBuf::from("/opt/jdk"),
            output_dir: PathBuf::from("/tmp/doris/collection"),
            timeout_seconds: 60,
            no_progress_animation: false,
            process_pid: None,
            process_command: None,
            last_detected: None,
            be_port: None,
            brpc_port: None,
            heartbeat_service_port: None,
            webserver_port: None,
            http_port: None,
            rpc_port: None,
            query_port: None,
            edit_log_port: None,
            cloud_http_port: None,
            meta_dir: None,
            priority_networks: None,
            meta_service_endpoint: None,
            be_process_pid: None,
            be_process_command: None,
            be_install_dir: None,
            fe_process_pid: None,
            fe_process_command: None,
            fe_install_dir: None,
        }
    }
}

impl DorisConfig {
    /// Get BE HTTP ports from configuration or return default ports[8040, 8041]
    pub fn get_be_http_ports(&self) -> Vec<u16> {
        if let Some(port) = self.webserver_port {
            vec![port]
        } else {
            vec![8040, 8041]
        }
    }

    /// Update configuration with values from app Config
    pub fn with_app_config(mut self, config: &crate::config::Config) -> Self {
        self.jdk_path = config.jdk_path.clone();
        self.output_dir = config.output_dir.clone();
        self.timeout_seconds = config.timeout_seconds;
        self.no_progress_animation = config.no_progress_animation;
        self
    }

    /// Check if the current process PID is still valid
    pub fn is_process_valid(&self) -> bool {
        match self.process_pid {
            Some(pid) => {
                // Check if process is still running
                std::process::Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .output()
                    .map(|output| output.status.success())
                    .unwrap_or(false)
            }
            None => false,
        }
    }

    /// Get current process PID if available and valid
    pub fn get_valid_pid(&self) -> Option<u32> {
        self.process_pid.filter(|_| self.is_process_valid())
    }
}

fn clean_process_info(config: &mut DorisConfig) {
    config.process_pid = None;
    config.process_command = None;
    config.last_detected = None;

    config.fe_process_pid = None;
    config.fe_process_command = None;
    config.fe_install_dir = None;
    config.be_process_pid = None;
    config.be_process_command = None;
    config.be_install_dir = None;
}

/// Update mixed deployment detection and environment setting
fn update_mixed_environment(config: &mut DorisConfig) -> Result<()> {
    // Detect if both FE and BE processes are running
    process_detector::detect_mixed_deployment(config)?;

    // Update environment to Mixed if both FE and BE processes are detected
    if config.fe_process_pid.is_some() && config.be_process_pid.is_some() {
        config.environment = Environment::Mixed;
    }

    Ok(())
}

fn persist_configuration(config: &DorisConfig) {
    if let Err(e) = config_persister::persist_config(config) {
        eprintln!("Warning: Failed to persist configuration: {e}");
    }
}

/// Apply environment-specific port configurations
fn apply_environment_specific_ports(
    config: &mut DorisConfig,
    parsed_config: &DorisConfig,
    env: Environment,
) {
    match env {
        Environment::BE => {
            apply_be_ports(config, parsed_config);
        }
        Environment::FE => {
            apply_fe_ports(config, parsed_config);
        }
        Environment::Mixed => {
            // For Mixed environment, apply both BE and FE configurations
            apply_be_ports(config, parsed_config);
            apply_fe_ports(config, parsed_config);
        }
        Environment::Unknown => {
            // No specific ports to apply for unknown environment
        }
    }
}

/// Apply BE-specific port configurations
fn apply_be_ports(config: &mut DorisConfig, parsed_config: &DorisConfig) {
    config.be_port = parsed_config.be_port;
    config.brpc_port = parsed_config.brpc_port;
    config.webserver_port = parsed_config.webserver_port;
    config.heartbeat_service_port = parsed_config.heartbeat_service_port;
}

/// Apply FE-specific port configurations
fn apply_fe_ports(config: &mut DorisConfig, parsed_config: &DorisConfig) {
    config.http_port = parsed_config.http_port;
    config.rpc_port = parsed_config.rpc_port;
    config.query_port = parsed_config.query_port;
    config.edit_log_port = parsed_config.edit_log_port;
    config.cloud_http_port = parsed_config.cloud_http_port;
    config.meta_dir = parsed_config.meta_dir.clone();
}

/// Load configuration, first from persisted file, then detect environment and generate if needed
pub fn load_config() -> Result<DorisConfig> {
    let mut config = config_persister::load_persisted_config().unwrap_or_default();

    match process_detector::detect_current_process() {
        Ok(current_process) => {
            if needs_config_update(&config, &current_process) {
                config = update_config_from_process(config, current_process)?;
                let _ = update_mixed_environment(&mut config);
                persist_configuration(&config);
            } else {
                let _ = update_mixed_environment(&mut config);
            }
        }
        Err(_) => {
            if config.process_pid.is_some() && !config.is_process_valid() {
                clean_process_info(&mut config);
                persist_configuration(&config);
            }

            if config.environment == Environment::Unknown {
                return fallback_load_config();
            }
        }
    }

    Ok(config)
}

/// Parse configuration based on environment type with fallback to default
fn parse_env_specific_config(env: Environment) -> DorisConfig {
    let result = match env {
        Environment::BE => config_parser::parse_be_config(),
        Environment::FE => config_parser::parse_fe_config(),
        Environment::Mixed => config_parser::parse_be_config(),
        Environment::Unknown => return DorisConfig::default(),
    };
    result.unwrap_or_else(|_| DorisConfig::default())
}

/// Fallback to original configuration loading behavior
fn fallback_load_config() -> Result<DorisConfig> {
    let env = match process_detector::detect_environment() {
        Ok(env) => env,
        Err(_) => {
            return Ok(DorisConfig::default());
        }
    };

    let mut config = parse_env_specific_config(env);
    config.environment = env;

    // If we detect both FE and BE processes, update to Mixed environment
    if env != Environment::Unknown {
        let _ = update_mixed_environment(&mut config);
    }

    persist_configuration(&config);
    Ok(config)
}

/// Convert DorisConfig to application Config
pub fn to_app_config(doris_config: DorisConfig) -> crate::config::Config {
    crate::config::Config {
        jdk_path: doris_config.jdk_path,
        output_dir: doris_config.output_dir,
        timeout_seconds: doris_config.timeout_seconds,
        no_progress_animation: doris_config.no_progress_animation,
    }
}

/// Get current process PID from configuration (convenience function)
pub fn get_current_pid() -> Option<u32> {
    load_config().ok()?.get_valid_pid()
}

/// Check if configuration needs to be updated based on detected process
fn needs_config_update(
    config: &DorisConfig,
    process: &process_detector::ProcessDetectionResult,
) -> bool {
    // Check if key configuration has changed
    config.process_pid != Some(process.pid)
        || config.environment != process.environment
        || config.install_dir != process.doris_home
        || config.jdk_path != process.java_home
        || !config.is_process_valid()
}

/// Update configuration based on detected process information
fn update_config_from_process(
    mut config: DorisConfig,
    process: process_detector::ProcessDetectionResult,
) -> Result<DorisConfig> {
    // Update process information
    config.process_pid = Some(process.pid);
    config.process_command = Some(process.command);
    config.last_detected = Some(chrono::Utc::now());

    // Update environment and paths
    config.environment = process.environment;
    config.install_dir = process.doris_home.clone();
    config.jdk_path = process.java_home.clone();

    // Update related paths based on environment
    config.conf_dir = process.doris_home.join("conf");
    config.log_dir = process.doris_home.join("log");

    // Try to parse configuration for port information using detected path
    if let Ok(parsed_config) =
        config_parser::parse_config_from_path(process.environment, &process.doris_home)
    {
        // Apply port configurations based on environment
        apply_environment_specific_ports(&mut config, &parsed_config, process.environment);
    }

    Ok(config)
}
