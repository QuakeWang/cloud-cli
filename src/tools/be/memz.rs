use super::be_http_client;
use crate::config::Config;
use crate::error::Result;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use chrono::Utc;
use regex::Regex;
use std::fs;
use std::path::PathBuf;

/// Tool to analyze Jemalloc memory usage in BE
pub struct MemzTool;

/// Tool to analyze global memory usage in BE
pub struct MemzGlobalTool;

impl Tool for MemzTool {
    fn name(&self) -> &str {
        "memz"
    }

    fn description(&self) -> &str {
        "Analyze Jemalloc memory usage in BE"
    }

    fn execute(&self, config: &Config, _pid: u32) -> Result<ExecutionResult> {
        ui::print_info("Fetching Jemalloc memory usage from BE...");

        let result = be_http_client::request_be_webserver_port("/memz", None);

        match result {
            Ok(html_content) => {
                let (metrics_table, full_html) = extract_memory_metrics(&html_content);

                let output_path = save_html_to_file(config, &full_html, "memz")?;
                let path_display = output_path.display().to_string();

                ui::print_success("Memory metrics fetched successfully!");
                println!();
                ui::print_info("Results:");
                println!("{metrics_table}");

                Ok(ExecutionResult {
                    output_path,
                    message: format!("Jemalloc memory profile saved to {path_display}"),
                })
            }
            Err(e) => {
                ui::print_error(&format!("Failed to fetch memory metrics: {e}."));
                ui::print_info("Tips: Ensure the BE service is running and accessible.");
                Err(e)
            }
        }
    }

    fn requires_pid(&self) -> bool {
        false
    }
}

impl Tool for MemzGlobalTool {
    fn name(&self) -> &str {
        "memz-global"
    }

    fn description(&self) -> &str {
        "Analyze global memory usage in BE"
    }

    fn execute(&self, config: &Config, _pid: u32) -> Result<ExecutionResult> {
        ui::print_info("Fetching global memory usage from BE...");

        let result = be_http_client::request_be_webserver_port("/memz?type=global", None);

        match result {
            Ok(html_content) => {
                let (metrics_table, full_html) = extract_memory_metrics(&html_content);

                let output_path = save_html_to_file(config, &full_html, "memz_global")?;
                let path_display = output_path.display().to_string();

                ui::print_success("Global memory metrics fetched successfully!");
                println!();
                ui::print_info("Results:");
                println!("{metrics_table}");

                Ok(ExecutionResult {
                    output_path,
                    message: format!("Global memory profile saved to {path_display}"),
                })
            }
            Err(e) => {
                ui::print_error(&format!("Failed to fetch global memory metrics: {e}."));
                ui::print_info("Tips: Ensure the BE service is running and accessible.");
                Err(e)
            }
        }
    }

    fn requires_pid(&self) -> bool {
        false
    }
}

/// Format bytes to a human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB ({bytes} bytes)", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB ({bytes} bytes)", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB ({bytes} bytes)", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

/// Extract memory metrics from the HTML response
fn extract_memory_metrics(html_content: &str) -> (String, String) {
    let re = Regex::new(r"Allocated: (\d+), active: (\d+), metadata: (\d+).*?, resident: (\d+), mapped: (\d+), retained: (\d+)").unwrap();
    let thread_cache_re = Regex::new(r"tcache_bytes:\s+(\d+)").unwrap();
    let dirty_pages_re = Regex::new(r"dirty:\s+N/A\s+\d+\s+\d+\s+\d+\s+(\d+)").unwrap();

    let mut allocated = "Unknown".to_string();
    let mut active = "Unknown".to_string();
    let mut metadata = "Unknown".to_string();
    let mut resident = "Unknown".to_string();
    let mut mapped = "Unknown".to_string();
    let mut retained = "Unknown".to_string();
    let mut thread_cache = "Unknown".to_string();
    let mut dirty_pages = "Unknown".to_string();

    if re
        .captures(html_content)
        .map(|caps| caps.len() > 6)
        .unwrap_or(false)
    {
        let caps = re.captures(html_content).unwrap();
        if let Some(bytes) = caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok()) {
            allocated = format_bytes(bytes);
        }

        if let Some(bytes) = caps.get(2).and_then(|m| m.as_str().parse::<u64>().ok()) {
            active = format_bytes(bytes);
        }

        if let Some(bytes) = caps.get(3).and_then(|m| m.as_str().parse::<u64>().ok()) {
            metadata = format_bytes(bytes);
        }

        if let Some(bytes) = caps.get(4).and_then(|m| m.as_str().parse::<u64>().ok()) {
            resident = format_bytes(bytes);
        }

        if let Some(bytes) = caps.get(5).and_then(|m| m.as_str().parse::<u64>().ok()) {
            mapped = format_bytes(bytes);
        }

        if let Some(bytes) = caps.get(6).and_then(|m| m.as_str().parse::<u64>().ok()) {
            retained = format_bytes(bytes);
        }
    }

    if let Some(bytes) = thread_cache_re
        .captures(html_content)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
    {
        thread_cache = format_bytes(bytes);
    }

    if let Some(bytes) = dirty_pages_re
        .captures(html_content)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
    {
        dirty_pages = format_bytes(bytes);
    }

    let table = format!(
        " Key Memory Metrics:\n\
        ┌───────────────────┬────────────────────────────────────┐\n\
        │ Metric            │ Value                              │\n\
        ├───────────────────┼────────────────────────────────────┤\n\
        │ Allocated         │ {allocated:<34} │\n\
        │ Active            │ {active:<34} │\n\
        │ Metadata          │ {metadata:<34} │\n\
        │ Resident          │ {resident:<34} │\n\
        │ Mapped            │ {mapped:<34} │\n\
        │ Retained          │ {retained:<34} │\n\
        │ Thread Cache      │ {thread_cache:<34} │\n\
        │ Dirty Pages       │ {dirty_pages:<34} │\n\
        └───────────────────┴────────────────────────────────────┘"
    );

    (table, html_content.to_string())
}

/// Save HTML content to file and return the path
fn save_html_to_file(config: &Config, html_content: &str, file_prefix: &str) -> Result<PathBuf> {
    config.ensure_output_dir()?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{file_prefix}_{timestamp}.html");
    let output_path = config.output_dir.join(filename);

    fs::write(&output_path, html_content)?;

    Ok(output_path)
}
