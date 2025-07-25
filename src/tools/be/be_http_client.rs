use crate::config_loader;
use crate::error::{CliError, Result};
use crate::executor;
use crate::ui;
use std::process::Command;

const BE_DEFAULT_IP: &str = "127.0.0.1";

/// Send an HTTP GET request to a BE API endpoint
pub fn request_be_webserver_port(endpoint: &str, filter_pattern: Option<&str>) -> Result<String> {
    let be_http_ports = get_be_http_ports()?;

    for &port in &be_http_ports {
        let url = format!("http://{BE_DEFAULT_IP}:{port}{endpoint}");
        let mut curl_cmd = Command::new("curl");
        curl_cmd.args(["-sS", &url]);

        if let Ok(output) = executor::execute_command(&mut curl_cmd, "curl") {
            let content = String::from_utf8_lossy(&output.stdout);

            // If a filter pattern is provided, filter the content
            if let Some(pattern) = filter_pattern {
                let filtered_lines: Vec<&str> = content
                    .lines()
                    .filter(|line| line.contains(pattern))
                    .collect();
                return Ok(filtered_lines.join("\n"));
            }

            return Ok(content.to_string());
        }
    }

    let ports_str = be_http_ports
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    Err(CliError::ToolExecutionFailed(format!(
        "Could not connect to any BE http port ({ports_str}). Check if BE is running."
    )))
}

/// Get BE HTTP ports from configuration or use defaults
pub fn get_be_http_ports() -> Result<Vec<u16>> {
    match config_loader::load_config() {
        Ok(doris_config) => Ok(doris_config.get_be_http_ports()),
        Err(_) => {
            // Fallback to default ports if configuration cannot be loaded
            ui::print_warning(
                "Could not load configuration, using default BE HTTP ports (8040, 8041)",
            );
            Ok(vec![8040, 8041])
        }
    }
}
