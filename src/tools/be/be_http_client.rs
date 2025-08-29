use crate::config_loader;
use crate::error::{CliError, Result};
use crate::executor;
use crate::tools::{be, mysql};
use crate::ui;
use std::collections::BTreeSet;
use std::process::Command;

const BE_DEFAULT_IP: &str = "127.0.0.1";

/// Send an HTTP GET request to a BE API endpoint
pub fn request_be_webserver_port(endpoint: &str, filter_pattern: Option<&str>) -> Result<String> {
    let mut be_targets: BTreeSet<(String, u16)> = BTreeSet::new();

    let ports = get_be_http_ports()?;

    let selected_host = be::list::get_selected_be_host();

    let cluster_hosts = get_be_ip().unwrap_or_default();

    let mut all_hosts = BTreeSet::new();
    if let Some(host) = &selected_host {
        all_hosts.insert(host.clone());
    }
    for host in cluster_hosts {
        all_hosts.insert(host);
    }

    if all_hosts.is_empty() {
        all_hosts.insert(BE_DEFAULT_IP.to_string());
    }

    for host in all_hosts {
        be_targets.extend(ports.iter().map(|p| (host.clone(), *p)));
    }

    for (host, port) in &be_targets {
        let url = format!("http://{host}:{port}{endpoint}");
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

    let ports_str = be_targets
        .iter()
        .map(|(h, p)| format!("{h}:{p}"))
        .collect::<Vec<_>>()
        .join(", ");

    ui::print_warning(
        "Could not connect to any BE http endpoint. You can select a host via 'be-list'.",
    );
    Err(CliError::ToolExecutionFailed(format!(
        "Could not connect to any BE http port ({ports_str}). Check if BE is running."
    )))
}

/// Get BE HTTP ports from configuration or use defaults
pub fn get_be_http_ports() -> Result<Vec<u16>> {
    if let Ok(doris_config) = config_loader::load_config() {
        let config_ports = doris_config.get_be_http_ports();
        if !config_ports.is_empty() && config_ports != vec![8040, 8041] {
            return Ok(config_ports);
        }
    }

    if let Ok(info) = mysql::ClusterInfo::load_from_file() {
        let be_ports: Vec<u16> = info
            .backends
            .iter()
            .filter(|b| b.alive)
            .map(|b| b.http_port)
            .collect();

        if !be_ports.is_empty() {
            return Ok(be_ports);
        }
    }

    Ok(vec![8040, 8041])
}

pub fn get_be_ip() -> Result<Vec<String>> {
    if let Ok(info) = mysql::ClusterInfo::load_from_file() {
        let hosts = info.list_be_hosts();
        if !hosts.is_empty() {
            return Ok(hosts);
        }
    }

    Ok(vec![BE_DEFAULT_IP.to_string()])
}
