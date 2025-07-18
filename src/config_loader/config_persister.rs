use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config_loader::{DorisConfig, Environment};
use crate::error::{CliError, Result};

/// Serializable configuration structure
#[derive(Serialize, Deserialize)]
struct PersistentConfig {
    metadata: Metadata,
    paths: Paths,
    ports: Ports,
    network: Network,
    settings: Settings,
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

/// Convert internal config to persistent format
fn to_persistent_config(config: &DorisConfig) -> PersistentConfig {
    let env_str = match config.environment {
        Environment::FE => "FE",
        Environment::BE => "BE",
        Environment::Unknown => "Unknown",
    };

    PersistentConfig {
        metadata: Metadata {
            environment: env_str.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        paths: Paths {
            install_dir: config.install_dir.to_string_lossy().to_string(),
            conf_dir: config.conf_dir.to_string_lossy().to_string(),
            log_dir: config.log_dir.to_string_lossy().to_string(),
            jdk_path: config.jdk_path.to_string_lossy().to_string(),
            output_dir: config.output_dir.to_string_lossy().to_string(),
            meta_dir: config
                .meta_dir
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
        },
        ports: Ports {
            be_port: config.be_port,
            brpc_port: config.brpc_port,
            heartbeat_service_port: config.heartbeat_service_port,
            webserver_port: config.webserver_port,
            http_port: config.http_port,
            rpc_port: config.rpc_port,
            query_port: config.query_port,
            edit_log_port: config.edit_log_port,
            cloud_http_port: config.cloud_http_port,
        },
        network: Network {
            priority_networks: config.priority_networks.clone(),
            meta_service_endpoint: config.meta_service_endpoint.clone(),
        },
        settings: Settings {
            timeout_seconds: config.timeout_seconds,
            no_progress_animation: config.no_progress_animation,
        },
    }
}

/// Convert persistent config to internal format
fn from_persistent_config(persistent: PersistentConfig) -> DorisConfig {
    let environment = match persistent.metadata.environment.as_str() {
        "FE" => Environment::FE,
        "BE" => Environment::BE,
        _ => Environment::Unknown,
    };

    DorisConfig {
        environment,
        install_dir: PathBuf::from(persistent.paths.install_dir),
        conf_dir: PathBuf::from(persistent.paths.conf_dir),
        log_dir: PathBuf::from(persistent.paths.log_dir),
        jdk_path: PathBuf::from(persistent.paths.jdk_path),
        output_dir: PathBuf::from(persistent.paths.output_dir),
        timeout_seconds: persistent.settings.timeout_seconds,
        no_progress_animation: persistent.settings.no_progress_animation,
        be_port: persistent.ports.be_port,
        brpc_port: persistent.ports.brpc_port,
        heartbeat_service_port: persistent.ports.heartbeat_service_port,
        webserver_port: persistent.ports.webserver_port,
        http_port: persistent.ports.http_port,
        rpc_port: persistent.ports.rpc_port,
        query_port: persistent.ports.query_port,
        edit_log_port: persistent.ports.edit_log_port,
        cloud_http_port: persistent.ports.cloud_http_port,
        meta_dir: persistent.paths.meta_dir.map(PathBuf::from),
        priority_networks: persistent.network.priority_networks,
        meta_service_endpoint: persistent.network.meta_service_endpoint,
    }
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

/// Persist configuration to file
pub fn persist_config(config: &DorisConfig) -> Result<()> {
    let config_paths = get_config_file_paths()?;
    let mut success = false;

    // Convert to persistent format
    let persistent_config = to_persistent_config(config);
    let toml_str = toml::to_string_pretty(&persistent_config)
        .map_err(|e| CliError::ConfigError(format!("Failed to serialize config: {e}")))?;

    // Try each path in order until one succeeds
    for config_path in config_paths {
        // Ensure parent directory exists
        if let Err(e) = ensure_dir_exists(&config_path) {
            eprintln!("\x1b[33m Notice: Failed to create directory for config: {e}\x1b[0m");
            continue;
        }

        // Check if we have write permission
        if !is_path_writable(config_path.parent().unwrap_or(&config_path)) {
            continue;
        }

        // Try to write the file
        match fs::File::create(&config_path) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(toml_str.as_bytes()) {
                    eprintln!(
                        "\x1b[33m Notice: Failed to write config to {}: {}\x1b[0m",
                        config_path.display(),
                        e
                    );
                    continue;
                }

                // Successfully wrote the file
                success = true;
                break;
            }
            Err(e) => {
                eprintln!(
                    "\x1b[33m Notice: Failed to create config file {}: {}\x1b[0m",
                    config_path.display(),
                    e
                );
                continue;
            }
        }
    }

    if !success {
        eprintln!(
            "\x1b[33m Notice: Could not persist configuration to any location. Configuration will not be saved.\x1b[0m"
        );
    }

    // Don't return an error even if we couldn't persist - the application should still work
    Ok(())
}

/// Load persisted configuration from file
pub fn load_persisted_config() -> Result<DorisConfig> {
    let config_paths = get_config_file_paths()?;

    for config_path in config_paths {
        if !config_path.exists() {
            continue;
        }

        // Try to read the file
        match fs::read_to_string(&config_path) {
            Ok(content) => {
                // Try to parse the content
                match toml::from_str::<PersistentConfig>(&content) {
                    Ok(persistent_config) => {
                        // Successfully read and parsed the file
                        return Ok(from_persistent_config(persistent_config));
                    }
                    Err(e) => {
                        eprintln!(
                            "\x1b[33m Notice: Failed to parse config file {}: {}\x1b[0m",
                            config_path.display(),
                            e
                        );
                        continue;
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "\x1b[33m Notice: Failed to read config file {}: {}\x1b[0m",
                    config_path.display(),
                    e
                );
                continue;
            }
        }
    }

    // If we get here, we couldn't load from any location
    Err(CliError::ConfigError(
        "No valid configuration file found".to_string(),
    ))
}
