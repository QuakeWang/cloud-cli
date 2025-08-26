use chrono::Duration;
use std::collections::BTreeMap;

use super::job_manager::RoutineLoadJobManager;
use super::log_parser::{FeLogParser, LogCommitEntry, scan_file};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::common::fs_utils;
use crate::tools::fe::routine_load::messages as ErrMsg;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use crate::ui::InputHelper;

pub struct RoutineLoadTrafficMonitor;

impl Tool for RoutineLoadTrafficMonitor {
    fn name(&self) -> &str {
        "routine_load_traffic_monitor"
    }
    fn description(&self) -> &str {
        "Aggregate per-minute loadedRows from FE logs"
    }
    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let job_id = self.get_job_id()?;
        let log_dir = self.get_log_directory()?;

        let minutes = self.prompt_time_window()?;

        ui::print_info(&format!(
            "Analyzing traffic in {log_dir} for job {job_id} (last {minutes} min)...",
            log_dir = log_dir.display(),
            job_id = job_id,
            minutes = minutes
        ));

        let entries = self.collect_and_parse_logs(&log_dir, &job_id)?;

        let filtered_entries = self.filter_entries_by_time_window(entries, minutes)?;

        let per_minute_data = self.aggregate_per_minute(filtered_entries);

        self.display_traffic_results(&per_minute_data)?;

        Ok(ExecutionResult {
            output_path: std::path::PathBuf::from("console_output"),
            message: "Traffic monitor completed".into(),
        })
    }
}

impl RoutineLoadTrafficMonitor {
    fn get_job_id(&self) -> Result<String> {
        let job_manager = RoutineLoadJobManager;
        job_manager
            .get_current_job_id()
            .ok_or_else(|| CliError::InvalidInput(ErrMsg::NO_JOB_ID.into()))
    }

    fn get_log_directory(&self) -> Result<std::path::PathBuf> {
        let doris = crate::config_loader::load_config()?;
        Ok(doris.log_dir)
    }

    fn prompt_time_window(&self) -> Result<i64> {
        InputHelper::prompt_number_with_default("Analyze recent minutes", 60, 1)
    }

    fn collect_and_parse_logs(
        &self,
        log_dir: &std::path::Path,
        job_id: &str,
    ) -> Result<Vec<LogCommitEntry>> {
        let files = fs_utils::collect_fe_logs(log_dir)?;
        let parser = FeLogParser::new();
        let mut entries: Vec<LogCommitEntry> = Vec::new();

        for path in files {
            scan_file(&parser, &path, job_id, &mut entries)?;
        }

        if entries.is_empty() {
            return Err(CliError::ToolExecutionFailed(
                "No matching entries found".into(),
            ));
        }

        Ok(entries)
    }

    fn filter_entries_by_time_window(
        &self,
        mut entries: Vec<LogCommitEntry>,
        minutes: i64,
    ) -> Result<Vec<LogCommitEntry>> {
        let latest_ts = entries.iter().map(|e| e.timestamp).max().unwrap();
        let window_start = latest_ts - Duration::minutes(minutes);
        entries.retain(|e| e.timestamp >= window_start);

        if entries.is_empty() {
            return Err(CliError::ToolExecutionFailed(
                "No entries in selected window".into(),
            ));
        }

        Ok(entries)
    }

    fn aggregate_per_minute(&self, entries: Vec<LogCommitEntry>) -> BTreeMap<String, u128> {
        let mut per_minute: BTreeMap<String, u128> = BTreeMap::new();

        for entry in entries {
            let rows = entry.loaded_rows.unwrap_or(0) as u128;
            let key = entry.timestamp.format("%H:%M").to_string();
            *per_minute.entry(key).or_insert(0) += rows;
        }

        per_minute
    }

    fn display_traffic_results(&self, per_minute_data: &BTreeMap<String, u128>) -> Result<()> {
        ui::print_info("");
        ui::print_info("Per-minute loadedRows (ascending time)");
        ui::print_info(&"-".repeat(40));

        for (minute, rows) in per_minute_data.iter() {
            ui::print_info(&format!("{minute} loadedRows={rows}"));
        }

        let total_rows: u128 = per_minute_data.values().sum();
        ui::print_info(&"-".repeat(40));
        ui::print_info(&format!(
            "Total minutes: {count}",
            count = per_minute_data.len()
        ));
        ui::print_info(&format!("Total loadedRows: {total_rows}"));

        let avg_rows = if !per_minute_data.is_empty() {
            total_rows / per_minute_data.len() as u128
        } else {
            0
        };
        ui::print_info(&format!("Average per minute: {avg_rows}"));

        Ok(())
    }
}
