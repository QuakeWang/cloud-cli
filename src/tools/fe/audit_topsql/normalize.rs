use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

static RE_LITERALS: Lazy<Regex> = Lazy::new(|| Regex::new(r"'[^']*'|\b\d+(?:\.\d+)?\b").unwrap());
static RE_IN_LIST: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bin\s*\(\s*\?(?:\s*,\s*\?)*\s*\)").unwrap());
static RE_NOT_IN_LIST: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bnot\s+in\s*\(\s*\?(?:\s*,\s*\?)*\s*\)").unwrap());
static RE_MULTI_SPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
static RE_OPERATOR_SPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*([=(),])\s*").unwrap());
static RE_LINE_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)--[^\n]*").unwrap());
static RE_BLOCK_COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
static RE_CTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:^|\bwith\b|,)\s*([a-z0-9_]+)\s+as\s*\(").unwrap());
static RE_FROM: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bfrom\s+([a-z0-9_.`]+)").unwrap());

pub fn normalize_sql(sql: &str) -> String {
    if sql.is_empty() {
        return String::new();
    }

    let mut out = RE_BLOCK_COMMENT.replace_all(sql, " ").to_string();
    if out.contains('\n') {
        out = RE_LINE_COMMENT.replace_all(&out, " ").to_string();
    }
    out = out.replace(['\r', '\n', '\t'], " ");

    out = RE_LITERALS.replace_all(&out, "?").to_string();
    out = RE_NOT_IN_LIST.replace_all(&out, "not in (?)").to_string();
    out = RE_IN_LIST.replace_all(&out, "in (?)").to_string();
    out = RE_OPERATOR_SPACE.replace_all(&out, "$1").to_string();
    out.make_ascii_lowercase();
    out = RE_MULTI_SPACE.replace_all(&out, " ").to_string();
    out.trim().to_string()
}

pub fn guess_table(normalized_sql: &str) -> Option<String> {
    if normalized_sql.is_empty() {
        return None;
    }

    let mut ctes: HashSet<String> = HashSet::new();
    for cap in RE_CTE.captures_iter(normalized_sql) {
        if let Some(name) = cap.get(1) {
            ctes.insert(name.as_str().to_ascii_lowercase());
        }
    }

    for cap in RE_FROM.captures_iter(normalized_sql) {
        let Some(m) = cap.get(1) else { continue };
        let table = m.as_str().replace('`', "");
        let table_lc = table.to_ascii_lowercase();
        if ctes.contains(&table_lc) {
            continue;
        }
        if matches!(
            table_lc.as_str(),
            "a" | "b"
                | "c"
                | "d"
                | "t"
                | "t_index"
                | "params"
                | "current_data"
                | "last_period_data"
        ) {
            continue;
        }
        return Some(table);
    }

    None
}
