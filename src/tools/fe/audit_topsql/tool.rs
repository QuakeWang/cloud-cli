use super::aggregate::TemplateAggregator;
use super::normalize::{guess_table, normalize_sql};
use super::parser::parse_records_streaming;
use super::report::{ReportOptions, render_console_visualization, render_report};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::tools::common::fs_utils;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use dialoguer::{Input, Select};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

const ENV_AUDIT_TOPSQL_INPUT: &str = "CLOUD_CLI_FE_AUDIT_TOPSQL_INPUT";
const MIN_COUNT_EXCLUSIVE: u64 = 10;
const TOP_N: usize = 50;
const SAMPLES: usize = 10;
const MAX_STMT_LEN: usize = 4000;

pub struct FeAuditTopSqlTool;

impl Tool for FeAuditTopSqlTool {
    fn name(&self) -> &str {
        "fe-audit-topsql"
    }

    fn description(&self) -> &str {
        "Analyze fe.audit.log and generate a TopSQL template report"
    }

    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, config: &Config, _pid: u32) -> Result<ExecutionResult> {
        config.ensure_output_dir()?;

        let doris_config = crate::config_loader::load_config()?;
        let input_path = select_audit_log_path(&doris_config.log_dir)?;

        ui::print_info("Parsing audit log...");
        let file = File::open(&input_path).map_err(CliError::IoError)?;
        let reader = BufReader::new(file);
        let mut aggregator = TemplateAggregator::new();
        let stats = parse_records_streaming(reader, |r| {
            let tpl = normalize_sql(&r.stmt);
            if tpl.is_empty() {
                return;
            }
            let table = guess_table(&tpl);
            aggregator.push(tpl, table, r.time_ms, r.cpu_ms, r.stmt, r.query_id);
        })?;

        ui::print_info("Finalizing report...");
        let analysis = aggregator.finish(MIN_COUNT_EXCLUSIVE);

        let report_opts = ReportOptions {
            input_path: input_path.display().to_string(),
            min_count_exclusive: MIN_COUNT_EXCLUSIVE,
            top_n: TOP_N,
            samples: SAMPLES,
            max_stmt_len: MAX_STMT_LEN,
        };
        let report = render_report(
            &analysis,
            &report_opts,
            stats.lines,
            stats.records_ok,
            stats.records_bad,
        );

        let out_path = build_output_path(&config.output_dir, &input_path);
        std::fs::write(&out_path, report).map_err(CliError::IoError)?;

        let console_view = render_console_visualization(
            &analysis,
            &report_opts,
            stats.lines,
            stats.records_ok,
            stats.records_bad,
        );
        ui::print_info(&console_view);

        Ok(ExecutionResult {
            output_path: out_path,
            message: "TopSQL report generated successfully.".to_string(),
        })
    }
}

fn select_audit_log_path(log_dir: &Path) -> Result<PathBuf> {
    if let Ok(input) = std::env::var(ENV_AUDIT_TOPSQL_INPUT) {
        let path = PathBuf::from(input.trim());
        if !path.exists() {
            return Err(CliError::ConfigError(format!(
                "Audit log does not exist: {} (from {})",
                path.display(),
                ENV_AUDIT_TOPSQL_INPUT
            )));
        }
        return Ok(path);
    }

    let candidates = fs_utils::collect_fe_audit_logs(log_dir).ok();

    if let Some(files) = candidates {
        let mut items: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        items.push("Enter path manually".to_string());

        let selection = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select FE audit log file")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| CliError::InvalidInput(format!("Audit log selection failed: {e}")))?;

        if selection < files.len() {
            return Ok(files[selection].clone());
        }
    } else {
        ui::print_warning(&format!(
            "Failed to discover audit logs under: {}",
            log_dir.display()
        ));
    }

    let input: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Enter FE audit log path")
        .interact_text()
        .map_err(|e| CliError::InvalidInput(format!("Path input failed: {e}")))?;

    let path = PathBuf::from(input.trim());
    if !path.exists() {
        return Err(CliError::ConfigError(format!(
            "Audit log does not exist: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn build_output_path(output_dir: &Path, input_path: &Path) -> PathBuf {
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let base = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("fe.audit.log");
    output_dir.join(format!("fe-audit-topsql-{base}-{ts}.txt"))
}
