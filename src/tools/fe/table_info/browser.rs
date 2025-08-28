use anyhow::Result;

use crate::ui::{InteractiveSelector, print_error, print_info};

use super::{FeTableInfoTool, TableIdentity};
use std::fs;
use std::path::PathBuf;

pub fn run_interactive(config: &crate::config::Config) -> Result<()> {
    loop {
        match select_database_or_bulk(config)? {
            DatabaseSelection::Single(db) => match select_table_or_bulk(config, &db)? {
                TableSelection::Single(ident) => {
                    let report = FeTableInfoTool::collect_one(config, &ident)?;
                    render_brief(&report);
                }
                TableSelection::AllInDb(db_name) => {
                    let total = FeTableInfoTool::list_tables(config, Some(&db_name))?.len();
                    let conc = FeTableInfoTool::suggest_concurrency(total);
                    let reports = FeTableInfoTool::collect_all_in_db(config, &db_name, conc)?;
                    if let Ok(files) = save_reports_txt(config, &reports, false) {
                        for f in files {
                            print_info(&format!("Saved: {}", f.display()));
                        }
                    }
                    render_batch_summary(&db_name, reports.len());
                }
            },
            DatabaseSelection::AllDbs => {
                print_info("Scanning all databases and tables...");
                let all_tables = FeTableInfoTool::list_tables(config, None)?;
                let conc = if all_tables.is_empty() {
                    16
                } else {
                    FeTableInfoTool::suggest_concurrency(all_tables.len())
                };
                print_info(&format!("Found {} tables, starting...", all_tables.len()));
                let reports = FeTableInfoTool::collect_many(config, &all_tables, conc)?;
                if let Ok(files) = save_reports_txt(config, &reports, true) {
                    print_info(&format!("Saved: {}", files[0].display()));
                }
                render_batch_summary("<all_dbs>", reports.len());
            }
        }

        match prompt_next_action()? {
            NextAction::AnalyzeAnother => continue,
            NextAction::BackToFeMenu => return Ok(()),
            NextAction::ExitApp => {
                crate::ui::print_goodbye();
                std::process::exit(0);
            }
        }
    }
}

pub fn select_database(config: &crate::config::Config) -> Result<String> {
    let dbs = FeTableInfoTool::list_databases(config)?;
    match create_string_selector(dbs, "Select a database".to_string(), false, "")? {
        SelectionResult::Single(db) => Ok(db),
        SelectionResult::All => unreachable!(),
    }
}

enum SelectionResult<T> {
    Single(T),
    All,
}

fn create_string_selector(
    items: Vec<String>,
    title: String,
    add_all_option: bool,
    all_option_text: &str,
) -> Result<SelectionResult<String>> {
    if items.is_empty() {
        print_error("No items found.");
        anyhow::bail!("no items")
    }

    let mut options = items;
    if add_all_option {
        options.push(all_option_text.to_string());
    }

    let selector = InteractiveSelector::new(options.clone(), title).with_page_size(30);
    let selected = selector.select()?.clone();

    if add_all_option && selected == all_option_text {
        Ok(SelectionResult::All)
    } else {
        Ok(SelectionResult::Single(selected))
    }
}

enum DatabaseSelection {
    Single(String),
    AllDbs,
}

fn select_database_or_bulk(config: &crate::config::Config) -> Result<DatabaseSelection> {
    let dbs = FeTableInfoTool::list_databases(config)?;
    match create_string_selector(
        dbs,
        "Select a database".to_string(),
        true,
        "[All Databases]",
    )? {
        SelectionResult::Single(db) => Ok(DatabaseSelection::Single(db)),
        SelectionResult::All => Ok(DatabaseSelection::AllDbs),
    }
}

enum TableSelection {
    Single(TableIdentity),
    AllInDb(String),
}

fn select_table_or_bulk(config: &crate::config::Config, database: &str) -> Result<TableSelection> {
    let tables = FeTableInfoTool::list_tables(config, Some(database))?;
    let names: Vec<String> = tables
        .into_iter()
        .filter(|t| t.schema == database)
        .map(|t| t.name)
        .collect();

    match create_string_selector(
        names,
        format!("Select a table in {}", database),
        true,
        "[All tables in this DB]",
    )? {
        SelectionResult::Single(name) => Ok(TableSelection::Single(TableIdentity {
            schema: database.to_string(),
            name,
        })),
        SelectionResult::All => Ok(TableSelection::AllInDb(database.to_string())),
    }
}

fn render_brief(report: &super::TableInfoReport) {
    let content = generate_report_content(report);
    for line in content.lines() {
        print_info(line);
    }
}

fn generate_report_content(report: &super::TableInfoReport) -> String {
    let mut out = String::new();
    out.push('\n');
    out.push_str(&"=".repeat(80));
    out.push('\n');
    out.push_str(&format!(
        "Table Info: {}.{}\n",
        report.ident.schema, report.ident.name
    ));
    out.push_str(&"-".repeat(80));
    out.push('\n');

    let model = format!("{:?}", report.model);
    let keys = if report.key_columns.is_empty() {
        "-".to_string()
    } else {
        report.key_columns.join(", ")
    };
    let bucket_str = match report.bucket {
        super::BucketCount::Fixed(n) => n.to_string(),
        super::BucketCount::Auto => "AUTO".to_string(),
    };
    let bucket_key = report
        .bucketing_key
        .as_ref()
        .map(|v| v.join(", "))
        .unwrap_or_else(|| "-".to_string());
    let mow = report
        .merge_on_write
        .map(|v| if v { "Yes" } else { "No" })
        .unwrap_or("-");

    out.push_str(&format!("  {:<18} {}\n", "Table Type:", model));
    out.push_str(&format!("  {:<18} {}\n", "Key Columns:", keys));
    out.push_str(&format!("  {:<18} {}\n", "Bucketing Key:", bucket_key));
    out.push_str(&format!("  {:<18} {}\n", "Bucket Count:", bucket_str));
    out.push_str(&format!("  {:<18} {}\n", "Merge-on-Write:", mow));

    let indexes_line = if report.indexes.is_empty() {
        "None".to_string()
    } else {
        report
            .indexes
            .iter()
            .map(|i| format!("{}({})", i.name, i.index_type))
            .collect::<Vec<_>>()
            .join(", ")
    };
    out.push_str(&format!(
        "  {:<18} {}\n",
        "Indexes:",
        truncate(&indexes_line, 50)
    ));

    out.push('\n');
    out.push_str("Partitions:\n");
    out.push_str(&build_partitions_table(&report.partitions));
    out.push_str(&format!("Total partitions: {}\n", report.partitions.len()));
    out.push_str(&"=".repeat(80));
    out
}

fn render_batch_summary(scope: &str, total: usize) {
    print_info("");
    print_info(&"=".repeat(80));
    print_info(&format!("Batch collection completed for {}", scope));
    print_info(&format!("Collected tables: {}", total));
    print_info(&"=".repeat(80));
}

fn build_partitions_table(parts: &[super::PartitionStat]) -> String {
    let w_part = 18usize;
    let w_size = 10usize;
    let w_rows = 12usize;
    let w_buck = 8usize;

    let mut s = String::new();
    let top = format!(
        "┌{}┬{}┬{}┬{}┐\n",
        "─".repeat(w_part + 2),
        "─".repeat(w_size + 2),
        "─".repeat(w_rows + 2),
        "─".repeat(w_buck + 2)
    );
    let mid = format!(
        "├{}┼{}┼{}┼{}┤\n",
        "─".repeat(w_part + 2),
        "─".repeat(w_size + 2),
        "─".repeat(w_rows + 2),
        "─".repeat(w_buck + 2)
    );
    let bot = format!(
        "└{}┴{}┴{}┴{}┘\n",
        "─".repeat(w_part + 2),
        "─".repeat(w_size + 2),
        "─".repeat(w_rows + 2),
        "─".repeat(w_buck + 2)
    );

    s.push_str(&top);
    s.push_str(&format!(
        "│ {:<w_part$} │ {:>w_size$} │ {:>w_rows$} │ {:>w_buck$} │\n",
        "Partition",
        "Size",
        "Rows",
        "Buckets",
        w_part = w_part,
        w_size = w_size,
        w_rows = w_rows,
        w_buck = w_buck
    ));
    s.push_str(&mid);
    for p in parts.iter() {
        let size = crate::tools::common::format_utils::format_bytes(p.size_bytes, 3, false);
        s.push_str(&format!(
            "│ {:<w_part$} │ {:>w_size$} │ {:>w_rows$} │ {:>w_buck$} │\n",
            truncate(&p.name, w_part),
            size,
            p.rows,
            p.buckets,
            w_part = w_part,
            w_size = w_size,
            w_rows = w_rows,
            w_buck = w_buck
        ));
    }
    s.push_str(&bot);
    s
}

enum NextAction {
    AnalyzeAnother,
    BackToFeMenu,
    ExitApp,
}

fn prompt_next_action() -> Result<NextAction> {
    let items = vec![
        "Analyze another table/database".to_string(),
        "Back to FE menu".to_string(),
        "Exit".to_string(),
    ];
    let selector =
        InteractiveSelector::new(items.clone(), "What would you like to do next?".to_string())
            .with_page_size(30);
    let sel = selector.select()?;
    match sel.as_str() {
        "Analyze another table/database" => Ok(NextAction::AnalyzeAnother),
        "Back to FE menu" => Ok(NextAction::BackToFeMenu),
        _ => Ok(NextAction::ExitApp),
    }
}

fn save_reports_txt(
    config: &crate::config::Config,
    reports: &[super::TableInfoReport],
    single_file: bool,
) -> anyhow::Result<Vec<PathBuf>> {
    let base_dir: PathBuf = config.output_dir.join("table-info");
    config.ensure_output_dir()?;

    if single_file {
        let file_path = base_dir.join("all_databases_table_info.txt");
        crate::tools::common::fs_utils::ensure_dir_exists(&file_path)?;
        let mut content = String::new();
        for r in reports {
            content.push_str(&generate_report_content(r));
            content.push('\n');
            content.push_str(&"-".repeat(80));
            content.push('\n');
        }
        fs::write(&file_path, content)?;
        Ok(vec![file_path])
    } else {
        let mut files: Vec<PathBuf> = Vec::with_capacity(reports.len());
        for r in reports {
            let dir = base_dir.join(&r.ident.schema);
            let file_path = dir.join(format!("{}.txt", &r.ident.name));
            crate::tools::common::fs_utils::ensure_dir_exists(&file_path)?;
            let content = generate_report_content(r);
            fs::write(&file_path, content)?;
            files.push(file_path);
        }
        Ok(files)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
