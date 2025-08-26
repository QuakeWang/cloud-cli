use crate::error::Result;

/// Collect cluster info asynchronously in the background
pub fn spawn_cluster_info_collector(
    doris_config: crate::config_loader::DorisConfig,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if should_update_cluster_info() {
            collect_cluster_info_with_retry(&doris_config);
        }
    })
}

/// Collect cluster info with retry mechanism and timeout
pub fn collect_cluster_info_with_retry(doris_config: &crate::config_loader::DorisConfig) {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_SECS: u64 = 2;
    const TIMEOUT_SECS: u64 = 30;

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(TIMEOUT_SECS);
    let mut retry_count = 0;

    while retry_count < MAX_RETRIES && start.elapsed() < timeout {
        match collect_cluster_info_background(doris_config) {
            Ok(_) => break,
            Err(e) => {
                retry_count += 1;

                if let crate::error::CliError::MySQLAccessDenied(_) = e {
                    break;
                }
                if let crate::error::CliError::ConfigError(_) = e {
                    break;
                }

                if retry_count >= MAX_RETRIES || start.elapsed() >= timeout {
                    if std::env::var("CLOUD_CLI_DEBUG").is_ok() {
                        eprintln!(
                            "Background cluster info collection failed after {retry_count} attempts: {e}"
                        );
                    }
                    break;
                } else {
                    std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS));
                }
            }
        }
    }
}

/// Check if cluster info needs to be updated
pub fn should_update_cluster_info() -> bool {
    let clusters_file = match dirs::home_dir() {
        Some(home) => home.join(".config").join("cloud-cli").join("clusters.toml"),
        None => return true,
    };

    if !clusters_file.exists() {
        return true;
    }

    let metadata = match std::fs::metadata(&clusters_file) {
        Ok(m) => m,
        Err(_) => return true,
    };

    if metadata.len() < 100 {
        return true;
    }

    let modified = match metadata.modified() {
        Ok(m) => m,
        Err(_) => return true,
    };

    let duration = match std::time::SystemTime::now().duration_since(modified) {
        Ok(d) => d,
        Err(_) => return true,
    };

    duration.as_secs() > 300
}

/// Implementation for collecting cluster info in the background
pub fn collect_cluster_info_background(
    doris_config: &crate::config_loader::DorisConfig,
) -> Result<()> {
    if doris_config.mysql.is_none() {
        return Ok(());
    }
    let mysql_tool = crate::tools::mysql::MySQLTool;
    let cluster_info = mysql_tool.query_cluster_info(doris_config)?;
    cluster_info.save_to_file()?;
    Ok(())
}
