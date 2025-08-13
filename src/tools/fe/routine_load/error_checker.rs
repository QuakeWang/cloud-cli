use super::job_manager::RoutineLoadJobManager;
use crate::config::Config;
use crate::config_loader;
use crate::error::{CliError, Result};
use crate::tools::mysql::MySQLTool;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;

pub struct RoutineLoadErrorChecker;

impl Tool for RoutineLoadErrorChecker {
    fn name(&self) -> &str {
        "routine_load_error_checker"
    }
    fn description(&self) -> &str {
        "Check for errors in Routine Load job"
    }
    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let job_manager = RoutineLoadJobManager;
        let job_id = job_manager.get_current_job_id().ok_or_else(|| {
            CliError::InvalidInput("No Job ID in memory. Run 'Get Job ID' first.".into())
        })?;
        let database = job_manager
            .get_last_database()
            .ok_or_else(|| CliError::InvalidInput("Unknown database for current Job ID".into()))?;

        ui::print_info(&format!(
            "Checking Routine Load errors for job {}...",
            job_id
        ));

        let doris_config = config_loader::load_config()?;
        let sql = format!("USE `{}`; SHOW ROUTINE LOAD \\G", database);
        let output = MySQLTool::query_sql_with_config(&doris_config, &sql)?;

        let jobs = job_manager.parse_routine_load_output(&output)?;
        let job = jobs.into_iter().find(|j| j.id == job_id).ok_or_else(|| {
            CliError::InvalidInput(format!("Job {} not found in database {}", job_id, database))
        })?;

        let mut findings: Vec<String> = Vec::new();

        if job.state != "RUNNING" && job.state != "NEED_SCHEDULE" {
            findings.push(format!("State is {}", job.state));
        }

        if let Some(stat) = &job.statistic {
            if stat.error_rows > 0 {
                findings.push(format!("Error rows: {}", stat.error_rows));
            }
            if stat.unselected_rows > 0 {
                findings.push(format!("Unselected rows: {}", stat.unselected_rows));
            }
        }

        if let Some(urls) = &job.error_log_urls {
            if !urls.trim().is_empty() && urls.trim() != "NULL" {
                findings.push(format!("Error log URLs: {}", urls.trim()));
            }
        }

        println!("\nRoutine Load Error Check Report\n================================\n");
        println!("Job ID: {}", job.id);
        println!("Name  : {}", job.name);
        println!("State : {}", job.state);
        println!("DB    : {}", job.db_name);
        println!("Table : {}", job.table_name);

        if findings.is_empty() {
            println!("\nNo obvious errors detected.");
        } else {
            println!("\nFindings:");
            for f in &findings {
                println!("  - {}", f);
            }
        }

        println!("\nHints:");
        println!("  - Check FE logs if state is PAUSED/STOPPED/CANCELLED");
        println!("  - Review ErrorLogUrls if present");
        println!("  - Verify source offsets and Lag for Kafka");

        Ok(ExecutionResult {
            output_path: std::path::PathBuf::from("console_output"),
            message: format!("Error check completed for Job ID: {}", job_id),
        })
    }
}
