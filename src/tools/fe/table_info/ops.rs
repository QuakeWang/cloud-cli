use anyhow::Result;
use regex::Regex;

use super::{ColumnDef, CreateTableParsed, IndexInfo, TableIdentity, TableStatsFromPartitions};

const V2_MIN_COLS: usize = 15; // up to DataSize index (14)
const V3_MIN_COLS: usize = 22;

fn parse_column_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().trim_matches('`').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_bucket_count(buckets: &str) -> super::BucketCount {
    if buckets.eq_ignore_ascii_case("AUTO") {
        super::BucketCount::Auto
    } else {
        buckets
            .parse::<u32>()
            .map(super::BucketCount::Fixed)
            .unwrap_or(super::BucketCount::Auto)
    }
}

pub fn fetch_and_parse_all(
    exec: &super::sql::MySqlExecutor,
    ident: &TableIdentity,
) -> Result<(
    CreateTableParsed,
    TableStatsFromPartitions,
    Vec<ColumnDef>,
    Vec<IndexInfo>,
)> {
    let create_rs = super::sql::query_show_create(exec, ident)?;
    let parts_rs = super::sql::query_partitions(exec, ident)?;

    let create = parse_create_table(create_rs.0.as_str())?;
    let parts = parse_partitions(&parts_rs)?;
    let cols: Vec<ColumnDef> = Vec::new();
    let idxs = parse_indexes_from_create(create_rs.0.as_str());

    Ok((create, parts, cols, idxs))
}

pub fn parse_create_table(raw_sql: &str) -> Result<CreateTableParsed> {
    let model = if raw_sql.contains("UNIQUE KEY(") || raw_sql.contains("UNIQUE KEY (`") {
        super::TableModel::UniqueKey
    } else if raw_sql.contains("AGGREGATE KEY(") {
        super::TableModel::AggregateKey
    } else {
        super::TableModel::DuplicateKey
    };

    let key_cols = Regex::new(r"(?i)(UNIQUE|DUPLICATE|AGGREGATE)\s+KEY\((?P<cols>[^\)]*)\)")?
        .captures(raw_sql)
        .and_then(|c| c.name("cols").map(|m| m.as_str().to_string()))
        .unwrap_or_default();
    let key_columns = parse_column_list(&key_cols);

    let re_hash = Regex::new(
        r"DISTRIBUTED\s+BY\s+HASH\((?P<cols>[^\)]*)\)\s+BUCKETS\s+(?P<buckets>AUTO|\d+)",
    )?;
    let re_random = Regex::new(r"DISTRIBUTED\s+BY\s+RANDOM\s+BUCKETS\s+(?P<buckets>AUTO|\d+)")?;

    let bucketing = if let Some(c) = re_hash.captures(raw_sql) {
        let cols = c.name("cols").map(|m| m.as_str()).unwrap_or("");
        let columns = parse_column_list(cols);
        let buckets = c.name("buckets").map(|m| m.as_str()).unwrap_or("AUTO");
        let bucket_count = parse_bucket_count(buckets);
        super::BucketingSpec::Hash {
            columns,
            buckets: bucket_count,
        }
    } else if let Some(c) = re_random.captures(raw_sql) {
        let buckets = c.name("buckets").map(|m| m.as_str()).unwrap_or("AUTO");
        let bucket_count = parse_bucket_count(buckets);
        super::BucketingSpec::Random {
            buckets: bucket_count,
        }
    } else {
        super::BucketingSpec::Hash {
            columns: vec![],
            buckets: super::BucketCount::Auto,
        }
    };

    let mow = if matches!(model, super::TableModel::UniqueKey) {
        let lower = raw_sql.to_ascii_lowercase();
        if lower.contains("merge-on-write\" = \"true\"")
            || lower.contains("enable_unique_key_merge_on_write\" = \"true\"")
            || lower.contains("merge-on-write: yes")
        {
            Some(true)
        } else if lower.contains("merge-on-write\" = \"false\"") {
            Some(false)
        } else {
            None
        }
    } else {
        None
    };

    Ok(CreateTableParsed {
        model,
        key_columns,
        bucketing,
        merge_on_write: mow,
    })
}

pub fn parse_partitions(rows: &super::sql::ResultSet) -> Result<TableStatsFromPartitions> {
    let mut partitions = Vec::new();
    let mut first_bucket: Option<u32> = None;
    let mut all_equal: bool = true;

    for line in rows.0.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('\t').collect();
        if cols.len() < V2_MIN_COLS {
            continue;
        }

        // Decide layout by column count
        let (name_idx, buckets_idx, size_idx, rowcount_idx_opt): (
            usize,
            usize,
            usize,
            Option<usize>,
        ) = if cols.len() >= V3_MIN_COLS {
            // Doris 3.x (has RowCount at the end)
            (1, 8, 14, Some(cols.len() - 1))
        } else if cols.len() >= V2_MIN_COLS {
            // Doris 2.x (no RowCount)
            (1, 8, 14, None)
        } else {
            continue;
        };

        let name = cols
            .get(name_idx)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let buckets = cols
            .get(buckets_idx)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let data_size = cols.get(size_idx).map(|s| s.trim()).unwrap_or("");
        let row_count = rowcount_idx_opt
            .and_then(|i| cols.get(i))
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        let size_bytes = super::parse_size(data_size);
        let avg_bucket_sz = if buckets > 0 {
            Some(size_bytes / buckets as u64)
        } else {
            None
        };

        partitions.push(super::PartitionStat {
            name,
            size_bytes,
            rows: row_count,
            buckets,
            avg_bucket_size_bytes: avg_bucket_sz,
        });

        if buckets > 0 {
            if let Some(fb) = first_bucket {
                if fb != buckets {
                    all_equal = false;
                }
            } else {
                first_bucket = Some(buckets);
            }
        }
    }

    let total_buckets = if all_equal { first_bucket } else { None };
    Ok(TableStatsFromPartitions {
        partitions,
        total_buckets,
    })
}

pub fn parse_indexes_from_create(ddl: &str) -> Vec<IndexInfo> {
    let mut result: Vec<IndexInfo> = Vec::new();

    // Parse explicit INDEX ... USING ... (case-insensitive, supports multiple lines)
    if let Ok(re_idx) = Regex::new(
        r"(?mi)^\s*INDEX\s+`?(?P<name>[A-Za-z0-9_]+)`?\s*\((?P<cols>[^\)]*)\)\s*USING\s+(?P<itype>[A-Za-z0-9_]+)",
    ) {
        for cap in re_idx.captures_iter(ddl) {
            let name = cap
                .name("name")
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let cols_raw = cap.name("cols").map(|m| m.as_str()).unwrap_or("");
            let columns = parse_column_list(cols_raw);
            let itype = cap
                .name("itype")
                .map(|m| m.as_str())
                .unwrap_or("INDEX")
                .to_uppercase();
            result.push(IndexInfo {
                name,
                columns,
                index_type: itype,
            });
        }
    }

    // Parse bloom_filter_columns from PROPERTIES (fallback)
    if let Ok(re_bf) = Regex::new(r#"(?i)"bloom_filter_columns"\s*=\s*"(?P<cols>[^"]*)""#)
        && let Some(cap) = re_bf.captures(ddl)
    {
        let cols_raw = cap.name("cols").map(|m| m.as_str()).unwrap_or("");
        let columns = parse_column_list(cols_raw);
        if !columns.is_empty() {
            let display_name = format!("bloom_filter({})", columns.join(","));
            result.push(IndexInfo {
                name: display_name,
                columns,
                index_type: "BLOOM_FILTER".to_string(),
            });
        }
    }

    result
}
