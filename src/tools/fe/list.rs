use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::Tool;
use crate::ui;
use std::collections::BTreeSet;

pub struct FeListTool;

impl Tool for FeListTool {
    fn name(&self) -> &str {
        "fe-list"
    }

    fn description(&self) -> &str {
        "List and select a FE host (IP) for this session"
    }

    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<crate::tools::ExecutionResult> {
        let info = crate::tools::mysql::ClusterInfo::load_from_file()?;
        let mut hosts: BTreeSet<String> = BTreeSet::new();
        for fe in info.frontends.iter().filter(|f| f.alive) {
            if !fe.host.is_empty() {
                hosts.insert(fe.host.clone());
            }
        }
        if hosts.is_empty() {
            return Err(CliError::ConfigError(
                "No FE hosts found in clusters.toml".to_string(),
            ));
        }
        let items: Vec<String> = hosts.iter().cloned().collect();

        let selection = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select Frontend (FE) host")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| CliError::InvalidInput(format!("FE selection failed: {e}")))?;

        let host = items[selection].clone();
        crate::tools::common::host_selection::set_selected_host(false, host.clone());
        ui::print_success(&format!("Selected FE host: {host}"));

        Ok(crate::tools::ExecutionResult {
            output_path: std::path::PathBuf::from("console_output"),
            message: "FE target updated for this session".to_string(),
        })
    }
}
