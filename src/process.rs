use crate::config_loader;
use crate::error::{CliError, Result};

/// Select process interactively when config PID is not available
pub fn select_process_interactively() -> Result<u32> {
    // This is only called when config file PID is invalid
    // Use the enhanced process detector for discovery
    match config_loader::process_detector::detect_current_process() {
        Ok(result) => {
            crate::ui::print_info(&format!(
                "Found {} process",
                if result.environment == config_loader::Environment::FE {
                    "FE"
                } else {
                    "BE"
                }
            ));
            crate::ui::print_process_info(result.pid, &result.command);
            Ok(result.pid)
        }
        Err(_) => Err(CliError::ProcessNotFound(
            "No Doris process found".to_string(),
        )),
    }
}
