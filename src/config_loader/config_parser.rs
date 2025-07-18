use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::config_loader::DorisConfig;
use crate::config_loader::process_detector;
use crate::error::{CliError, Result};

// Common configuration keys
const LOG_DIR_KEY: &str = "LOG_DIR";
const PRIORITY_NETWORKS_KEY: &str = "priority_networks";
const META_SERVICE_KEY: &str = "meta_service_endpoint";

/// Parse BE configuration
pub fn parse_be_config() -> Result<DorisConfig> {
    let (install_dir, jdk_path) = process_detector::get_be_paths()?;
    let conf_dir = install_dir.join("conf");
    let be_conf_path = process_detector::get_be_config_path()?;

    process_detector::verify_config_file(&be_conf_path)?;

    let content = fs::read_to_string(&be_conf_path)
        .map_err(|e| CliError::ConfigError(format!("Failed to read BE config file: {e}")))?;

    let install_dir_clone = install_dir.clone();

    let mut config = DorisConfig {
        install_dir,
        conf_dir,
        jdk_path,
        ..DorisConfig::default()
    };

    parse_config_content(&content, &mut config, &install_dir_clone)?;

    Ok(config)
}

/// Parse FE configuration
pub fn parse_fe_config() -> Result<DorisConfig> {
    let (install_dir, jdk_path) = process_detector::get_fe_paths()?;
    let conf_dir = install_dir.join("conf");
    let fe_conf_path = process_detector::get_fe_config_path()?;

    process_detector::verify_config_file(&fe_conf_path)?;

    let content = fs::read_to_string(&fe_conf_path)
        .map_err(|e| CliError::ConfigError(format!("Failed to read FE config file: {e}")))?;

    let mut config = DorisConfig {
        install_dir,
        conf_dir,
        jdk_path,
        ..DorisConfig::default()
    };

    parse_fe_config_content(&content, &mut config)?;

    Ok(config)
}

/// Parse BE config content
fn parse_config_content(content: &str, config: &mut DorisConfig, install_dir: &Path) -> Result<()> {
    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        // Parse LOG_DIR
        if line.starts_with(LOG_DIR_KEY) {
            if let Some(log_dir) = extract_value(line) {
                // Handle variable substitution, e.g., ${DORIS_HOME}/log
                if log_dir.contains("${DORIS_HOME}") {
                    let replaced =
                        log_dir.replace("${DORIS_HOME}", install_dir.to_str().unwrap_or(""));
                    config.log_dir = PathBuf::from(replaced);
                } else {
                    config.log_dir = PathBuf::from(log_dir);
                }
            }
        }

        // Parse ports and network config
        parse_key_value(line, "be_port", &mut config.be_port)?;
        parse_key_value(line, "brpc_port", &mut config.brpc_port)?;
        parse_key_value(
            line,
            "heartbeat_service_port",
            &mut config.heartbeat_service_port,
        )?;
        parse_key_value(line, "webserver_port", &mut config.webserver_port)?;
        parse_key_value(line, PRIORITY_NETWORKS_KEY, &mut config.priority_networks)?;
        parse_key_value(line, META_SERVICE_KEY, &mut config.meta_service_endpoint)?;
    }

    Ok(())
}

/// Extract value from a key=value or key = value line
fn extract_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = if line.contains('=') {
        line.splitn(2, '=').collect()
    } else if line.contains(" = ") {
        line.splitn(2, " = ").collect()
    } else {
        return None;
    };

    if parts.len() == 2 {
        Some(parts[1].trim().trim_matches('"').to_string())
    } else {
        None
    }
}

/// Parse FE config content
fn parse_fe_config_content(content: &str, config: &mut DorisConfig) -> Result<()> {
    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        // Parse LOG_DIR
        if line.starts_with(LOG_DIR_KEY) {
            if let Some(log_dir) = extract_value(line) {
                config.log_dir = PathBuf::from(log_dir);
            }
        }

        // Parse ports and network config
        parse_key_value(line, "http_port", &mut config.http_port)?;
        parse_key_value(line, "rpc_port", &mut config.rpc_port)?;
        parse_key_value(line, "query_port", &mut config.query_port)?;
        parse_key_value(line, "edit_log_port", &mut config.edit_log_port)?;
        parse_key_value(line, "cloud_http_port", &mut config.cloud_http_port)?;
        parse_key_value(line, PRIORITY_NETWORKS_KEY, &mut config.priority_networks)?;
        parse_key_value(line, META_SERVICE_KEY, &mut config.meta_service_endpoint)?;

        // Parse metadata directory
        if line.starts_with("meta_dir") {
            if let Some(meta_dir) = extract_value(line) {
                config.meta_dir = Some(PathBuf::from(meta_dir));
            }
        }
    }

    Ok(())
}

/// Generic key-value parser
fn parse_key_value<T: FromStr>(line: &str, key: &str, value: &mut Option<T>) -> Result<()> {
    if line.starts_with(key) && (line.contains('=') || line.contains(" = ")) {
        if let Some(val_str) = extract_value(line) {
            if let Ok(parsed_val) = val_str.parse::<T>() {
                *value = Some(parsed_val);
            }
        }
    }
    Ok(())
}
