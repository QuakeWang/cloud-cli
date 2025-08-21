use super::job_manager::RoutineLoadJobManager;
use super::models::RoutineLoadJob;
use crate::config::Config;
use crate::config_loader;
use crate::error::{CliError, Result};
use crate::tools::mysql::MySQLTool;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use crate::ui::{InputHelper, InteractiveSelector};
use crate::ui::{NoJobsNextAction, show_no_jobs_recovery_menu, show_unknown_db_recovery_menu};
use chrono::Utc;

/// Routine Load Job Lister
pub struct RoutineLoadJobLister;

impl Tool for RoutineLoadJobLister {
    fn name(&self) -> &str {
        "routine_load_job_lister"
    }

    fn description(&self) -> &str {
        "List and select Routine Load jobs"
    }

    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        // Retry loop: allow reselecting database if no jobs found
        let mut database = self.prompt_database_name()?;
        loop {
            match self.query_routine_load_jobs(&database) {
                Ok(jobs) => {
                    self.display_jobs(&jobs)?;
                    let selected_job = self.prompt_job_selection(&jobs)?;
                    self.save_selected_job(selected_job, &database)?;
                    let report = self.generate_selection_report(selected_job)?;
                    ui::print_info(&format!("\n{report}"));
                    return Ok(ExecutionResult {
                        output_path: std::path::PathBuf::from("console_output"),
                        message: format!(
                            "Job ID '{}' selected and saved in memory",
                            selected_job.id
                        ),
                    });
                }
                Err(CliError::ToolExecutionFailed(msg))
                    if msg.contains("No Routine Load jobs found in database") =>
                {
                    match show_no_jobs_recovery_menu(&database)? {
                        NoJobsNextAction::ChooseAnotherDatabase => {
                            database = self.prompt_database_name()?;
                        }
                        NoJobsNextAction::BackToMenu => return Err(CliError::GracefulExit),
                    }
                }
                Err(CliError::ToolExecutionFailed(msg)) if msg.contains("Unknown database") => {
                    match show_unknown_db_recovery_menu(&database)? {
                        NoJobsNextAction::ChooseAnotherDatabase => {
                            database = self.prompt_database_name()?;
                        }
                        NoJobsNextAction::BackToMenu => return Err(CliError::GracefulExit),
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl RoutineLoadJobLister {
    fn prompt_database_name(&self) -> Result<String> {
        let doris_config = config_loader::load_config()?;
        match MySQLTool::list_databases(&doris_config) {
            Ok(output) => {
                let dbs = output;

                if !dbs.is_empty() {
                    ui::print_info("Select a database:");
                    let selector =
                        InteractiveSelector::new(dbs.clone(), "Available databases:".to_string())
                            .with_page_size(30);
                    if let Ok(selected) = selector.select() {
                        return Ok(selected.clone());
                    }
                }
            }
            Err(_) => {
                // Fallback to manual input
            }
        }

        ui::print_info("Please enter the database name:");
        InputHelper::prompt_non_empty("Database name")
    }

    fn query_routine_load_jobs(&self, database: &str) -> Result<Vec<RoutineLoadJob>> {
        let doris_config = config_loader::load_config()?;

        let sql = format!("USE `{}`; SHOW ALL ROUTINE LOAD \\G", database);
        let output = MySQLTool::query_sql_with_config(&doris_config, &sql)?;

        let job_manager = RoutineLoadJobManager;
        let jobs = job_manager.parse_routine_load_output(&output)?;

        if jobs.is_empty() {
            return Err(CliError::ToolExecutionFailed(format!(
                "No Routine Load jobs found in database '{database}'"
            )));
        }

        Ok(jobs)
    }

    fn display_jobs(&self, jobs: &[RoutineLoadJob]) -> Result<()> {
        ui::print_info("\nRoutine Load Jobs in Database:");
        ui::print_info(&"=".repeat(100));

        for job in jobs.iter() {
            ui::print_info(&format!(
                "ID: {} | Name: {} | State: {} | CreateTime: {}",
                job.id, job.name, job.state, job.create_time
            ));
        }

        ui::print_info(&"-".repeat(100));
        ui::print_info(&format!("Total jobs found: {count}", count = jobs.len()));
        ui::print_info(&"=".repeat(100));

        let running_count = jobs.iter().filter(|j| j.state == "RUNNING").count();
        let paused_count = jobs.iter().filter(|j| j.state == "PAUSED").count();
        let stopped_count = jobs.iter().filter(|j| j.state == "STOPPED").count();

        println!(
            "Summary: {} total jobs ({running_count} running, {paused_count} paused, {stopped_count} stopped)",
            jobs.len()
        );

        Ok(())
    }

    fn prompt_job_selection<'a>(&self, jobs: &'a [RoutineLoadJob]) -> Result<&'a RoutineLoadJob> {
        let selector =
            InteractiveSelector::new(jobs.to_vec(), "Select a Routine Load job:".to_string());
        let selected_job = selector.select()?;

        jobs.iter()
            .find(|j| j.id == selected_job.id)
            .ok_or_else(|| CliError::InvalidInput("Selected job not found in original list".into()))
    }

    fn save_selected_job(&self, job: &RoutineLoadJob, database: &str) -> Result<()> {
        let job_manager = RoutineLoadJobManager;

        job_manager.save_job_id(job.id.clone(), job.name.clone(), database.to_string())?;

        job_manager.update_job_cache(vec![job.clone()])?;

        ui::print_success(&format!("Job ID '{}' saved in memory", job.id));

        Ok(())
    }

    fn generate_selection_report(&self, job: &RoutineLoadJob) -> Result<String> {
        let mut report = String::new();
        report.push_str("Routine Load Job Selection Report\n");
        report.push_str("=================================\n\n");
        report.push_str(&format!("Selected Job ID: {}\n", job.id));
        report.push_str(&format!("Job Name: {}\n", job.name));
        report.push_str(&format!("State: {}\n", job.state));
        report.push_str(&format!("Database: {}\n", job.db_name));
        report.push_str(&format!("Table: {}\n", job.table_name));
        report.push_str(&format!("Create Time: {}\n", job.create_time));

        if let Some(ref pause_time) = job.pause_time {
            report.push_str(&format!("Pause Time: {}\n", pause_time));
        }

        if let Some(ref stat) = job.statistic {
            report.push_str("\nStatistics:\n");
            report.push_str(&format!("  Loaded Rows: {}\n", stat.loaded_rows));
            report.push_str(&format!("  Error Rows: {}\n", stat.error_rows));
            report.push_str(&format!("  Received Bytes: {}\n", stat.received_bytes));
        }

        if let Some(ref progress) = job.progress {
            report.push_str("\nProgress:\n");
            for (partition, offset) in progress {
                report.push_str(&format!("  Partition {partition}: {offset}\n"));
            }
        }

        if let Some(ref lag) = job.lag {
            report.push_str("\nLag:\n");
            for (partition, lag_value) in lag {
                report.push_str(&format!("  Partition {partition}: {lag_value}\n"));
            }
        }

        report.push_str(&format!(
            "\nSelection Time: {}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        Ok(report)
    }
}
