use std::collections::HashMap;

/// Routine Load job information
#[derive(Debug, Clone)]
pub struct RoutineLoadJob {
    pub id: String,
    pub name: String,
    pub state: String,
    pub db_name: String,
    pub table_name: String,
    pub create_time: String,
    pub pause_time: Option<String>,
    pub end_time: Option<String>,
    pub current_task_num: Option<String>,
    pub data_source_type: Option<String>,
    pub statistic: Option<JobStatistic>,
    pub progress: Option<HashMap<String, String>>,
    pub lag: Option<HashMap<String, i64>>,
    pub error_log_urls: Option<String>,
    pub other_msg: Option<String>,
}

/// Job statistics information
#[derive(Debug, Clone)]
pub struct JobStatistic {
    pub received_bytes: u64,
    pub loaded_rows: u64,
    pub error_rows: u64,
    pub committed_task_num: u64,
    pub load_rows_rate: u64,
    pub aborted_task_num: u64,
    pub total_rows: u64,
    pub unselected_rows: u64,
    pub received_bytes_rate: u64,
    pub task_execute_time_ms: u64,
}

/// In-memory state management
#[derive(Debug, Clone)]
pub struct RoutineLoadState {
    pub current_job_id: Option<String>,
    pub current_job_name: Option<String>,
    pub last_database: Option<String>,
    pub job_cache: HashMap<String, RoutineLoadJob>,
}

impl RoutineLoadState {
    pub fn new() -> Self {
        Self {
            current_job_id: None,
            current_job_name: None,
            last_database: None,
            job_cache: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.current_job_id = None;
        self.current_job_name = None;
        self.last_database = None;
        self.job_cache.clear();
    }
}

impl Default for RoutineLoadState {
    fn default() -> Self {
        Self::new()
    }
}

/// Log commit entry for parsed log data
#[derive(Debug, Clone, Default)]
pub struct LogCommitEntry {
    pub timestamp: chrono::NaiveDateTime,
    pub loaded_rows: Option<u64>,
    pub received_bytes: Option<u64>,
    pub task_execution_ms: Option<u64>,
    pub transaction_id: Option<String>,
}
