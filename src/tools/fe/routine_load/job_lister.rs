use super::job_manager::RoutineLoadJobManager;
use super::models::RoutineLoadJob;
use crate::config::Config;
use crate::config_loader;
use crate::error::{CliError, Result};
use crate::tools::common::fs_utils::ensure_dir_exists;
use crate::tools::mysql::MySQLTool;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use crate::ui::{InputHelper, InteractiveSelector};
use crate::ui::{NoJobsNextAction, show_no_jobs_recovery_menu, show_unknown_db_recovery_menu};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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

    fn execute(&self, config: &Config, _pid: u32) -> Result<ExecutionResult> {
        // Retry loop: allow reselecting database if no jobs found
        let mut database = self.prompt_database_name()?;
        loop {
            match self.query_routine_load_jobs(&database) {
                Ok(jobs) => {
                    self.display_jobs(&jobs)?;
                    let selected_job = self.prompt_job_selection(&jobs)?;
                    self.save_selected_job(selected_job, &database)?;
                    let report =
                        self.generate_selection_report(selected_job, &config.output_dir)?;
                    ui::print_info("");
                    ui::print_info(&report);
                    return Ok(ExecutionResult {
                        output_path: config.output_dir.clone(),
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
        ui::print_info("");
        ui::print_info("Routine Load Jobs in Database:");
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

    fn generate_selection_report(
        &self,
        job: &RoutineLoadJob,
        output_dir: &std::path::Path,
    ) -> Result<String> {
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

        if job.lag.is_some() {
            // Partitions Overview: show Top 30 (largest non-zero lag) and Bottom 20 (smallest non-zero lag)
            let rows = self.build_partition_rows(job.progress.as_ref(), job.lag.as_ref());
            let nonzero_count = rows.iter().filter(|(_, _, lag_v)| *lag_v > 0).count();
            let zero_count = rows.len().saturating_sub(nonzero_count);
            if !rows.is_empty() {
                report.push_str("\nPartitions Overview (non-zero lags only):\n");
                report.push_str(&self.format_partitions_overview_nonzero_top_bottom(&rows, 30, 20));
                report.push_str(&format!(
                    "non-zero-lag: {nonzero_count}, zero-lag: {zero_count}, total: {}\n",
                    rows.len()
                ));

                match self.write_full_partitions_file(&rows, &job.id, output_dir) {
                    Ok(path) => {
                        report.push_str(&format!("Full partitions saved to: {}\n", path.display()));
                    }
                    Err(e) => {
                        report.push_str(&format!("Failed to save full partitions file: {}\n", e));
                    }
                }
            }
        }

        report.push_str(&format!(
            "\nSelection Time: {}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        Ok(report)
    }

    fn build_partition_rows(
        &self,
        progress: Option<&HashMap<String, String>>,
        lag: Option<&HashMap<String, i64>>,
    ) -> Vec<(String, Option<String>, i64)> {
        let mut rows: Vec<(String, Option<String>, i64)> = Vec::new();

        // Union of partitions
        let mut keys: Vec<String> = Vec::new();
        if let Some(p) = progress {
            keys.extend(p.keys().cloned());
        }
        if let Some(l) = lag {
            for k in l.keys() {
                if !keys.contains(k) {
                    keys.push(k.clone());
                }
            }
        }

        for part in keys {
            let prog = progress.and_then(|p| p.get(&part).cloned());
            let lag_v = lag.and_then(|l| l.get(&part).copied()).unwrap_or(0);
            rows.push((part, prog, lag_v));
        }

        rows.sort_by(|a, b| b.2.cmp(&a.2));
        rows
    }

    fn format_partitions_overview_nonzero_top_bottom(
        &self,
        rows: &[(String, Option<String>, i64)],
        top_n: usize,
        bottom_n: usize,
    ) -> String {
        // filter non-zero lag rows
        let mut nonzero: Vec<(String, Option<String>, i64)> = rows
            .iter()
            .filter(|(_, _, lag_v)| *lag_v > 0)
            .cloned()
            .collect();
        let total = nonzero.len();
        let mut out = String::new();

        // Top section (largest lag first)
        out.push_str("Top by lag:\n");
        out.push_str("┌─────────────┬─────────────┬─────────────┐\n");
        out.push_str("│  Partition  │   Progress  │     Lag     │\n");
        out.push_str("├─────────────┼─────────────┼─────────────┤\n");
        // sort desc
        nonzero.sort_by(|a, b| b.2.cmp(&a.2));
        let mut printed = 0usize;
        for (part, prog, lag_v) in nonzero.iter().take(top_n) {
            let prog_s = prog.as_deref().unwrap_or("N/A");
            out.push_str(&format!("│ {part:>11} │ {prog_s:>11} │ {lag_v:>11} │\n"));
            printed += 1;
        }
        if printed == 0 {
            out.push_str("│               (no data)               │\n");
        }
        out.push_str("└─────────────┴─────────────┴─────────────┘\n");

        // Bottom section (smallest lag last); avoid overlap with top
        out.push_str("Bottom by lag:\n");
        out.push_str("┌─────────────┬─────────────┬─────────────┐\n");
        out.push_str("│  Partition  │   Progress  │     Lag     │\n");
        out.push_str("├─────────────┼─────────────┼─────────────┤\n");
        // sort asc
        nonzero.sort_by(|a, b| a.2.cmp(&b.2));
        let start = 0usize; // beginning for smallest
        let end = bottom_n.min(total);
        for (part, prog, lag_v) in nonzero.iter().skip(start).take(end) {
            let prog_s = prog.as_deref().unwrap_or("N/A");
            out.push_str(&format!("│ {part:>11} │ {prog_s:>11} │ {lag_v:>11} │\n"));
        }
        out.push_str("└─────────────┴─────────────┴─────────────┘\n");

        out
    }

    fn write_full_partitions_file(
        &self,
        rows: &[(String, Option<String>, i64)],
        job_id: &str,
        base_dir: &std::path::Path,
    ) -> Result<PathBuf> {
        let file_path = base_dir.join(format!("routine_load_partitions_{job_id}.txt"));
        ensure_dir_exists(&file_path)?;

        let mut content = String::from("Partition\tProgress\tLag\n");
        for (part, prog, lag_v) in rows {
            let prog_s = prog.as_deref().unwrap_or("N/A");
            content.push_str(&format!("{part}\t{prog_s}\t{lag_v}\n"));
        }
        fs::write(&file_path, content)
            .map_err(|e| CliError::ToolExecutionFailed(format!("Write failed: {e}")))?;

        Ok(file_path)
    }
}
