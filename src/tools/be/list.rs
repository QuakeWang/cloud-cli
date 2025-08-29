use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::{ExecutionResult, Tool};
use crate::ui;

pub use crate::tools::common::host_selection::{
    get_selected_host as get_selected_be_host_generic,
    set_selected_host as set_selected_be_host_generic,
};
pub fn set_selected_be_host(host: String) {
    set_selected_be_host_generic(true, host);
}
pub fn get_selected_be_host() -> Option<String> {
    get_selected_be_host_generic(true)
}

pub struct BeListTool;

impl Tool for BeListTool {
    fn name(&self) -> &str {
        "be-list"
    }

    fn description(&self) -> &str {
        "List and select a BE host (IP) for this session"
    }

    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<crate::tools::ExecutionResult> {
        let info = crate::tools::mysql::ClusterInfo::load_from_file()?;
        let hosts = info.list_be_hosts();
        if hosts.is_empty() {
            return Err(CliError::ConfigError(
                "No BE hosts found in clusters.toml".to_string(),
            ));
        }

        let items: Vec<String> = hosts;

        let selection = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select Backend (BE) host")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| CliError::InvalidInput(format!("BE selection failed: {e}")))?;

        let host = items[selection].clone();
        set_selected_be_host(host.clone());
        ui::print_success(&format!("Selected BE host: {host}"));

        Ok(ExecutionResult {
            output_path: std::path::PathBuf::from("console_output"),
            message: "BE host updated for this session".to_string(),
        })
    }
}
