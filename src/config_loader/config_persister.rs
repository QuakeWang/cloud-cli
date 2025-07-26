use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config_loader::{DorisConfig, Environment, MySQLConfig};
use crate::error::{CliError, Result};

trait ConfigConverter<T> {
    fn convert_to(&self) -> T;
}

/// Serializable configuration structure with organized FE and BE sections
#[derive(Serialize, Deserialize)]
struct OrganizedConfig {
    metadata: Metadata,
    paths: CommonPaths,
    fe: Option<FeConfig>,
    be: Option<BeConfig>,
    network: Network,
    settings: Settings,
    process: ProcessInfo,
    mysql: Option<MySQLConfig>,
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    environment: String,
    version: String,
}

#[derive(Serialize, Deserialize)]
struct Paths {
    install_dir: String,
    conf_dir: String,
    log_dir: String,
    jdk_path: String,
    output_dir: String,
    meta_dir: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Ports {
    be_port: Option<u16>,
    brpc_port: Option<u16>,
    heartbeat_service_port: Option<u16>,
    webserver_port: Option<u16>,
    http_port: Option<u16>,
    rpc_port: Option<u16>,
    query_port: Option<u16>,
    edit_log_port: Option<u16>,
    cloud_http_port: Option<u16>,
}

#[derive(Serialize, Deserialize)]
struct Network {
    priority_networks: Option<String>,
    meta_service_endpoint: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Settings {
    timeout_seconds: u64,
    no_progress_animation: bool,
}

#[derive(Serialize, Deserialize)]
struct ProcessInfo {
    pid: Option<u32>,
    command: Option<String>,
    last_detected: Option<String>,
    be_process_pid: Option<u32>,
    be_process_command: Option<String>,
    be_install_dir: Option<String>,
    fe_process_pid: Option<u32>,
    fe_process_command: Option<String>,
    fe_install_dir: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CommonPaths {
    jdk_path: String,
    output_dir: String,
}

#[derive(Serialize, Deserialize)]
struct FeConfig {
    install_dir: String,
    conf_dir: String,
    log_dir: String,
    meta_dir: Option<String>,
    ports: FePorts,
    process_pid: Option<u32>,
    process_command: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct BeConfig {
    install_dir: String,
    conf_dir: String,
    log_dir: String,
    ports: BePorts,
    process_pid: Option<u32>,
    process_command: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FePorts {
    http_port: Option<u16>,
    rpc_port: Option<u16>,
    query_port: Option<u16>,
    edit_log_port: Option<u16>,
    cloud_http_port: Option<u16>,
}

#[derive(Serialize, Deserialize)]
struct BePorts {
    be_port: Option<u16>,
    brpc_port: Option<u16>,
    heartbeat_service_port: Option<u16>,
    webserver_port: Option<u16>,
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

impl ConfigConverter<Metadata> for DorisConfig {
    fn convert_to(&self) -> Metadata {
        let env_str = match self.environment {
            Environment::FE => "FE",
            Environment::BE => "BE",
            Environment::Mixed => "FE + BE",
            Environment::Unknown => "Unknown",
        };

        Metadata {
            environment: env_str.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl ConfigConverter<Paths> for DorisConfig {
    fn convert_to(&self) -> Paths {
        Paths {
            install_dir: path_to_string(&self.install_dir),
            conf_dir: path_to_string(&self.conf_dir),
            log_dir: path_to_string(&self.log_dir),
            jdk_path: path_to_string(&self.jdk_path),
            output_dir: path_to_string(&self.output_dir),
            meta_dir: self.meta_dir.as_ref().map(|p| path_to_string(p)),
        }
    }
}

impl ConfigConverter<Ports> for DorisConfig {
    fn convert_to(&self) -> Ports {
        Ports {
            be_port: self.be_port,
            brpc_port: self.brpc_port,
            heartbeat_service_port: self.heartbeat_service_port,
            webserver_port: self.webserver_port,
            http_port: self.http_port,
            rpc_port: self.rpc_port,
            query_port: self.query_port,
            edit_log_port: self.edit_log_port,
            cloud_http_port: self.cloud_http_port,
        }
    }
}

impl ConfigConverter<Network> for DorisConfig {
    fn convert_to(&self) -> Network {
        Network {
            priority_networks: self.priority_networks.clone(),
            meta_service_endpoint: self.meta_service_endpoint.clone(),
        }
    }
}

impl ConfigConverter<Settings> for DorisConfig {
    fn convert_to(&self) -> Settings {
        Settings {
            timeout_seconds: self.timeout_seconds,
            no_progress_animation: self.no_progress_animation,
        }
    }
}

impl ConfigConverter<ProcessInfo> for DorisConfig {
    fn convert_to(&self) -> ProcessInfo {
        ProcessInfo {
            pid: self.process_pid,
            command: self.process_command.clone(),
            last_detected: self.last_detected.map(|dt| dt.to_rfc3339()),
            be_process_pid: self.be_process_pid,
            be_process_command: self.be_process_command.clone(),
            be_install_dir: self.be_install_dir.as_ref().map(|p| path_to_string(p)),
            fe_process_pid: self.fe_process_pid,
            fe_process_command: self.fe_process_command.clone(),
            fe_install_dir: self.fe_install_dir.as_ref().map(|p| path_to_string(p)),
        }
    }
}

impl ConfigConverter<DorisConfig> for PersistentConfig {
    fn convert_to(&self) -> DorisConfig {
        let environment = match self.metadata.environment.as_str() {
            "FE" => Environment::FE,
            "BE" => Environment::BE,
            "FE + BE" => Environment::Mixed,
            _ => Environment::Unknown,
        };

        DorisConfig {
            environment,
            install_dir: PathBuf::from(&self.paths.install_dir),
            conf_dir: PathBuf::from(&self.paths.conf_dir),
            log_dir: PathBuf::from(&self.paths.log_dir),
            jdk_path: PathBuf::from(&self.paths.jdk_path),
            output_dir: PathBuf::from(&self.paths.output_dir),
            timeout_seconds: self.settings.timeout_seconds,
            no_progress_animation: self.settings.no_progress_animation,
            process_pid: self.process.pid,
            process_command: self.process.command.clone(),
            last_detected: self
                .process
                .last_detected
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            be_process_pid: self.process.be_process_pid,
            be_process_command: self.process.be_process_command.clone(),
            be_install_dir: self.process.be_install_dir.as_ref().map(PathBuf::from),
            fe_process_pid: self.process.fe_process_pid,
            fe_process_command: self.process.fe_process_command.clone(),
            fe_install_dir: self.process.fe_install_dir.as_ref().map(PathBuf::from),
            be_port: self.ports.be_port,
            brpc_port: self.ports.brpc_port,
            heartbeat_service_port: self.ports.heartbeat_service_port,
            webserver_port: self.ports.webserver_port,
            http_port: self.ports.http_port,
            rpc_port: self.ports.rpc_port,
            query_port: self.ports.query_port,
            edit_log_port: self.ports.edit_log_port,
            cloud_http_port: self.ports.cloud_http_port,
            meta_dir: self.paths.meta_dir.as_ref().map(PathBuf::from),
            priority_networks: self.network.priority_networks.clone(),
            meta_service_endpoint: self.network.meta_service_endpoint.clone(),
            mysql: self.mysql.clone(),
        }
    }
}

/// Convert persistent format to internal config
fn from_persistent_config(persistent: PersistentConfig) -> DorisConfig {
    let environment = match persistent.metadata.environment.as_str() {
        "FE" => Environment::FE,
        "BE" => Environment::BE,
        "FE + BE" => Environment::Mixed,
        _ => Environment::Unknown,
    };

    DorisConfig {
        environment,
        install_dir: PathBuf::from(&persistent.paths.install_dir),
        conf_dir: PathBuf::from(&persistent.paths.conf_dir),
        log_dir: PathBuf::from(&persistent.paths.log_dir),
        jdk_path: PathBuf::from(&persistent.paths.jdk_path),
        output_dir: PathBuf::from(&persistent.paths.output_dir),
        timeout_seconds: persistent.settings.timeout_seconds,
        no_progress_animation: persistent.settings.no_progress_animation,
        process_pid: persistent.process.pid,
        process_command: persistent.process.command.clone(),
        last_detected: persistent
            .process
            .last_detected
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        be_process_pid: persistent.process.be_process_pid,
        be_process_command: persistent.process.be_process_command.clone(),
        be_install_dir: persistent
            .process
            .be_install_dir
            .as_ref()
            .map(PathBuf::from),
        fe_process_pid: persistent.process.fe_process_pid,
        fe_process_command: persistent.process.fe_process_command.clone(),
        fe_install_dir: persistent
            .process
            .fe_install_dir
            .as_ref()
            .map(PathBuf::from),
        be_port: persistent.ports.be_port,
        brpc_port: persistent.ports.brpc_port,
        heartbeat_service_port: persistent.ports.heartbeat_service_port,
        webserver_port: persistent.ports.webserver_port,
        http_port: persistent.ports.http_port,
        rpc_port: persistent.ports.rpc_port,
        query_port: persistent.ports.query_port,
        edit_log_port: persistent.ports.edit_log_port,
        cloud_http_port: persistent.ports.cloud_http_port,
        meta_dir: persistent.paths.meta_dir.as_ref().map(PathBuf::from),
        priority_networks: persistent.network.priority_networks.clone(),
        meta_service_endpoint: persistent.network.meta_service_endpoint.clone(),
        mysql: persistent.mysql.clone(),
    }
}

/// Get configuration file paths in order of preference
fn get_config_file_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    // Only use the standard user config directory path
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(
            home_dir
                .join(".config")
                .join("cloud-cli")
                .join("config.toml"),
        );
    }

    if paths.is_empty() {
        return Err(CliError::ConfigError(
            "Could not determine user home directory for config path".to_string(),
        ));
    }

    Ok(paths)
}

/// Try to create directory if it doesn't exist
fn ensure_dir_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                CliError::ConfigError(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }
    }
    Ok(())
}

/// Test if a path is writable
fn is_path_writable(path: &Path) -> bool {
    let test_file = path.with_file_name(".write_test_temp");
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(test_file);
            true
        }
        Err(_) => false,
    }
}

pub enum PersistResult {
    Success(PathBuf),
    PartialSuccess(PathBuf, Vec<(PathBuf, String)>),
    AllFailed(Vec<(PathBuf, String)>),
}

impl PersistResult {
    pub fn is_success(&self) -> bool {
        !matches!(self, PersistResult::AllFailed(_))
    }
}

/// Persist configuration to file
pub fn persist_config(config: &DorisConfig) -> Result<PersistResult> {
    let config_paths = get_config_file_paths()?;
    let organized_config = to_organized_config(config);
    let toml_str = toml::to_string_pretty(&organized_config)?;

    let mut errors = Vec::new();

    for config_path in &config_paths {
        if let Err(e) = ensure_dir_exists(config_path) {
            errors.push((
                config_path.clone(),
                format!("Failed to create directory: {e}"),
            ));
            continue;
        }

        if !is_path_writable(config_path.parent().unwrap_or(config_path)) {
            errors.push((config_path.clone(), "No write permission".to_string()));
            continue;
        }

        match fs::File::create(config_path) {
            Ok(mut file) => match file.write_all(toml_str.as_bytes()) {
                Ok(_) => {
                    if errors.is_empty() {
                        return Ok(PersistResult::Success(config_path.clone()));
                    } else {
                        return Ok(PersistResult::PartialSuccess(config_path.clone(), errors));
                    }
                }
                Err(e) => {
                    errors.push((config_path.clone(), format!("Write error: {e}")));
                }
            },
            Err(e) => {
                errors.push((config_path.clone(), format!("Create file error: {e}")));
            }
        }
    }

    if !errors.is_empty() {
        Ok(PersistResult::AllFailed(errors))
    } else {
        Err(CliError::ConfigError(
            "No valid paths to persist config".to_string(),
        ))
    }
}

fn migrate_legacy_config(content: &str, config_path: &Path) -> Option<DorisConfig> {
    #[derive(Deserialize)]
    struct LegacyConfig {
        metadata: Metadata,
        paths: Paths,
        ports: Ports,
        network: Network,
        settings: Settings,
    }

    match toml::from_str::<LegacyConfig>(content) {
        Ok(legacy) => {
            let new_config = PersistentConfig {
                metadata: legacy.metadata,
                paths: legacy.paths,
                ports: legacy.ports,
                network: legacy.network,
                settings: legacy.settings,
                process: ProcessInfo {
                    pid: None,
                    command: None,
                    last_detected: None,
                    be_process_pid: None,
                    be_process_command: None,
                    be_install_dir: None,
                    fe_process_pid: None,
                    fe_process_command: None,
                    fe_install_dir: None,
                },
                mysql: None,
            };

            match toml::to_string_pretty(&new_config) {
                Ok(new_content) => {
                    if let Err(e) = fs::write(config_path, new_content) {
                        eprintln!("Warning: Failed to save migrated config: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to serialize migrated config: {e}");
                }
            }

            Some(new_config.convert_to())
        }
        Err(_) => None,
    }
}

/// Load persisted configuration from file
pub fn load_persisted_config() -> Result<DorisConfig> {
    let config_paths = get_config_file_paths()?;
    let mut last_error = None;

    for config_path in config_paths {
        if !config_path.exists() {
            continue;
        }

        match fs::read_to_string(&config_path) {
            Ok(content) => {
                if let Some(config) = parse_legacy_config_with_mysql(&content) {
                    return Ok(config);
                }

                if let Ok(organized_config) = toml::from_str::<OrganizedConfig>(&content) {
                    return Ok(from_organized_config(&organized_config));
                }

                match toml::from_str::<PersistentConfig>(&content) {
                    Ok(persistent_config) => {
                        return Ok(from_persistent_config(persistent_config));
                    }
                    Err(e) => {
                        if e.to_string().contains("missing field `process`") {
                            if let Some(config) = migrate_legacy_config(&content, &config_path) {
                                return Ok(config);
                            }
                        }

                        last_error = Some(CliError::ConfigError(format!(
                            "Failed to parse config file {}: {}",
                            config_path.display(),
                            e
                        )));
                    }
                }
            }
            Err(e) => {
                last_error = Some(CliError::ConfigError(format!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                )));
            }
        }
    }

    match last_error {
        Some(e) => {
            eprintln!("Warning: {e}");
            Ok(DorisConfig::default())
        }
        None => Ok(DorisConfig::default()),
    }
}

/// Parse legacy config format that includes mysql section
fn parse_legacy_config_with_mysql(content: &str) -> Option<DorisConfig> {
    #[derive(Deserialize)]
    struct LegacyConfigWithMySQL {
        metadata: Metadata,
        paths: CommonPaths,
        network: Network,
        settings: Settings,
        process: ProcessInfo,
        mysql: Option<MySQLConfig>,
    }

    match toml::from_str::<LegacyConfigWithMySQL>(content) {
        Ok(legacy) => {
            let environment = match legacy.metadata.environment.as_str() {
                "FE" => Environment::FE,
                "BE" => Environment::BE,
                "FE + BE" => Environment::Mixed,
                _ => Environment::Unknown,
            };

            Some(DorisConfig {
                environment,
                install_dir: PathBuf::from("/opt/selectdb"),
                conf_dir: PathBuf::from("/opt/selectdb/conf"),
                log_dir: PathBuf::from("/opt/selectdb/log"),
                jdk_path: PathBuf::from(&legacy.paths.jdk_path),
                output_dir: PathBuf::from(&legacy.paths.output_dir),
                timeout_seconds: legacy.settings.timeout_seconds,
                no_progress_animation: legacy.settings.no_progress_animation,
                process_pid: legacy.process.pid,
                process_command: legacy.process.command.clone(),
                last_detected: legacy
                    .process
                    .last_detected
                    .as_ref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
                be_process_pid: legacy.process.be_process_pid,
                be_process_command: legacy.process.be_process_command.clone(),
                be_install_dir: legacy.process.be_install_dir.as_ref().map(PathBuf::from),
                fe_process_pid: legacy.process.fe_process_pid,
                fe_process_command: legacy.process.fe_process_command.clone(),
                fe_install_dir: legacy.process.fe_install_dir.as_ref().map(PathBuf::from),
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
                priority_networks: legacy.network.priority_networks.clone(),
                meta_service_endpoint: legacy.network.meta_service_endpoint.clone(),
                mysql: legacy.mysql,
            })
        }
        Err(_) => None,
    }
}

/// Convert organized config to internal config
fn from_organized_config(organized: &OrganizedConfig) -> DorisConfig {
    let environment = match organized.metadata.environment.as_str() {
        "FE" => Environment::FE,
        "BE" => Environment::BE,
        "FE + BE" => Environment::Mixed,
        _ => Environment::Unknown,
    };

    let mut config = DorisConfig {
        environment,
        // Default paths
        install_dir: PathBuf::new(),
        conf_dir: PathBuf::new(),
        log_dir: PathBuf::new(),

        // Common paths
        jdk_path: PathBuf::from(&organized.paths.jdk_path),
        output_dir: PathBuf::from(&organized.paths.output_dir),

        // Settings
        timeout_seconds: organized.settings.timeout_seconds,
        no_progress_animation: organized.settings.no_progress_animation,

        // Network
        priority_networks: organized.network.priority_networks.clone(),
        meta_service_endpoint: organized.network.meta_service_endpoint.clone(),

        // Process info from the common section
        process_pid: organized.process.pid,
        process_command: organized.process.command.clone(),
        last_detected: organized
            .process
            .last_detected
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),

        // Initialize all other fields to None/default
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
        be_process_pid: None,
        be_process_command: None,
        be_install_dir: None,
        fe_process_pid: None,
        fe_process_command: None,
        fe_install_dir: None,
        mysql: organized.mysql.clone(),
    };

    // Set BE specific configurations if available
    if let Some(be) = &organized.be {
        config.be_port = be.ports.be_port;
        config.brpc_port = be.ports.brpc_port;
        config.heartbeat_service_port = be.ports.heartbeat_service_port;
        config.webserver_port = be.ports.webserver_port;

        if environment == Environment::BE {
            config.install_dir = PathBuf::from(&be.install_dir);
            config.conf_dir = PathBuf::from(&be.conf_dir);
            config.log_dir = PathBuf::from(&be.log_dir);
        }

        config.be_process_pid = be.process_pid;
        config.be_process_command = be.process_command.clone();
        config.be_install_dir = Some(PathBuf::from(&be.install_dir));
    }

    // Set FE specific configurations if available
    if let Some(fe) = &organized.fe {
        config.http_port = fe.ports.http_port;
        config.rpc_port = fe.ports.rpc_port;
        config.query_port = fe.ports.query_port;
        config.edit_log_port = fe.ports.edit_log_port;
        config.cloud_http_port = fe.ports.cloud_http_port;
        config.meta_dir = fe.meta_dir.as_ref().map(PathBuf::from);

        if environment == Environment::FE {
            config.install_dir = PathBuf::from(&fe.install_dir);
            config.conf_dir = PathBuf::from(&fe.conf_dir);
            config.log_dir = PathBuf::from(&fe.log_dir);
        }

        config.fe_process_pid = fe.process_pid;
        config.fe_process_command = fe.process_command.clone();
        config.fe_install_dir = Some(PathBuf::from(&fe.install_dir));
    }

    // For mixed environment, prioritize BE for main install_dir
    if environment == Environment::Mixed {
        if let Some(be) = &organized.be {
            config.install_dir = PathBuf::from(&be.install_dir);
            config.conf_dir = PathBuf::from(&be.conf_dir);
            config.log_dir = PathBuf::from(&be.log_dir);
        } else if let Some(fe) = &organized.fe {
            config.install_dir = PathBuf::from(&fe.install_dir);
            config.conf_dir = PathBuf::from(&fe.conf_dir);
            config.log_dir = PathBuf::from(&fe.log_dir);
        }
    }

    config
}

/// Serializable configuration structure
#[derive(Serialize, Deserialize)]
struct PersistentConfig {
    metadata: Metadata,
    paths: Paths,
    ports: Ports,
    network: Network,
    settings: Settings,
    process: ProcessInfo,
    mysql: Option<MySQLConfig>,
}

/// Convert DorisConfig to the new organized format
fn to_organized_config(config: &DorisConfig) -> OrganizedConfig {
    // Common paths
    let common_paths = CommonPaths {
        jdk_path: path_to_string(&config.jdk_path),
        output_dir: path_to_string(&config.output_dir),
    };

    // FE configuration
    let fe_config =
        if config.environment == Environment::FE || config.environment == Environment::Mixed {
            let fe_install_dir = config
                .fe_install_dir
                .as_ref()
                .unwrap_or(&config.install_dir);
            Some(FeConfig {
                install_dir: path_to_string(fe_install_dir),
                conf_dir: path_to_string(&fe_install_dir.join("conf")),
                log_dir: path_to_string(&fe_install_dir.join("log")),
                meta_dir: config.meta_dir.as_ref().map(|p| path_to_string(p)),
                ports: FePorts {
                    http_port: config.http_port,
                    rpc_port: config.rpc_port,
                    query_port: config.query_port,
                    edit_log_port: config.edit_log_port,
                    cloud_http_port: config.cloud_http_port,
                },
                process_pid: if config.environment == Environment::FE {
                    config.process_pid
                } else {
                    config.fe_process_pid
                },
                process_command: if config.environment == Environment::FE {
                    config.process_command.clone()
                } else {
                    config.fe_process_command.clone()
                },
            })
        } else {
            None
        };

    // BE configuration
    let be_config =
        if config.environment == Environment::BE || config.environment == Environment::Mixed {
            let be_install_dir = config
                .be_install_dir
                .as_ref()
                .unwrap_or(&config.install_dir);
            Some(BeConfig {
                install_dir: path_to_string(be_install_dir),
                conf_dir: path_to_string(&be_install_dir.join("conf")),
                log_dir: path_to_string(&be_install_dir.join("log")),
                ports: BePorts {
                    be_port: config.be_port,
                    brpc_port: config.brpc_port,
                    heartbeat_service_port: config.heartbeat_service_port,
                    webserver_port: config.webserver_port,
                },
                process_pid: if config.environment == Environment::BE {
                    config.process_pid
                } else {
                    config.be_process_pid
                },
                process_command: if config.environment == Environment::BE {
                    config.process_command.clone()
                } else {
                    config.be_process_command.clone()
                },
            })
        } else {
            None
        };

    // Create organized config
    OrganizedConfig {
        metadata: config.convert_to(),
        paths: common_paths,
        fe: fe_config,
        be: be_config,
        network: config.convert_to(),
        settings: config.convert_to(),
        process: config.convert_to(),
        mysql: config.mysql.clone(),
    }
}
