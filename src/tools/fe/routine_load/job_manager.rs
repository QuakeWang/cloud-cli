use super::models::{JobStatistic, RoutineLoadJob, RoutineLoadState};
use crate::error::{CliError, Result};
use crate::tools::mysql::parser::{parse_key_value_pairs, split_into_blocks};
use once_cell::sync::Lazy;
use serde_json;
use std::collections::HashMap;
use std::sync::Mutex;

/// Global Routine Load state manager
static ROUTINE_LOAD_STATE: Lazy<Mutex<RoutineLoadState>> =
    Lazy::new(|| Mutex::new(RoutineLoadState::new()));

/// Routine Load Job ID manager
pub struct RoutineLoadJobManager;

impl RoutineLoadJobManager {
    /// Helper function: safely acquire state lock and execute operation
    fn with_state<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut RoutineLoadState) -> Result<T>,
    {
        let mut state = ROUTINE_LOAD_STATE
            .lock()
            .map_err(|_| CliError::ToolExecutionFailed("Failed to acquire state lock".into()))?;
        f(&mut state)
    }

    /// Helper function: read-only access to state
    fn with_state_readonly<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&RoutineLoadState) -> Result<T>,
    {
        let state = ROUTINE_LOAD_STATE
            .lock()
            .map_err(|_| CliError::ToolExecutionFailed("Failed to acquire state lock".into()))?;
        f(&state)
    }

    /// Save Job ID to memory
    pub fn save_job_id(&self, job_id: String, job_name: String, database: String) -> Result<()> {
        self.with_state(|state| {
            state.current_job_id = Some(job_id.clone());
            state.current_job_name = Some(job_name);
            state.last_database = Some(database);
            Ok(())
        })
    }

    /// Get current Job ID from memory
    pub fn get_current_job_id(&self) -> Option<String> {
        self.with_state_readonly(|state| Ok(state.current_job_id.clone()))
            .unwrap_or(None)
    }

    pub fn get_current_job_name(&self) -> Option<String> {
        self.with_state_readonly(|state| Ok(state.current_job_name.clone()))
            .unwrap_or(None)
    }

    pub fn get_last_database(&self) -> Option<String> {
        self.with_state_readonly(|state| Ok(state.last_database.clone()))
            .unwrap_or(None)
    }

    pub fn validate_job_id(&self, job_id: &str) -> Result<bool> {
        if !job_id.chars().all(|c| c.is_ascii_digit()) {
            return Ok(false);
        }

        self.with_state_readonly(|state| Ok(state.job_cache.contains_key(job_id)))
    }

    pub fn clear_state(&self) -> Result<()> {
        self.with_state(|state| {
            state.clear();
            Ok(())
        })
    }

    pub fn update_job_cache(&self, jobs: Vec<RoutineLoadJob>) -> Result<()> {
        self.with_state(|state| {
            state.job_cache.clear();
            for job in jobs {
                state.job_cache.insert(job.id.clone(), job);
            }
            Ok(())
        })
    }

    /// Get job cache
    pub fn get_job_cache(&self) -> Result<HashMap<String, RoutineLoadJob>> {
        self.with_state_readonly(|state| Ok(state.job_cache.clone()))
    }

    /// Parse Routine Load output
    pub fn parse_routine_load_output(&self, output: &str) -> Result<Vec<RoutineLoadJob>> {
        let blocks = split_into_blocks(output);
        let mut jobs = Vec::new();

        for block in blocks {
            if let Some(job) = self.parse_job_block(&block)? {
                jobs.push(job);
            }
        }

        Ok(jobs)
    }

    /// Parse single job block
    fn parse_job_block(&self, block: &str) -> Result<Option<RoutineLoadJob>> {
        let fields = parse_key_value_pairs(block);

        if !fields.contains_key("Id") || !fields.contains_key("Name") {
            return Ok(None);
        }

        let statistic = if let Some(stat_str) = fields.get("Statistic") {
            if stat_str != "NULL" {
                Some(self.parse_statistic(stat_str)?)
            } else {
                None
            }
        } else {
            None
        };

        let progress = if let Some(prog_str) = fields.get("Progress") {
            if prog_str != "NULL" {
                Some(self.parse_progress(prog_str)?)
            } else {
                None
            }
        } else {
            None
        };

        let lag = if let Some(lag_str) = fields.get("Lag") {
            if lag_str != "NULL" {
                Some(self.parse_lag(lag_str)?)
            } else {
                None
            }
        } else {
            None
        };

        let job = RoutineLoadJob {
            id: fields.get("Id").unwrap().clone(),
            name: fields.get("Name").unwrap().clone(),
            state: fields
                .get("State")
                .unwrap_or(&"UNKNOWN".to_string())
                .clone(),
            db_name: fields.get("DbName").unwrap_or(&"".to_string()).clone(),
            table_name: fields.get("TableName").unwrap_or(&"".to_string()).clone(),
            create_time: fields.get("CreateTime").unwrap_or(&"".to_string()).clone(),
            pause_time: fields.get("PauseTime").filter(|&s| s != "NULL").cloned(),
            end_time: fields.get("EndTime").filter(|&s| s != "NULL").cloned(),
            current_task_num: fields.get("CurrentTaskNum").cloned(),
            data_source_type: fields.get("DataSourceType").cloned(),
            statistic,
            progress,
            lag,
            error_log_urls: fields.get("ErrorLogUrls").cloned(),
            other_msg: fields.get("OtherMsg").cloned(),
        };

        Ok(Some(job))
    }

    /// Parse Statistic JSON field
    fn parse_statistic(&self, stat_str: &str) -> Result<JobStatistic> {
        let stat: serde_json::Value = serde_json::from_str(stat_str).map_err(|e| {
            CliError::ToolExecutionFailed(format!("Failed to parse statistic: {}", e))
        })?;

        Ok(JobStatistic {
            received_bytes: stat["receivedBytes"].as_u64().unwrap_or(0),
            loaded_rows: stat["loadedRows"].as_u64().unwrap_or(0),
            error_rows: stat["errorRows"].as_u64().unwrap_or(0),
            committed_task_num: stat["committedTaskNum"].as_u64().unwrap_or(0),
            load_rows_rate: stat["loadRowsRate"].as_u64().unwrap_or(0),
            aborted_task_num: stat["abortedTaskNum"].as_u64().unwrap_or(0),
            total_rows: stat["totalRows"].as_u64().unwrap_or(0),
            unselected_rows: stat["unselectedRows"].as_u64().unwrap_or(0),
            received_bytes_rate: stat["receivedBytesRate"].as_u64().unwrap_or(0),
            task_execute_time_ms: stat["taskExecuteTimeMs"].as_u64().unwrap_or(0),
        })
    }

    /// Parse Progress JSON field
    fn parse_progress(&self, prog_str: &str) -> Result<HashMap<String, String>> {
        let prog: HashMap<String, String> = serde_json::from_str(prog_str).map_err(|e| {
            CliError::ToolExecutionFailed(format!("Failed to parse progress: {}", e))
        })?;
        Ok(prog)
    }

    /// Parse Lag JSON field
    fn parse_lag(&self, lag_str: &str) -> Result<HashMap<String, u64>> {
        let lag: HashMap<String, u64> = serde_json::from_str(lag_str)
            .map_err(|e| CliError::ToolExecutionFailed(format!("Failed to parse lag: {e}")))?;
        Ok(lag)
    }
}
