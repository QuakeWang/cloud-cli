mod error_checker;
mod job_lister;
mod job_manager;
mod log_parser;
mod models;
mod performance_analyzer;
mod traffic_monitor;

pub use error_checker::RoutineLoadErrorChecker;
pub use job_lister::RoutineLoadJobLister;
pub use job_manager::RoutineLoadJobManager;
pub use models::*;
pub use performance_analyzer::RoutineLoadPerformanceAnalyzer;
pub use traffic_monitor::RoutineLoadTrafficMonitor;

/// Routine Load tool index enum to avoid hardcoded indices
#[derive(Debug, Clone, Copy)]
pub enum RoutineLoadToolIndex {
    JobLister = 4,
    ErrorChecker = 5,
    PerformanceAnalyzer = 6,
    TrafficMonitor = 7,
}

impl RoutineLoadToolIndex {
    /// Get tool instance
    pub fn get_tool(
        self,
        tools: &[Box<dyn crate::tools::Tool>],
    ) -> Option<&dyn crate::tools::Tool> {
        tools.get(self as usize).map(|t| &**t)
    }
}

// Re-export all tools for use in ToolRegistry
pub fn get_routine_load_tools() -> Vec<Box<dyn crate::tools::Tool>> {
    vec![
        Box::new(RoutineLoadJobLister),
        Box::new(RoutineLoadErrorChecker),
        Box::new(RoutineLoadPerformanceAnalyzer),
        Box::new(RoutineLoadTrafficMonitor),
    ]
}
