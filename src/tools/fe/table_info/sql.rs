use anyhow::Result;

// Minimal ResultSet abstraction wrapping raw mysql output (-N -B -r -A)
#[derive(Debug, Clone)]
pub struct ResultSet(pub String);

pub struct MySqlExecutor {
    doris: crate::config_loader::DorisConfig,
}

impl MySqlExecutor {
    pub fn from_config(doris: crate::config_loader::DorisConfig) -> Self {
        Self { doris }
    }

    pub fn query(&self, sql: &str) -> Result<ResultSet> {
        let output = crate::tools::mysql::MySQLTool::query_sql_raw_with_config(&self.doris, sql)?;
        Ok(ResultSet(output))
    }
}

pub fn query_table_list(exec: &MySqlExecutor, schema: Option<&str>) -> Result<ResultSet> {
    let mut sql = String::from(
        "SELECT table_schema, table_name FROM information_schema.tables \
        WHERE TABLE_TYPE = 'BASE TABLE' AND ENGINE = 'Doris' \
        AND TABLE_SCHEMA NOT IN ('__internal_schema', 'information_schema', 'mysql')",
    );
    if let Some(db) = schema {
        sql.push_str(&format!(" AND table_schema = '{}'", db.replace("'", "''")));
    }
    sql.push_str(" ORDER BY table_schema, table_name;");
    exec.query(&sql)
}

pub fn query_database_list(exec: &MySqlExecutor) -> Result<ResultSet> {
    exec.query("SHOW DATABASES;")
}

pub fn query_show_create(exec: &MySqlExecutor, ident: &super::TableIdentity) -> Result<ResultSet> {
    let sql = format!(
        "SHOW CREATE TABLE `{}`.`{}`;",
        ident.schema.replace("`", "``"),
        ident.name.replace("`", "``")
    );
    exec.query(&sql)
}

pub fn query_partitions(exec: &MySqlExecutor, ident: &super::TableIdentity) -> Result<ResultSet> {
    let sql = format!(
        "SHOW PARTITIONS FROM `{}`.`{}`;",
        ident.schema.replace("`", "``"),
        ident.name.replace("`", "``")
    );
    exec.query(&sql)
}
