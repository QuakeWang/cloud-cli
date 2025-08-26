use crate::config::Config;
use crate::config_loader;
use crate::error::{self, Result};
use crate::ui::{print_error, print_info, print_success, print_warning};

pub fn handle_tool_execution_error(
    config: &Config,
    error: &error::CliError,
    service_name: &str,
    tool_name: &str,
) -> Result<Option<Config>> {
    print_info("");

    if service_name == "FE"
        && tool_name.contains("routine_load")
        && error.to_string().contains("No Job ID in memory")
    {
        print_warning("Routine Load tool execution failed: No Job ID selected.");
        print_error(&format!("Error: {error}"));

        print_info("");
        print_info("Would you like to:");

        let options = [
            "Go to Get Job ID".to_string(),
            "Return to Routine Load menu".to_string(),
            "Cancel and return to menu".to_string(),
        ];

        let options_ref: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
        let selection = crate::ui::dialogs::select_index("Choose an option", &options_ref)?;

        return match selection {
            0 | 1 => Err(error::CliError::GracefulExit),
            2 => Ok(None),
            _ => Err(error::CliError::InvalidInput(
                "Invalid selection".to_string(),
            )),
        };
    }

    // BE connectivity: provide network-centric guidance instead of config fixes
    if service_name == "BE" && is_be_connectivity_error(error) {
        print_warning("BE connectivity issue detected.");
        print_error(&format!("Error: {error}"));

        let options = ["Retry", "Return to menu"];
        let selection = crate::ui::dialogs::select_index("Choose an option", &options)?;
        return match selection {
            0 => Ok(Some(config.clone())), // retry with same config
            _ => Ok(None),
        };
    }

    // FE profiler script missing: show simple guidance
    if service_name == "FE" && is_fe_profiler_script_missing(tool_name, error) {
        print_warning("FE profiler script missing.");
        print_error(&format!("Error: {error}"));

        let options = ["Return to menu"];
        let _ = crate::ui::dialogs::select_index("Choose an option", &options)?;
        return Ok(None);
    }

    print_warning("Tool execution failed due to configuration issues.");
    print_error(&format!("Error: {error}"));

    print_info("");
    print_info("Would you like to:");
    // Build options conditionally
    let mut labels: Vec<&str> = Vec::new();
    type ActionFn = fn(&Config) -> Result<Option<Config>>;
    let mut actions: Vec<ActionFn> = Vec::new();

    if is_jdk_missing(config, error) {
        labels.push("Fix JDK path and retry");
        actions.push(fix_jdk_path as ActionFn);
    }
    if is_output_dir_invalid(config, error) {
        labels.push("Fix output directory and retry");
        actions.push(fix_output_directory as ActionFn);
    }
    labels.push("Cancel and return to menu");

    let selection = crate::ui::dialogs::select_index("Choose an option", &labels)?;
    if selection < actions.len() {
        actions[selection](config)
    } else {
        Ok(None)
    }
}

fn fix_jdk_path(config: &Config) -> Result<Option<Config>> {
    let new_path: String = crate::ui::dialogs::input_text(
        "Enter the correct JDK path",
        &config.jdk_path.to_string_lossy(),
    )?;

    let new_path = std::path::PathBuf::from(new_path);

    if !new_path.exists() {
        print_error(&format!("Path does not exist: {}", new_path.display()));
        return Ok(None);
    }

    let jmap_path = new_path.join("bin/jmap");
    let jstack_path = new_path.join("bin/jstack");

    if !jmap_path.exists() || !jstack_path.exists() {
        print_error("Required JDK tools (jmap/jstack) not found in the specified path");
        return Ok(None);
    }

    let fixed_config = config.clone().with_jdk_path(new_path);

    if let Err(e) = persist_updated_config(&fixed_config) {
        print_warning(&format!("Failed to persist configuration: {e}"));
    }

    print_success("JDK path updated successfully!");
    Ok(Some(fixed_config))
}

fn fix_output_directory(config: &Config) -> Result<Option<Config>> {
    let new_path: String = crate::ui::dialogs::input_text(
        "Enter the output directory path",
        &config.output_dir.to_string_lossy(),
    )?;

    let new_path = std::path::PathBuf::from(new_path);

    if let Err(e) = std::fs::create_dir_all(&new_path) {
        print_error(&format!("Cannot create directory: {e}"));
        return Ok(None);
    }

    let fixed_config = config.clone().with_output_dir(new_path);

    if let Err(e) = persist_updated_config(&fixed_config) {
        print_warning(&format!("Failed to persist configuration: {e}"));
    }

    print_success("Output directory updated successfully!");
    Ok(Some(fixed_config))
}

fn persist_updated_config(config: &Config) -> Result<()> {
    let mut doris_config = config_loader::load_config()?;
    doris_config = doris_config.with_app_config(config);
    match config_loader::config_persister::persist_config(&doris_config) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

fn is_be_connectivity_error(error: &error::CliError) -> bool {
    let s = error.to_string();
    s.contains("Could not connect to any BE http port")
}

fn is_fe_profiler_script_missing(tool_name: &str, error: &error::CliError) -> bool {
    tool_name.contains("fe-profiler") && error.to_string().contains("profile_fe.sh not found")
}

fn is_jdk_missing(config: &Config, error: &error::CliError) -> bool {
    let s = error.to_string();
    if s.contains("JDK path does not exist") || s.contains("jmap") || s.contains("jstack") {
        return true;
    }
    let jmap = config.jdk_path.join("bin/jmap");
    let jstack = config.jdk_path.join("bin/jstack");
    !jmap.exists() || !jstack.exists()
}

fn is_output_dir_invalid(config: &Config, error: &error::CliError) -> bool {
    let s = error.to_string();
    if s.contains("Cannot create directory") || s.contains("Output dir input failed") {
        return true;
    }
    !config.output_dir.exists()
}
