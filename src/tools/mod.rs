pub mod be;
pub mod common;
pub mod fe;

use crate::config::Config;
use crate::error::Result;
use std::path::PathBuf;

/// Result of executing a tool
#[derive(Debug)]
pub struct ExecutionResult {
    /// Path to the generated output file
    pub output_path: PathBuf,
    /// Success message describing the operation
    pub message: String,
}

/// Trait for diagnostic tools that can be executed against processes
pub trait Tool {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn execute(&self, config: &Config, pid: u32) -> Result<ExecutionResult>;

    /// Indicates whether the tool requires a process PID to execute.
    /// Most tools do, so the default is true.
    fn requires_pid(&self) -> bool {
        true
    }
}

/// Registry for all available diagnostic tools
pub struct ToolRegistry {
    fe_tools: Vec<Box<dyn Tool>>,
    be_tools: Vec<Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Creates a new tool registry with all available tools
    pub fn new() -> Self {
        use crate::tools::be::{BeVarsTool, PipelineTasksTool, PstackTool};
        use crate::tools::be::{JmapDumpTool as BeJmapDumpTool, JmapHistoTool as BeJmapHistoTool};
        use crate::tools::fe::{JmapDumpTool, JmapHistoTool, JstackTool};

        let mut registry = Self {
            fe_tools: Vec::new(),
            be_tools: Vec::new(),
        };

        // Register FE tools
        registry.fe_tools.push(Box::new(JmapDumpTool));
        registry.fe_tools.push(Box::new(JmapHistoTool));
        registry.fe_tools.push(Box::new(JstackTool));

        // Register BE tools
        registry.be_tools.push(Box::new(PstackTool));
        registry.be_tools.push(Box::new(BeVarsTool));
        registry.be_tools.push(Box::new(BeJmapDumpTool));
        registry.be_tools.push(Box::new(BeJmapHistoTool));
        registry.be_tools.push(Box::new(PipelineTasksTool));

        registry
    }

    pub fn fe_tools(&self) -> &[Box<dyn Tool>] {
        &self.fe_tools
    }

    pub fn be_tools(&self) -> &[Box<dyn Tool>] {
        &self.be_tools
    }

    pub fn get_fe_tool(&self, index: usize) -> Option<&dyn Tool> {
        self.fe_tools.get(index).map(|b| &**b)
    }

    pub fn get_be_tool(&self, index: usize) -> Option<&dyn Tool> {
        self.be_tools.get(index).map(|b| &**b)
    }
}
