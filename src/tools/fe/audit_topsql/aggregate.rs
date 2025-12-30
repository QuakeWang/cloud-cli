use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TemplateStats {
    pub sql_template: String,
    pub table: Option<String>,
    pub slowest_stmt: String,
    pub slowest_query_id: Option<String>,
    pub slowest_time_ms: u64,
    pub count: u64,
    pub total_time_ms: u64,
    pub total_cpu_ms: u64,
    pub max_time_ms: u64,
    pub min_time_ms: u64,
}

impl TemplateStats {
    pub fn avg_time_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.total_time_ms as f64 / self.count as f64
    }

    pub fn avg_cpu_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.total_cpu_ms as f64 / self.count as f64
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub items: Vec<TemplateStats>,
    pub total_templates: u64,
    pub total_executions: u64,
    pub total_time_ms: u64,
    pub total_cpu_ms: u64,
    pub used_fallback: bool,
}

#[derive(Debug, Default)]
struct TemplateAgg {
    table: Option<String>,
    slowest_stmt: String,
    slowest_query_id: Option<String>,
    slowest_time_ms: u64,
    count: u64,
    total_time_ms: u64,
    total_cpu_ms: u64,
    max_time_ms: u64,
    min_time_ms: u64,
}

pub struct TemplateAggregator {
    map: HashMap<String, TemplateAgg>,
}

impl TemplateAggregator {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn push(
        &mut self,
        template: String,
        table: Option<String>,
        time_ms: u64,
        cpu_ms: u64,
        stmt: String,
        query_id: Option<String>,
    ) {
        let entry = self.map.entry(template).or_insert_with(|| TemplateAgg {
            min_time_ms: u64::MAX,
            ..Default::default()
        });

        if entry.table.is_none() {
            entry.table = table;
        }

        entry.count += 1;
        entry.total_time_ms += time_ms;
        entry.total_cpu_ms += cpu_ms;
        entry.max_time_ms = entry.max_time_ms.max(time_ms);
        entry.min_time_ms = entry.min_time_ms.min(time_ms);

        if time_ms > entry.slowest_time_ms {
            entry.slowest_time_ms = time_ms;
            entry.slowest_stmt = stmt;
            entry.slowest_query_id = query_id;
        }
    }

    pub fn finish(self, min_count_exclusive: u64) -> AnalysisResult {
        let has_threshold_matches = self.map.values().any(|x| x.count > min_count_exclusive);

        let mut items: Vec<TemplateStats> = self
            .map
            .into_iter()
            .filter(|(_, x)| !has_threshold_matches || x.count > min_count_exclusive)
            .map(|(sql_template, x)| TemplateStats {
                sql_template,
                table: x.table,
                slowest_stmt: x.slowest_stmt,
                slowest_query_id: x.slowest_query_id,
                slowest_time_ms: x.slowest_time_ms,
                count: x.count,
                total_time_ms: x.total_time_ms,
                total_cpu_ms: x.total_cpu_ms,
                max_time_ms: x.max_time_ms,
                min_time_ms: if x.min_time_ms == u64::MAX {
                    0
                } else {
                    x.min_time_ms
                },
            })
            .collect();

        items.sort_by_key(|x| std::cmp::Reverse(x.total_cpu_ms));

        let total_templates = items.len() as u64;
        let (total_executions, total_time_ms, total_cpu_ms) =
            items.iter().fold((0u64, 0u64, 0u64), |acc, it| {
                (
                    acc.0 + it.count,
                    acc.1 + it.total_time_ms,
                    acc.2 + it.total_cpu_ms,
                )
            });

        AnalysisResult {
            used_fallback: !has_threshold_matches && !items.is_empty(),
            items,
            total_templates,
            total_executions,
            total_time_ms,
            total_cpu_ms,
        }
    }
}
