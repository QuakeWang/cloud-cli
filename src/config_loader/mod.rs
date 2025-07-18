use crate::error::Result;
use std::path::PathBuf;

pub mod config_parser;
pub mod config_persister;
pub mod process_detector;

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    FE,
    BE,
    Unknown,
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::FE => write!(f, "FE"),
            Environment::BE => write!(f, "BE"),
            Environment::Unknown => write!(f, "Unknown"),
        }
    }
}

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
    // BE specific configurations
    pub be_port: Option<u16>,
    pub brpc_port: Option<u16>,
    pub heartbeat_service_port: Option<u16>,
    pub webserver_port: Option<u16>,
    // FE specific configurations
    pub http_port: Option<u16>,
    pub rpc_port: Option<u16>,
    pub query_port: Option<u16>,
    pub edit_log_port: Option<u16>,
    pub cloud_http_port: Option<u16>,
    pub meta_dir: Option<PathBuf>,
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
            output_dir: PathBuf::from("/opt/selectdb/information"),
            timeout_seconds: 60,
            no_progress_animation: false,
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
}

/// Load configuration, first from persisted file, then detect environment and generate if needed
pub fn load_config() -> Result<DorisConfig> {
    if let Ok(config) = config_persister::load_persisted_config() {
        return Ok(config);
    }

    let env = match process_detector::detect_environment() {
        Ok(env) => env,
        Err(e) => {
            eprintln!("\x1b[31m Warning: {}\x1b[0m", e);
            return Ok(DorisConfig::default());
        }
    };

    let mut config = match env {
        Environment::BE => parse_be_config_with_fallback(),
        Environment::FE => parse_fe_config_with_fallback(),
        Environment::Unknown => {
            eprintln!(
                "\x1b[31m Warning: No Doris process detected. Using default configuration.\x1b[0m"
            );
            DorisConfig::default()
        }
    };

    config.environment = env;

    let _ = config_persister::persist_config(&config);

    Ok(config)
}

/// Parse BE configuration with fallback to default
fn parse_be_config_with_fallback() -> DorisConfig {
    match config_parser::parse_be_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "\x1b[31m Warning: Failed to parse BE configuration: {}. Using default configuration.\x1b[0m",
                e
            );
            DorisConfig::default()
        }
    }
}

/// Parse FE configuration with fallback to default
fn parse_fe_config_with_fallback() -> DorisConfig {
    match config_parser::parse_fe_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "\x1b[31m Warning: Failed to parse FE configuration: {}. Using default configuration.\x1b[0m",
                e
            );
            DorisConfig::default()
        }
    }
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

/// Get the current Doris configuration
pub fn get_current_config() -> Result<DorisConfig> {
    load_config()
}
