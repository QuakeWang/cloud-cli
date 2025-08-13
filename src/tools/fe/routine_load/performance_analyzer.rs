use super::job_manager::RoutineLoadJobManager;
use super::log_parser::{FeLogParser, LogCommitEntry, collect_fe_logs, scan_file};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use chrono::Duration;
use std::collections::HashSet;

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
        let job_id = job_manager.get_current_job_id().ok_or_else(|| {
            CliError::InvalidInput("No Job ID in memory. Run 'Get Job ID' first.".into())
        })?;

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
        let minutes_str: String = dialoguer::Input::new()
            .with_prompt("Analyze recent minutes")
            .default("30".to_string())
            .interact_text()
            .map_err(|e| CliError::InvalidInput(e.to_string()))?;

        let minutes: i64 = minutes_str.trim().parse().unwrap_or(30).max(1);
        Ok(minutes)
    }

    fn collect_and_parse_logs(
        &self,
        log_dir: &std::path::Path,
        job_id: &str,
    ) -> Result<Vec<LogCommitEntry>> {
        let files = collect_fe_logs(log_dir)?;
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

    /// Deduplicate entries
    fn deduplicate_entries(&self, mut entries: Vec<LogCommitEntry>) -> Result<Vec<LogCommitEntry>> {
        let mut seen: HashSet<String> = HashSet::new();
        entries.retain(|e| {
            let key = if let Some(txn) = &e.transaction_id {
                format!("txn:{}", txn)
            } else {
                format!(
                    "ts:{}|r:{}|b:{}|ms:{}",
                    e.timestamp,
                    e.loaded_rows.unwrap_or(0),
                    e.received_bytes.unwrap_or(0),
                    e.task_execution_ms.unwrap_or(0)
                )
            };
            seen.insert(key)
        });

        if entries.is_empty() {
            return Err(CliError::ToolExecutionFailed(
                "No matching commit entries found in FE logs".into(),
            ));
        }

        Ok(entries)
    }

    /// Display performance analysis results
    fn display_performance_results(&self, entries: &[LogCommitEntry]) -> Result<()> {
        println!("\nPer-commit stats (time | ms | loadedRows | receivedBytes | txnId)");
        println!("{}", "-".repeat(90));

        let mut stats = PerformanceStats::new();

        // Sort by time in ascending order
        let mut sorted_entries = entries.to_vec();
        sorted_entries.sort_by_key(|e| e.timestamp);

        for entry in &sorted_entries {
            self.display_single_entry(entry);
            stats.update(entry);
        }

        println!("{}", "-".repeat(90));
        stats.display_summary();

        Ok(())
    }

    fn display_single_entry(&self, entry: &LogCommitEntry) {
        let time_str = entry.timestamp.format("%H:%M:%S").to_string();
        let ms = entry.task_execution_ms.unwrap_or(0);
        let rows = entry.loaded_rows.unwrap_or(0);
        let bytes = entry.received_bytes.unwrap_or(0);

        println!(
            "{} | {:>6} | {:>13} | {:>16} | {}",
            time_str,
            ms,
            fmt_int(rows),
            fmt_int(bytes),
            entry.transaction_id.clone().unwrap_or_else(|| "-".into())
        );
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
            println!(
                "count={}  avg_ms={}  min_ms={}  max_ms={}",
                self.count,
                self.sum_ms / self.count as u128,
                if self.min_ms == u64::MAX {
                    0
                } else {
                    self.min_ms
                },
                self.max_ms
            );
            println!(
                "          avg_rows={}  min_rows={}  max_rows={}",
                fmt_int_u128(self.sum_rows / self.count as u128),
                fmt_int(if self.min_rows == u64::MAX {
                    0
                } else {
                    self.min_rows
                }),
                fmt_int(self.max_rows)
            );
            println!(
                "          avg_bytes={}  min_bytes={}  max_bytes={}",
                fmt_int_u128(self.sum_bytes / self.count as u128),
                fmt_int(if self.min_bytes == u64::MAX {
                    0
                } else {
                    self.min_bytes
                }),
                fmt_int(self.max_bytes)
            );
        }
    }
}

fn fmt_int(v: u64) -> String {
    let s = v.to_string();
    group_digits(&s)
}

fn fmt_int_u128(v: u128) -> String {
    let s = v.to_string();
    group_digits(&s)
}

fn group_digits(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let mut count = 0;
    for i in (0..bytes.len()).rev() {
        out.push(bytes[i] as char);
        count += 1;
        if count % 3 == 0 && i != 0 {
            out.push(',');
        }
    }
    out.chars().rev().collect()
}
