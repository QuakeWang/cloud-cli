use dialoguer::Select;

use crate::error::{CliError, Result};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoJobsNextAction {
    ChooseAnotherDatabase,
    BackToMenu,
}

pub fn show_no_jobs_recovery_menu(database: &str) -> Result<NoJobsNextAction> {
    ui::print_warning(&format!(
        "\n[!] No Routine Load jobs found in database '{database}'"
    ));
    ui::print_info("This could mean:");
    ui::print_info("  - The database name is incorrect");
    ui::print_info("  - No Routine Load jobs have been created");

    let options = vec!["Choose another database", "Back to Routine Load menu"];

    let selection = Select::new()
        .with_prompt("What would you like to do?")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| CliError::InvalidInput(e.to_string()))?;

    let action = match selection {
        0 => NoJobsNextAction::ChooseAnotherDatabase,
        _ => NoJobsNextAction::BackToMenu,
    };

    Ok(action)
}

pub fn show_unknown_db_recovery_menu(database: &str) -> Result<NoJobsNextAction> {
    ui::print_warning(&format!("\n[!] Unknown database '{database}'"));
    ui::print_info("Please verify the database name or choose another one.");

    let options = vec!["Choose another database", "Back to Routine Load menu"];

    let selection = Select::new()
        .with_prompt("What would you like to do?")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| CliError::InvalidInput(e.to_string()))?;

    let action = match selection {
        0 => NoJobsNextAction::ChooseAnotherDatabase,
        _ => NoJobsNextAction::BackToMenu,
    };

    Ok(action)
}
