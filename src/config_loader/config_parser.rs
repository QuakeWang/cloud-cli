use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::config_loader::process_detector;
use crate::config_loader::regex_utils;
use crate::config_loader::{DorisConfig, Environment};
use crate::error::{CliError, Result};

// Type aliases for complex function pointer types
type PortParserFn = fn(&str, &str, &mut Option<u16>) -> Result<()>;
type PathParserFn = fn(&str, &str, &mut Option<PathBuf>) -> Result<()>;
type StringParserFn = fn(&str, &str, &mut Option<String>) -> Result<()>;

// Common configuration keys
const LOG_DIR_KEY: &str = "LOG_DIR";
const PRIORITY_NETWORKS_KEY: &str = "priority_networks";
const META_SERVICE_KEY: &str = "meta_service_endpoint";

trait ConfigParser {
    fn parse_line(&self, line: &str, config: &mut DorisConfig) -> Result<()>;
}

struct PortConfigParser<'a> {
    items: Vec<(&'a str, PortParserFn)>,
}

impl<'a> PortConfigParser<'a> {
    fn new(items: Vec<(&'a str, PortParserFn)>) -> Self {
        Self { items }
    }
}

impl<'a> ConfigParser for PortConfigParser<'a> {
    fn parse_line(&self, line: &str, config: &mut DorisConfig) -> Result<()> {
        for (key, parse_fn) in &self.items {
            let target = match *key {
                "be_port" => &mut config.be_port,
                "brpc_port" => &mut config.brpc_port,
                "heartbeat_service_port" => &mut config.heartbeat_service_port,
                "webserver_port" => &mut config.webserver_port,
                "http_port" => &mut config.http_port,
                "rpc_port" => &mut config.rpc_port,
                "query_port" => &mut config.query_port,
                "edit_log_port" => &mut config.edit_log_port,
                "cloud_http_port" => &mut config.cloud_http_port,
                _ => continue,
            };

            parse_fn(line, key, target)?;
        }
        Ok(())
    }
}

struct PathConfigParser<'a> {
    items: Vec<(&'a str, PathParserFn)>,
}

impl<'a> PathConfigParser<'a> {
    fn new(items: Vec<(&'a str, PathParserFn)>) -> Self {
        Self { items }
    }
}

impl<'a> ConfigParser for PathConfigParser<'a> {
    fn parse_line(&self, line: &str, config: &mut DorisConfig) -> Result<()> {
        for (key, parse_fn) in &self.items {
            let target = match *key {
                "meta_dir" => &mut config.meta_dir,
                _ => continue,
            };

            parse_fn(line, key, target)?;
        }
        Ok(())
    }
}

struct StringConfigParser<'a> {
    items: Vec<(&'a str, StringParserFn)>,
}

impl<'a> StringConfigParser<'a> {
    fn new(items: Vec<(&'a str, StringParserFn)>) -> Self {
        Self { items }
    }
}

impl<'a> ConfigParser for StringConfigParser<'a> {
    fn parse_line(&self, line: &str, config: &mut DorisConfig) -> Result<()> {
        for (key, parse_fn) in &self.items {
            let target = match *key {
                "priority_networks" => &mut config.priority_networks,
                "meta_service_endpoint" => &mut config.meta_service_endpoint,
                _ => continue,
            };

            parse_fn(line, key, target)?;
        }
        Ok(())
    }
}

/// Parse configuration from specified path
pub fn parse_config_from_path(env: Environment, install_dir: &Path) -> Result<DorisConfig> {
    let jdk_path = PathBuf::from("/opt/jdk");
    parse_config_internal(env, install_dir, &jdk_path)
}

/// Internal helper for parsing configuration from path with JDK path
fn parse_config_internal(
    env: Environment,
    install_dir: &Path,
    jdk_path: &Path,
) -> Result<DorisConfig> {
    let conf_dir = install_dir.join("conf");

    let config_file = match env {
        Environment::BE => "be.conf",
        Environment::FE => "fe.conf",
        _ => return Err(CliError::ConfigError("Invalid environment".to_string())),
    };

    let conf_path = conf_dir.join(config_file);
    process_detector::verify_config_file(&conf_path)?;

    let error_msg = match env {
        Environment::BE => "Failed to read BE config file",
        Environment::FE => "Failed to read FE config file",
        _ => unreachable!(),
    };

    let content = fs::read_to_string(&conf_path)
        .map_err(|e| CliError::ConfigError(format!("{error_msg}: {e}")))?;

    let mut config = DorisConfig {
        environment: env,
        install_dir: install_dir.to_path_buf(),
        conf_dir,
        jdk_path: jdk_path.to_path_buf(),
        ..DorisConfig::default()
    };

    let install_dir_param = if env == Environment::BE {
        Some(install_dir)
    } else {
        None
    };
    parse_config_content(env, &content, &mut config, install_dir_param)?;

    Ok(config)
}

/// Parse BE configuration
pub fn parse_be_config() -> Result<DorisConfig> {
    let (install_dir, jdk_path) = process_detector::get_paths(Environment::BE)?;
    parse_config_internal(Environment::BE, &install_dir, &jdk_path)
}

/// Parse FE configuration
pub fn parse_fe_config() -> Result<DorisConfig> {
    let (install_dir, jdk_path) = process_detector::get_paths(Environment::FE)?;
    parse_config_internal(Environment::FE, &install_dir, &jdk_path)
}

/// Parse config content based on environment
fn parse_config_content(
    env: Environment,
    content: &str,
    config: &mut DorisConfig,
    install_dir: Option<&Path>,
) -> Result<()> {
    let port_parser = PortConfigParser::new(get_env_config_items(env));
    let path_parser = PathConfigParser::new(get_env_path_config_items(env));
    let common_parser = StringConfigParser::new(get_common_config_items());

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if line.starts_with(LOG_DIR_KEY) {
            if let Some(log_dir) = extract_value(line) {
                match install_dir {
                    Some(install) if log_dir.contains("${DORIS_HOME}") => {
                        let replaced =
                            log_dir.replace("${DORIS_HOME}", install.to_str().unwrap_or(""));
                        config.log_dir = PathBuf::from(replaced);
                    }
                    _ => {
                        config.log_dir = PathBuf::from(log_dir);
                    }
                }
            }
        }

        port_parser.parse_line(line, config)?;
        path_parser.parse_line(line, config)?;
        common_parser.parse_line(line, config)?;
    }

    Ok(())
}

fn get_env_config_items<'a>(env: Environment) -> Vec<(&'a str, PortParserFn)> {
    match env {
        Environment::BE => {
            vec![
                ("be_port", parse_key_value::<u16>),
                ("brpc_port", parse_key_value::<u16>),
                ("heartbeat_service_port", parse_key_value::<u16>),
                ("webserver_port", parse_key_value::<u16>),
            ]
        }
        Environment::FE => {
            vec![
                ("http_port", parse_key_value::<u16>),
                ("rpc_port", parse_key_value::<u16>),
                ("query_port", parse_key_value::<u16>),
                ("edit_log_port", parse_key_value::<u16>),
                ("cloud_http_port", parse_key_value::<u16>),
            ]
        }
        _ => vec![],
    }
}

fn get_env_path_config_items<'a>(env: Environment) -> Vec<(&'a str, PathParserFn)> {
    match env {
        Environment::FE => {
            vec![("meta_dir", parse_path_key_value)]
        }
        _ => vec![],
    }
}

fn get_common_config_items<'a>() -> Vec<(&'a str, StringParserFn)> {
    vec![
        (PRIORITY_NETWORKS_KEY, parse_key_value::<String>),
        (META_SERVICE_KEY, parse_key_value::<String>),
    ]
}

/// Extract value from a key=value or key = value line
fn extract_value(line: &str) -> Option<String> {
    regex_utils::extract_value_from_line(line)
}

/// Parse PathBuf key-value
fn parse_path_key_value(line: &str, key: &str, value: &mut Option<PathBuf>) -> Result<()> {
    if let Some(val_str) = regex_utils::extract_key_value(line, key) {
        *value = Some(PathBuf::from(val_str));
    }
    Ok(())
}

/// Generic key-value parser
fn parse_key_value<T: FromStr>(line: &str, key: &str, value: &mut Option<T>) -> Result<()> {
    if let Some(parsed_val) =
        regex_utils::extract_key_value(line, key).and_then(|s| s.parse::<T>().ok())
    {
        *value = Some(parsed_val);
    }
    Ok(())
}
