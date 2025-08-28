use anyhow::Result;
use serde::{Deserialize, Serialize};

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;

pub mod browser;
mod ops;
pub mod sql;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIdentity {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TableModel {
    UniqueKey,
    DuplicateKey,
    AggregateKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BucketCount {
    Fixed(u32),
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionStat {
    pub name: String,
    pub size_bytes: u64,
    pub rows: u64,
    pub buckets: u32,
    pub avg_bucket_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStatsFromPartitions {
    pub partitions: Vec<PartitionStat>,
    pub total_buckets: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BucketingSpec {
    Hash {
        columns: Vec<String>,
        buckets: BucketCount,
    },
    Random {
        buckets: BucketCount,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTableParsed {
    pub model: TableModel,
    pub key_columns: Vec<String>,
    pub bucketing: BucketingSpec,
    pub merge_on_write: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfoReport {
    pub ident: TableIdentity,
    pub model: TableModel,
    pub key_columns: Vec<String>,
    pub bucketing_key: Option<Vec<String>>,
    pub bucket: BucketCount,
    pub merge_on_write: Option<bool>,
    pub indexes: Vec<IndexInfo>,
    pub columns: Vec<ColumnDef>,
    pub partitions: Vec<PartitionStat>,
}

pub struct FeTableInfoTool;

impl FeTableInfoTool {
    fn create_client(cfg: &crate::config::Config) -> Result<sql::MySqlExecutor> {
        let doris_cfg = crate::config_loader::load_config()?.with_app_config(cfg);
        Ok(sql::MySqlExecutor::from_config(doris_cfg))
    }

    pub fn list_tables(
        cfg: &crate::config::Config,
        schema: Option<&str>,
    ) -> Result<Vec<TableIdentity>> {
        // Load doris config to pass mysql credentials
        let client = Self::create_client(cfg)?;
        let rs = sql::query_table_list(&client, schema)?;

        // Map raw lines "schema\ttable" into identities (since raw mode -N -B -r -A)
        let mut out = Vec::new();
        for line in rs.0.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split('\t');
            if let (Some(s), Some(t)) = (parts.next(), parts.next()) {
                out.push(TableIdentity {
                    schema: s.to_string(),
                    name: t.to_string(),
                });
            }
        }
        Ok(out)
    }

    pub fn list_databases(cfg: &crate::config::Config) -> anyhow::Result<Vec<String>> {
        let client = Self::create_client(cfg)?;
        let rs = sql::query_database_list(&client)?;
        let mut out = Vec::new();
        for line in rs.0.lines() {
            let db = line.trim();
            if db.is_empty() {
                continue;
            }
            if ["information_schema", "mysql", "__internal_schema"].contains(&db) {
                continue;
            }
            out.push(db.to_string());
        }
        out.sort();
        Ok(out)
    }

    pub fn collect_one(
        cfg: &crate::config::Config,
        ident: &TableIdentity,
    ) -> Result<TableInfoReport> {
        let client = Self::create_client(cfg)?;
        let (create, parts, cols, idxs) = ops::fetch_and_parse_all(&client, ident)?;
        let report = assemble_report(ident, &create, &parts, &cols, &idxs);
        Ok(report)
    }

    fn collect_many(
        cfg: &crate::config::Config,
        idents: &[TableIdentity],
        concurrency: usize,
    ) -> Result<Vec<TableInfoReport>> {
        if idents.is_empty() {
            return Ok(Vec::new());
        }

        let doris_cfg = crate::config_loader::load_config()?.with_app_config(cfg);
        let worker_count = concurrency
            .max(1)
            .min(Self::suggest_concurrency(idents.len()));

        let total = idents.len();
        let shared_idents: Arc<Vec<TableIdentity>> = Arc::new(idents.to_vec());
        let results: Arc<Mutex<Vec<Option<TableInfoReport>>>> =
            Arc::new(Mutex::new(vec![None; total]));
        let next_index = Arc::new(AtomicUsize::new(0));
        let progress = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let doris_cfg_cloned = doris_cfg.clone();
            let shared_idents_cloned = Arc::clone(&shared_idents);
            let results_cloned = Arc::clone(&results);
            let next_index_cloned = Arc::clone(&next_index);
            let progress_cloned = Arc::clone(&progress);

            let handle = thread::spawn(move || {
                let client = sql::MySqlExecutor::from_config(doris_cfg_cloned);
                loop {
                    let idx = next_index_cloned.fetch_add(1, Ordering::SeqCst);
                    if idx >= shared_idents_cloned.len() {
                        break;
                    }
                    let ident = &shared_idents_cloned[idx];
                    let res = ops::fetch_and_parse_all(&client, ident).map(
                        |(create, parts, cols, idxs)| {
                            assemble_report(ident, &create, &parts, &cols, &idxs)
                        },
                    );
                    match res {
                        Ok(rep) => {
                            if let Ok(mut guard) = results_cloned.lock() {
                                guard[idx] = Some(rep);
                            }
                        }
                        Err(e) => {
                            crate::ui::print_error(&format!(
                                "Collect failed for {}.{}: {}",
                                ident.schema, ident.name, e
                            ));
                        }
                    }
                    let done = progress_cloned.fetch_add(1, Ordering::SeqCst) + 1;
                    crate::ui::print_info(&format!(
                        "Process: {}/{} {}.{}",
                        done, total, ident.schema, ident.name
                    ));
                }
            });
            handles.push(handle);
        }

        for h in handles {
            let _ = h.join();
        }

        let reports: Vec<TableInfoReport> = results
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .flatten()
            .collect();
        Ok(reports)
    }

    pub fn collect_all_in_db(
        cfg: &crate::config::Config,
        db: &str,
        concurrency: usize,
    ) -> Result<Vec<TableInfoReport>> {
        let tables = Self::list_tables(cfg, Some(db))?;
        let idents: Vec<TableIdentity> = tables.into_iter().filter(|t| t.schema == db).collect();
        Self::collect_many(cfg, &idents, concurrency)
    }

    pub fn collect_all_in_all_dbs(
        cfg: &crate::config::Config,
        concurrency: usize,
    ) -> Result<Vec<TableInfoReport>> {
        // One shot: list all tables across all databases to avoid double scanning
        let idents: Vec<TableIdentity> = Self::list_tables(cfg, None)?;
        Self::collect_many(cfg, &idents, concurrency)
    }

    pub fn suggest_concurrency(total_tables: usize) -> usize {
        if total_tables <= 1 {
            return 1;
        }
        let hard_cap = 32usize;
        let mut c = 2usize;
        while c < total_tables && c < hard_cap {
            c = c.saturating_mul(2);
        }
        c = c.min(total_tables).min(hard_cap);
        c.max(1)
    }
}

fn assemble_report(
    ident: &TableIdentity,
    create: &CreateTableParsed,
    parts: &TableStatsFromPartitions,
    cols: &[ColumnDef],
    idxs: &[IndexInfo],
) -> TableInfoReport {
    let (final_bucket, bucketing_key) = match &create.bucketing {
        BucketingSpec::Hash { columns, buckets } => (buckets.clone(), Some(columns.clone())),
        BucketingSpec::Random { buckets } => (buckets.clone(), None),
    };

    let merge_on_write = match create.model {
        TableModel::UniqueKey => create.merge_on_write,
        _ => None,
    };

    TableInfoReport {
        ident: ident.clone(),
        model: create.model.clone(),
        key_columns: create.key_columns.clone(),
        bucketing_key,
        bucket: final_bucket,
        merge_on_write,
        indexes: idxs.to_vec(),
        columns: cols.to_vec(),
        partitions: parts.partitions.clone(),
    }
}

pub(crate) fn parse_size(input: &str) -> u64 {
    let s = input.trim();
    if s.is_empty() {
        return 0;
    }

    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return 0;
    }

    let num = parts[0].parse::<f64>().unwrap_or(0.0);
    let unit = parts
        .get(1)
        .map(|u| u.to_ascii_lowercase())
        .unwrap_or_else(|| "b".to_string());

    let factor = match unit.as_str() {
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };

    (num * factor) as u64
}
