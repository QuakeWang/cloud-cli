use chrono::Duration;
use std::collections::HashMap;

use super::job_manager::RoutineLoadJobManager;
use super::log_parser::{FeLogParser, LogCommitEntry, scan_file};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::common::fs_utils;
use crate::tools::fe::routine_load::messages as ErrMsg;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use crate::ui::{FormatHelper, InputHelper};

pub struct RoutineLoadPerformanceAnalyzer;

impl Tool for RoutineLoadPerformanceAnalyzer {
    fn name(&self) -> &str {
        "routine_load_performance_analyzer"
    }
    fn description(&self) -> &str {
        "Analyze per-commit rows/bytes/time from FE logs"
    }
    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let job_manager = RoutineLoadJobManager;
        let job_id = job_manager
            .get_current_job_id()
            .ok_or_else(|| CliError::InvalidInput(ErrMsg::NO_JOB_ID.into()))?;

        let doris = crate::config_loader::load_config()?;
        let log_dir = doris.log_dir;

        let minutes = self.prompt_time_window()?;

        ui::print_info(&format!(
            "Analyzing FE logs in {} for job {} (last {} min)...",
            log_dir.display(),
            job_id,
            minutes
        ));

        let entries = self.collect_and_parse_logs(&log_dir, &job_id)?;

        let filtered_entries = self.filter_entries_by_time_window(entries, minutes)?;

        let deduplicated_entries = self.deduplicate_entries(filtered_entries)?;

        self.display_performance_results(&deduplicated_entries)?;

        Ok(ExecutionResult {
            output_path: std::path::PathBuf::from("console_output"),
            message: "Performance analysis completed".into(),
        })
    }
}

impl RoutineLoadPerformanceAnalyzer {
    fn prompt_time_window(&self) -> Result<i64> {
        InputHelper::prompt_number_with_default("Analyze recent minutes", 30, 1)
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
                "No matching commit entries found in FE logs".into(),
            ));
        }

        Ok(entries)
    }

    fn filter_entries_by_time_window(
        &self,
        mut entries: Vec<LogCommitEntry>,
        minutes: i64,
    ) -> Result<Vec<LogCommitEntry>> {
        // Use latest timestamp from logs as reference to avoid timezone/clock inconsistencies
        let latest_ts = entries.iter().map(|e| e.timestamp).max().unwrap();
        let window_start = latest_ts - Duration::minutes(minutes);
        entries.retain(|e| e.timestamp >= window_start);

        if entries.is_empty() {
            return Err(CliError::ToolExecutionFailed(
                "No matching commit entries found in FE logs".into(),
            ));
        }

        Ok(entries)
    }

    fn deduplicate_entries(&self, entries: Vec<LogCommitEntry>) -> Result<Vec<LogCommitEntry>> {
        let mut map: HashMap<String, LogCommitEntry> = HashMap::new();

        for e in entries.into_iter() {
            let key = format!(
                "ts:{}|r:{}|b:{}|ms:{}",
                e.timestamp,
                e.loaded_rows.unwrap_or(0),
                e.received_bytes.unwrap_or(0),
                e.task_execution_ms.unwrap_or(0)
            );

            match map.get_mut(&key) {
                Some(existing) => {
                    if existing.transaction_id.is_none() && e.transaction_id.is_some() {
                        *existing = e;
                    }
                }
                None => {
                    map.insert(key, e);
                }
            }
        }

        let deduped: Vec<LogCommitEntry> = map.into_values().collect();
        if deduped.is_empty() {
            return Err(CliError::ToolExecutionFailed(
                "No matching commit entries found in FE logs".into(),
            ));
        }
        Ok(deduped)
    }

    fn display_performance_results(&self, entries: &[LogCommitEntry]) -> Result<()> {
        // Collect rows
        let mut sorted_entries = entries.to_vec();
        sorted_entries.sort_by_key(|e| e.timestamp);

        let headers = ["Time", "ms", "loadedRows", "receivedBytes", "txnId"];

        let mut rows: Vec<[String; 5]> = Vec::with_capacity(sorted_entries.len());
        let mut stats = PerformanceStats::new();

        for entry in &sorted_entries {
            let time_str = entry.timestamp.format("%H:%M:%S").to_string();
            let ms = entry.task_execution_ms.unwrap_or(0);
            let rows_val = entry.loaded_rows.unwrap_or(0);
            let bytes_val = entry.received_bytes.unwrap_or(0);
            let txn = entry.transaction_id.clone().unwrap_or_else(|| "-".into());

            rows.push([
                time_str,
                ms.to_string(),
                FormatHelper::fmt_int(rows_val),
                FormatHelper::fmt_int(bytes_val),
                txn,
            ]);
            stats.update(entry);
        }

        // Compute column widths
        let mut widths = [0usize; 5];
        for i in 0..5 {
            widths[i] = headers[i].len();
        }
        for row in &rows {
            for i in 0..5 {
                widths[i] = widths[i].max(row[i].len());
            }
        }

        // Render table
        ui::print_info("\nPer-commit stats");
        self.print_table(&headers, &rows, &widths)?;

        // Summary
        stats.display_summary();
        Ok(())
    }

    fn print_table(
        &self,
        headers: &[&str; 5],
        rows: &[[String; 5]],
        widths: &[usize; 5],
    ) -> Result<()> {
        // Separator line
        let sep = {
            let mut s = String::new();
            for (idx, w) in widths.iter().enumerate() {
                if idx > 0 {
                    s.push('+');
                }
                s.push_str(&"-".repeat(*w + 2));
            }
            s
        };

        // Header
        ui::print_info(&sep);
        let header_line = format!(
            " {:<w0$} | {:>w1$} | {:>w2$} | {:>w3$} | {:<w4$}",
            headers[0],
            headers[1],
            headers[2],
            headers[3],
            headers[4],
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2],
            w3 = widths[3],
            w4 = widths[4]
        );
        ui::print_info(&header_line);
        ui::print_info(&sep);

        // Rows
        for row in rows {
            let line = format!(
                " {:<w0$} | {:>w1$} | {:>w2$} | {:>w3$} | {:<w4$}",
                row[0],
                row[1],
                row[2],
                row[3],
                row[4],
                w0 = widths[0],
                w1 = widths[1],
                w2 = widths[2],
                w3 = widths[3],
                w4 = widths[4]
            );
            ui::print_info(&line);
        }
        ui::print_info(&sep);

        Ok(())
    }
}

/// Performance statistics information
struct PerformanceStats {
    count: u64,
    sum_ms: u128,
    min_ms: u64,
    max_ms: u64,
    sum_rows: u128,
    min_rows: u64,
    max_rows: u64,
    sum_bytes: u128,
    min_bytes: u64,
    max_bytes: u64,
}

impl PerformanceStats {
    fn new() -> Self {
        Self {
            count: 0,
            sum_ms: 0,
            min_ms: u64::MAX,
            max_ms: 0,
            sum_rows: 0,
            min_rows: u64::MAX,
            max_rows: 0,
            sum_bytes: 0,
            min_bytes: u64::MAX,
            max_bytes: 0,
        }
    }

    fn update(&mut self, entry: &LogCommitEntry) {
        let ms = entry.task_execution_ms.unwrap_or(0);
        let rows = entry.loaded_rows.unwrap_or(0);
        let bytes = entry.received_bytes.unwrap_or(0);

        self.count += 1;
        self.sum_ms += ms as u128;
        self.min_ms = self.min_ms.min(ms);
        self.max_ms = self.max_ms.max(ms);
        self.sum_rows += rows as u128;
        self.min_rows = self.min_rows.min(rows);
        self.max_rows = self.max_rows.max(rows);
        self.sum_bytes += bytes as u128;
        self.min_bytes = self.min_bytes.min(bytes);
        self.max_bytes = self.max_bytes.max(bytes);
    }

    fn display_summary(&self) {
        if self.count > 0 {
            ui::print_info(&format!(
                "count={}  avg_ms={}  min_ms={}  max_ms={}",
                self.count,
                self.sum_ms / self.count as u128,
                if self.min_ms == u64::MAX {
                    0
                } else {
                    self.min_ms
                },
                self.max_ms
            ));
            ui::print_info(&format!(
                "          avg_rows={}  min_rows={}  max_rows={}",
                FormatHelper::fmt_int_u128(self.sum_rows / self.count as u128),
                FormatHelper::fmt_int(if self.min_rows == u64::MAX {
                    0
                } else {
                    self.min_rows
                }),
                FormatHelper::fmt_int(self.max_rows)
            ));
            ui::print_info(&format!(
                "          avg_bytes={}  min_bytes={}  max_bytes={}",
                FormatHelper::fmt_int_u128(self.sum_bytes / self.count as u128),
                FormatHelper::fmt_int(if self.min_bytes == u64::MAX {
                    0
                } else {
                    self.min_bytes
                }),
                FormatHelper::fmt_int(self.max_bytes)
            ));
        }
    }
}
