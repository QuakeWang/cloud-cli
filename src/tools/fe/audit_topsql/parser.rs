use crate::error::{CliError, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::BufRead;

static RE_RECORD_START: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d{3} \[").unwrap());
static RE_TIME_MS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\|Time\(ms\)=(\d+)").unwrap());
static RE_CPU_MS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\|CpuTimeMS=(\d+)").unwrap());
static RE_QUERY_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"\|QueryId=([^|\n]+)").unwrap());

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub time_ms: u64,
    pub cpu_ms: u64,
    pub query_id: Option<String>,
    pub stmt: String,
}

#[derive(Debug, Default, Clone)]
pub struct ParseStats {
    pub lines: u64,
    pub records_total: u64,
    pub records_ok: u64,
    pub records_bad: u64,
}

pub fn parse_records_streaming<R: BufRead, F: FnMut(AuditRecord)>(
    reader: R,
    mut on_record: F,
) -> Result<ParseStats> {
    let mut stats = ParseStats::default();
    let mut buf = String::new();

    let flush = |record: &str, stats: &mut ParseStats, on_record: &mut F| {
        if record.trim().is_empty() {
            return;
        }
        stats.records_total += 1;

        let time_ms = RE_TIME_MS
            .captures(record)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);

        let cpu_ms = RE_CPU_MS
            .captures(record)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);

        let stmt = extract_stmt(record);
        if stmt.trim().is_empty() {
            stats.records_bad += 1;
            return;
        }

        let query_id = RE_QUERY_ID
            .captures(record)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .filter(|s| !s.is_empty());

        stats.records_ok += 1;
        on_record(AuditRecord {
            time_ms,
            cpu_ms,
            query_id,
            stmt,
        });
    };

    for line in reader.lines() {
        let mut line = line.map_err(CliError::IoError)?;
        stats.lines += 1;
        if line.ends_with('\r') {
            line.pop();
        }

        if RE_RECORD_START.is_match(&line) {
            if !buf.is_empty() {
                flush(&buf, &mut stats, &mut on_record);
                buf.clear();
            }
            buf.push_str(&line);
        } else if !buf.is_empty() {
            buf.push('\n');
            buf.push_str(&line);
        } else {
            continue;
        }
    }

    if !buf.is_empty() {
        flush(&buf, &mut stats, &mut on_record);
    }

    Ok(stats)
}

fn extract_stmt(record: &str) -> String {
    const STMT_KEY: &str = "|Stmt=";

    let Some(stmt_pos) = record.find(STMT_KEY) else {
        return String::new();
    };

    let stmt_start = stmt_pos + STMT_KEY.len();
    let stmt_end = find_stmt_end(record, stmt_start);

    let stmt = record[stmt_start..stmt_end].trim();
    stmt.strip_suffix('|').unwrap_or(stmt).trim().to_string()
}

fn find_stmt_end(record: &str, stmt_start: usize) -> usize {
    let mut search = stmt_start;
    while let Some(rel) = record[search..].find('|') {
        let idx = search + rel;
        if idx > stmt_start && is_kv_boundary(&record[idx..]) {
            return idx;
        }
        search = idx + 1;
    }
    record.len()
}

fn is_kv_boundary(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('|') else {
        return false;
    };
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() {
        return false;
    }

    let mut key_len = 1usize;
    for ch in chars {
        if ch == '=' {
            return key_len >= 2;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '(' | ')') {
            key_len += 1;
            continue;
        }
        return false;
    }
    false
}
