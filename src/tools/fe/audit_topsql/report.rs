use super::aggregate::{AnalysisResult, TemplateStats};
use crate::ui::FormatHelper;
use console::style;
use std::collections::HashMap;

const REPORT_WIDTH: usize = 96;
const CPU_SHARE_HOT: f64 = 0.25;
const CPU_SHARE_WARN: f64 = 0.15;
const CONSOLE_TEMPLATE_SNIPPET_LEN: usize = 80;
const CONSOLE_DIFF_CONTEXT_CHARS: usize = 32;
const CONSOLE_SAMPLE_STMT_MAX_LEN: usize = 400;
#[derive(Copy, Clone)]
enum Align {
    Left,
    Right,
}

struct TableColumn<'a> {
    header: &'a str,
    width: usize,
    align: Align,
}

impl<'a> TableColumn<'a> {
    const fn new(header: &'a str, width: usize, align: Align) -> Self {
        Self {
            header,
            width,
            align,
        }
    }

    fn format_cell(&self, value: &str) -> String {
        let truncated = truncate_to_width(value, self.width);
        match self.align {
            Align::Left => format!("{:<width$}", truncated, width = self.width),
            Align::Right => format!("{:>width$}", truncated, width = self.width),
        }
    }
}

pub struct ReportOptions {
    pub input_path: String,
    pub min_count_exclusive: u64,
    pub top_n: usize,
    pub samples: usize,
    pub max_stmt_len: usize,
}

pub fn render_report(
    result: &AnalysisResult,
    opts: &ReportOptions,
    parse_lines: u64,
    parsed: u64,
    bad: u64,
) -> String {
    let mut out = String::new();

    let rule = "=".repeat(REPORT_WIDTH);
    out.push_str(&format!("{rule}\nFE Audit TopSQL Report\n{rule}\n"));

    render_summary(&mut out, result, opts, parse_lines, parsed, bad);
    out.push('\n');

    let top_n = opts.top_n.min(result.items.len());
    let by_total_cpu = &result.items;

    render_template_metrics(&mut out, by_total_cpu, top_n, result.total_cpu_ms);
    render_samples(&mut out, by_total_cpu, opts, result.total_cpu_ms);

    out
}

pub fn render_console_visualization(
    result: &AnalysisResult,
    opts: &ReportOptions,
    _parse_lines: u64,
    _parsed: u64,
    _bad: u64,
) -> String {
    let mut out = String::new();
    render_console_insights(result, opts, &mut out);
    render_console_top_templates(result, &mut out);
    render_console_template_details(result, &mut out);
    out
}

fn render_console_insights(result: &AnalysisResult, opts: &ReportOptions, out: &mut String) {
    if result.items.is_empty() {
        out.push_str("Insights:\n  No SQL templates found in audit log.\n\n");
        return;
    }

    let mut table_map: HashMap<&str, (u64, usize)> = HashMap::new();
    for tpl in &result.items {
        if let Some(table) = tpl.table.as_deref() {
            let entry = table_map.entry(table).or_insert((0, 0));
            entry.0 += tpl.total_cpu_ms;
            entry.1 += 1;
        }
    }
    out.push_str("Insights:\n");
    if result.used_fallback {
        out.push_str(&format!(
            "  • No SQL patterns matched count > {}; showing results without count filter\n",
            opts.min_count_exclusive
        ));
    }
    if table_map.is_empty() {
        out.push_str("  • Table information unavailable in audit log\n");
    } else {
        let mut table_entries: Vec<(&str, (u64, usize))> = table_map.into_iter().collect();
        table_entries.sort_unstable_by_key(|(_, (cpu, _))| std::cmp::Reverse(*cpu));
        for (table, (cpu_ms, patterns)) in table_entries.into_iter().take(3) {
            let share = percent(result.total_cpu_ms, cpu_ms);
            let severity = if share >= CPU_SHARE_HOT {
                style("⚠").red().bold()
            } else if share >= CPU_SHARE_WARN {
                style("▲").yellow()
            } else {
                style("•").green()
            };
            out.push_str(&format!(
                "  {sev} Table {table} uses {pct} CPU ({patterns} patterns)\n",
                sev = severity,
                table = table,
                pct = fmt_pct(share),
                patterns = patterns
            ));
        }
    }

    out.push('\n');
}

fn render_console_top_templates(result: &AnalysisResult, out: &mut String) {
    if result.items.is_empty() {
        return;
    }
    out.push_str(&format!(
        "Top SQL Patterns Overview:\n{}\n",
        "-".repeat(REPORT_WIDTH)
    ));
    let mut prev_tpl: Option<&str> = None;
    for (idx, tpl) in result.items.iter().take(5).enumerate() {
        let share = percent(result.total_cpu_ms, tpl.total_cpu_ms);
        let colored_bar = colorize(render_bar(share, 30), share);
        let colored_pct = colorize(fmt_pct(share), share);
        let query_id = tpl.slowest_query_id.as_deref().unwrap_or("-");
        let header = format!(
            "#{rank:<2} {bar} {pct:<8} cpu={cpu}ms count={count} avg={avg:.2}ms min={min_time}ms max={max_time}ms slowest={slow}ms",
            rank = idx + 1,
            bar = colored_bar,
            pct = colored_pct,
            cpu = FormatHelper::fmt_int(tpl.total_cpu_ms),
            count = FormatHelper::fmt_int(tpl.count),
            avg = tpl.avg_time_ms(),
            min_time = FormatHelper::fmt_int(tpl.min_time_ms),
            max_time = FormatHelper::fmt_int(tpl.max_time_ms),
            slow = FormatHelper::fmt_int(tpl.slowest_time_ms),
        );
        out.push_str(&format!("{header}\n"));
        out.push_str(&format!(
            "      query_id={query_id} table={} sql={}\n",
            tpl.table.as_deref().unwrap_or("-"),
            truncate_one_line(&tpl.sql_template, CONSOLE_TEMPLATE_SNIPPET_LEN)
        ));

        if let Some(prev_tpl) = prev_tpl
            && let Some(diff_pos) = first_diff_char_pos(prev_tpl, &tpl.sql_template)
        {
            let head_len = (CONSOLE_TEMPLATE_SNIPPET_LEN.saturating_sub(3)) / 2;
            if diff_pos >= head_len {
                let prev_ctx = diff_context(prev_tpl, diff_pos, CONSOLE_DIFF_CONTEXT_CHARS);
                let cur_ctx = diff_context(&tpl.sql_template, diff_pos, CONSOLE_DIFF_CONTEXT_CHARS);
                out.push_str(&format!(
                    "      diff@{diff_pos} prev=\"{prev_ctx}\" -> curr=\"{cur_ctx}\"\n"
                ));
            }
        }
        prev_tpl = Some(&tpl.sql_template);
    }
    out.push('\n');
}

fn render_console_template_details(result: &AnalysisResult, out: &mut String) {
    if result.items.is_empty() {
        return;
    }
    out.push_str(&format!("Slowest Samples:\n{}\n", "-".repeat(REPORT_WIDTH)));
    for (idx, tpl) in result.items.iter().take(3).enumerate() {
        let query_id = tpl.slowest_query_id.as_deref().unwrap_or("-");
        let header = format!(
            "[#{rank}] query_id={query_id} slowest_time={slow}ms avg_time={avg:.2}ms count={count} cpu_total={cpu}ms",
            rank = idx + 1,
            slow = FormatHelper::fmt_int(tpl.slowest_time_ms),
            avg = tpl.avg_time_ms(),
            count = FormatHelper::fmt_int(tpl.count),
            cpu = FormatHelper::fmt_int(tpl.total_cpu_ms)
        );
        out.push_str(&format!("{}\n", style(header).bold()));
        let stmt = truncate_stmt(&tpl.slowest_stmt, CONSOLE_SAMPLE_STMT_MAX_LEN);
        out.push_str(&stmt);
        out.push_str(if stmt.ends_with('\n') { "\n" } else { "\n\n" });
    }
}
fn render_summary(
    out: &mut String,
    result: &AnalysisResult,
    opts: &ReportOptions,
    parse_lines: u64,
    parsed: u64,
    bad: u64,
) {
    out.push_str(&section_header("Summary"));
    let filter_value = if result.used_fallback {
        format!(
            "count > {} (no matches, showing all)",
            opts.min_count_exclusive
        )
    } else {
        format!("count > {}", opts.min_count_exclusive)
    };
    let columns = vec![
        TableColumn::new("Field", 24, Align::Left),
        TableColumn::new("Value", 68, Align::Left),
    ];
    let rows = vec![
        vec!["Input".to_string(), opts.input_path.clone()],
        vec!["Lines".to_string(), FormatHelper::fmt_int(parse_lines)],
        vec!["Parsed records".to_string(), FormatHelper::fmt_int(parsed)],
        vec!["Bad records".to_string(), FormatHelper::fmt_int(bad)],
        vec!["Filter".to_string(), filter_value],
        vec![
            "SQL templates".to_string(),
            FormatHelper::fmt_int(result.total_templates),
        ],
        vec![
            "Executions".to_string(),
            FormatHelper::fmt_int(result.total_executions),
        ],
        vec![
            "Total CPU".to_string(),
            format!("{} ms", FormatHelper::fmt_int(result.total_cpu_ms)),
        ],
        vec![
            "Total Time".to_string(),
            format!("{} ms", FormatHelper::fmt_int(result.total_time_ms)),
        ],
    ];
    render_table(out, &columns, &rows);
}

fn render_template_metrics(
    out: &mut String,
    items: &[TemplateStats],
    top_n: usize,
    total_cpu: u64,
) {
    if top_n == 0 {
        out.push_str("No SQL templates available for analysis.\n");
        return;
    }

    out.push_str(&section_header(&format!(
        "Top {top_n} SQL Patterns (by total_cpu_ms)"
    )));
    let columns = vec![
        TableColumn::new("rank", 4, Align::Right),
        TableColumn::new("cpu_ms", 12, Align::Right),
        TableColumn::new("cpu%", 7, Align::Right),
        TableColumn::new("count", 9, Align::Right),
        TableColumn::new("avg_time", 10, Align::Right),
        TableColumn::new("min_time", 10, Align::Right),
        TableColumn::new("max_time", 10, Align::Right),
        TableColumn::new("slowest", 10, Align::Right),
        TableColumn::new("table", 12, Align::Left),
        TableColumn::new("template", 40, Align::Left),
    ];
    let mut rows = Vec::new();
    for (idx, tpl) in items.iter().take(top_n).enumerate() {
        let share = percent(total_cpu, tpl.total_cpu_ms);
        rows.push(vec![
            (idx + 1).to_string(),
            FormatHelper::fmt_int(tpl.total_cpu_ms),
            fmt_pct(share),
            FormatHelper::fmt_int(tpl.count),
            format!("{:.2}", tpl.avg_time_ms()),
            FormatHelper::fmt_int(tpl.min_time_ms),
            FormatHelper::fmt_int(tpl.max_time_ms),
            FormatHelper::fmt_int(tpl.slowest_time_ms),
            tpl.table.clone().unwrap_or_else(|| "-".to_string()),
            truncate_one_line(&tpl.sql_template, 160),
        ]);
    }
    render_table(out, &columns, &rows);
}

fn render_samples(
    out: &mut String,
    items_by_total_cpu: &[TemplateStats],
    opts: &ReportOptions,
    total_cpu: u64,
) {
    let sample_n = opts.samples.min(items_by_total_cpu.len());
    if sample_n == 0 {
        return;
    }

    out.push_str(&section_header(&format!(
        "Slowest Samples (top {sample_n} by total_cpu_ms)"
    )));
    let columns = vec![
        TableColumn::new("#", 3, Align::Right),
        TableColumn::new("count", 10, Align::Right),
        TableColumn::new("total_cpu_ms", 14, Align::Right),
        TableColumn::new("avg_cpu", 10, Align::Right),
        TableColumn::new("max_time_ms", 12, Align::Right),
        TableColumn::new("cpu_share", 9, Align::Right),
        TableColumn::new("template", 36, Align::Left),
    ];
    let mut rows = Vec::new();
    for (idx, it) in items_by_total_cpu.iter().take(sample_n).enumerate() {
        let share = percent(total_cpu, it.total_cpu_ms);
        rows.push(vec![
            (idx + 1).to_string(),
            FormatHelper::fmt_int(it.count),
            FormatHelper::fmt_int(it.total_cpu_ms),
            format!("{:.2}", it.avg_cpu_ms()),
            format!("{:.2}", it.slowest_time_ms as f64),
            fmt_pct(share),
            truncate_one_line(&it.sql_template, 120),
        ]);
    }
    render_table(out, &columns, &rows);

    for (idx, it) in items_by_total_cpu.iter().take(sample_n).enumerate() {
        let query_id = it.slowest_query_id.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "Sample #{} (query_id={query_id})\n{}\n",
            idx + 1,
            "-".repeat(32)
        ));
        let stmt = truncate_stmt(&it.slowest_stmt, opts.max_stmt_len);
        out.push_str(&stmt);
        out.push_str(if stmt.ends_with('\n') { "\n" } else { "\n\n" });
    }
}

fn truncate_stmt(stmt: &str, max_len: usize) -> String {
    let stmt = stmt.trim_end();
    let total_chars = stmt.chars().count();
    if total_chars <= max_len {
        return stmt.to_string();
    }

    if max_len < 80 {
        let truncated = total_chars.saturating_sub(max_len);
        return format!(
            "{}\n-- [truncated {} chars]\n",
            stmt.chars().take(max_len).collect::<String>().trim_end(),
            truncated
        );
    }

    let head_len = max_len / 2;
    let tail_len = max_len.saturating_sub(head_len);
    let truncated = total_chars.saturating_sub(head_len + tail_len);

    let head = stmt
        .chars()
        .take(head_len)
        .collect::<String>()
        .trim_end()
        .to_string();
    let tail = stmt
        .chars()
        .skip(total_chars.saturating_sub(tail_len))
        .collect::<String>()
        .trim_start()
        .to_string();

    format!("{head}\n-- [truncated {truncated} chars, showing head and tail]\n{tail}")
}

fn truncate_one_line(s: &str, max_len: usize) -> String {
    let s = normalize_whitespace(&s.replace(['\r', '\n', '\t'], " "));
    let total_chars = s.chars().count();
    if total_chars <= max_len {
        return s;
    }

    if max_len < 20 {
        return format!("{}...", s.chars().take(max_len).collect::<String>());
    }

    let head_len = (max_len.saturating_sub(3)) / 2;
    let tail_len = max_len.saturating_sub(3).saturating_sub(head_len);
    let head = s.chars().take(head_len).collect::<String>();
    let tail = s
        .chars()
        .skip(total_chars.saturating_sub(tail_len))
        .collect::<String>();
    format!("{head}...{tail}")
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_diff_char_pos(a: &str, b: &str) -> Option<usize> {
    a.chars()
        .zip(b.chars())
        .position(|(ca, cb)| ca != cb)
        .or_else(|| {
            let common = a.chars().zip(b.chars()).count();
            (a.chars().count() != b.chars().count()).then_some(common)
        })
}

fn diff_context(s: &str, diff_pos: usize, context: usize) -> String {
    let start = diff_pos.saturating_sub(context);
    let len = context.saturating_mul(2).max(1);
    let snippet: String = s.chars().skip(start).take(len).collect();
    normalize_whitespace(&snippet.replace(['\r', '\n', '\t'], " "))
}

fn render_table(out: &mut String, columns: &[TableColumn<'_>], rows: &[Vec<String>]) {
    if columns.is_empty() {
        return;
    }
    let border = table_rule(columns, '-');
    out.push_str(&border);
    out.push_str(&table_header(columns));
    out.push_str(&table_rule(columns, '='));
    for row in rows {
        out.push('|');
        for (idx, col) in columns.iter().enumerate() {
            let value = row.get(idx).map(|s| s.as_str()).unwrap_or("");
            out.push(' ');
            out.push_str(&col.format_cell(value));
            out.push(' ');
            out.push('|');
        }
        out.push('\n');
    }
    out.push_str(&border);
    out.push('\n');
}

fn table_rule(columns: &[TableColumn<'_>], ch: char) -> String {
    let mut line = String::new();
    line.push('+');
    for col in columns {
        line.extend(std::iter::repeat_n(ch, col.width + 2));
        line.push('+');
    }
    line.push('\n');
    line
}

fn table_header(columns: &[TableColumn<'_>]) -> String {
    let mut line = String::new();
    line.push('|');
    for col in columns {
        let header = truncate_to_width(col.header, col.width);
        line.push(' ');
        line.push_str(&format!("{:^width$}", header, width = col.width));
        line.push(' ');
        line.push('|');
    }
    line.push('\n');
    line
}

fn truncate_to_width(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let len = value.chars().count();
    if len <= width {
        return value.to_string();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    let mut s: String = value.chars().take(width - 3).collect();
    s.push_str("...");
    s
}

fn section_header(title: &str) -> String {
    let rule = "-".repeat(REPORT_WIDTH);
    format!("{rule}\n{title}\n{rule}\n")
}

fn fmt_pct(x: f64) -> String {
    format!("{:.2}%", x * 100.0)
}

fn render_bar(share: f64, width: usize) -> String {
    let filled = (share.clamp(0.0, 1.0) * width as f64).round() as usize;
    "#".repeat(filled) + &".".repeat(width.saturating_sub(filled))
}

fn percent(total: u64, part: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

fn colorize(value: impl std::fmt::Display, share: f64) -> String {
    if share >= CPU_SHARE_HOT {
        style(value).red().bold().to_string()
    } else if share >= CPU_SHARE_WARN {
        style(value).yellow().to_string()
    } else {
        style(value).green().to_string()
    }
}
