use crate::error::{CliError, Result};
use chrono::NaiveDateTime;
use regex::Regex;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LogCommitEntry {
    pub timestamp: NaiveDateTime,
    pub loaded_rows: Option<u64>,
    pub received_bytes: Option<u64>,
    pub task_execution_ms: Option<u64>,
    pub transaction_id: Option<String>,
}

pub struct FeLogParser {
    re_ts: Regex,
    re_fields: Regex,
    re_txn: Regex,
}

impl FeLogParser {
    pub fn new() -> Self {
        Self {
            re_ts: Regex::new(r"^(?P<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}),\d{3}").unwrap(),
            re_fields: Regex::new(r"(loadedRows|receivedBytes|taskExecutionTimeMs)=([0-9]+)")
                .unwrap(),
            re_txn: Regex::new(r"transactionId:([0-9A-Za-z_-]+)").unwrap(),
        }
    }

    pub fn parse_line(&self, line: &str, job_id: &str) -> Option<LogCommitEntry> {
        if !line.contains(job_id) {
            return None;
        }
        // Only parse lines containing key fragments
        if !(line.contains("commitTxn") || line.contains("RLTaskTxnCommitAttachment")) {
            return None;
        }

        let ts = self.re_ts.captures(line)?.name("ts")?.as_str();
        let timestamp = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").ok()?;

        let mut entry = LogCommitEntry {
            timestamp,
            ..Default::default()
        };

        for cap in self.re_fields.captures_iter(line) {
            let k = &cap[1];
            let v: u64 = cap[2].parse().ok()?;
            match k {
                "loadedRows" => entry.loaded_rows = Some(v),
                "receivedBytes" => entry.received_bytes = Some(v),
                "taskExecutionTimeMs" => entry.task_execution_ms = Some(v),
                _ => {}
            }
        }

        if let Some(c) = self.re_txn.captures(line) {
            entry.transaction_id = Some(c[1].to_string());
        }

        Some(entry)
    }
}

pub fn collect_fe_logs(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Err(CliError::ConfigError(format!(
            "Log directory does not exist: {}",
            dir.display()
        )));
    }

    if !dir.is_dir() {
        return Err(CliError::ConfigError(format!(
            "Path is not a directory: {}",
            dir.display()
        )));
    }

    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(CliError::IoError)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.starts_with("fe.log"))
                .unwrap_or(false)
        })
        .collect();

    if files.is_empty() {
        return Err(CliError::ConfigError(format!(
            "No fe.log files found in directory: {}",
            dir.display()
        )));
    }

    // Sort by modification time (newest first)
    files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    files.reverse();

    Ok(files)
}

pub fn scan_file(
    parser: &FeLogParser,
    path: &Path,
    job_id: &str,
    out: &mut Vec<LogCommitEntry>,
) -> Result<()> {
    let f = fs::File::open(path).map_err(CliError::IoError)?;

    let reader = BufReader::new(f);

    for line_result in reader.lines() {
        let line = line_result.map_err(CliError::IoError)?;

        if let Some(entry) = parser.parse_line(&line, job_id) {
            out.push(entry);
        }
    }

    Ok(())
}
