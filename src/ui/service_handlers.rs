use crate::config::Config;
use crate::error::{self, Result};
use crate::tools::Tool;
use crate::ui::*;

/// Generic loop for handling a service type (FE or BE).
pub fn handle_service_loop(
    config: &Config,
    service_name: &str,
    tools: &[Box<dyn Tool>],
) -> Result<()> {
    if service_name == "FE" {
        handle_fe_service_loop(config, tools)
    } else {
        handle_be_service_loop(config, tools)
    }
}

/// Handle FE service loop with nested menu structure
pub fn handle_fe_service_loop(config: &Config, tools: &[Box<dyn Tool>]) -> Result<()> {
    loop {
        match crate::ui::show_fe_tools_menu()? {
            crate::ui::FeToolAction::FeList => {
                let tool = &*tools[0];
                if let Err(e) = crate::execute_tool_enhanced(config, tool, "FE") {
                    match e {
                        error::CliError::GracefulExit => {}
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }
            }
            crate::ui::FeToolAction::JmapDump => {
                let tool = &*tools[1];
                if let Err(e) = crate::execute_tool_enhanced(config, tool, "FE") {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }
                match crate::ui::show_post_execution_menu(tool.name())? {
                    crate::ui::PostExecutionAction::Continue => continue,
                    crate::ui::PostExecutionAction::BackToMain => return Ok(()),
                    crate::ui::PostExecutionAction::Exit => {
                        crate::ui::print_goodbye();
                        std::process::exit(0);
                    }
                }
            }
            crate::ui::FeToolAction::JmapHisto => {
                let tool = &*tools[2];
                if let Err(e) = crate::execute_tool_enhanced(config, tool, "FE") {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }
                match crate::ui::show_post_execution_menu(tool.name())? {
                    crate::ui::PostExecutionAction::Continue => continue,
                    crate::ui::PostExecutionAction::BackToMain => return Ok(()),
                    crate::ui::PostExecutionAction::Exit => {
                        crate::ui::print_goodbye();
                        std::process::exit(0);
                    }
                }
            }
            crate::ui::FeToolAction::Jstack => {
                let tool = &*tools[3];
                if let Err(e) = crate::execute_tool_enhanced(config, tool, "FE") {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }
                match crate::ui::show_post_execution_menu(tool.name())? {
                    crate::ui::PostExecutionAction::Continue => continue,
                    crate::ui::PostExecutionAction::BackToMain => return Ok(()),
                    crate::ui::PostExecutionAction::Exit => {
                        crate::ui::print_goodbye();
                        std::process::exit(0);
                    }
                }
            }
            crate::ui::FeToolAction::FeProfiler => {
                let tool = &*tools[4];
                if let Err(e) = crate::execute_tool_enhanced(config, tool, "FE") {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }
                match crate::ui::show_post_execution_menu(tool.name())? {
                    crate::ui::PostExecutionAction::Continue => continue,
                    crate::ui::PostExecutionAction::BackToMain => return Ok(()),
                    crate::ui::PostExecutionAction::Exit => {
                        crate::ui::print_goodbye();
                        std::process::exit(0);
                    }
                }
            }
            crate::ui::FeToolAction::TableInfo => {
                if let Err(e) = crate::tools::fe::table_info::browser::run_interactive(config) {
                    print_error(&format!("Table info browse failed: {e}"));
                }
            }
            crate::ui::FeToolAction::RoutineLoad => {
                if let Err(e) = handle_routine_load_loop(config, tools) {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Routine Load error: {e}")),
                    }
                }
            }
            crate::ui::FeToolAction::Back => return Ok(()),
        }
    }
}

/// Handle Routine Load sub-menu loop
pub fn handle_routine_load_loop(config: &Config, tools: &[Box<dyn Tool>]) -> Result<()> {
    loop {
        match crate::ui::show_routine_load_menu()? {
            crate::ui::RoutineLoadAction::GetJobId => execute_routine_load_tool(
                config,
                tools,
                crate::tools::fe::routine_load::RoutineLoadToolIndex::JobLister,
            )?,

            crate::ui::RoutineLoadAction::Performance => execute_routine_load_tool(
                config,
                tools,
                crate::tools::fe::routine_load::RoutineLoadToolIndex::PerformanceAnalyzer,
            )?,
            crate::ui::RoutineLoadAction::Traffic => execute_routine_load_tool(
                config,
                tools,
                crate::tools::fe::routine_load::RoutineLoadToolIndex::TrafficMonitor,
            )?,
            crate::ui::RoutineLoadAction::Back => return Ok(()),
        }
    }
}

fn execute_routine_load_tool(
    config: &Config,
    tools: &[Box<dyn Tool>],
    tool_index: crate::tools::fe::routine_load::RoutineLoadToolIndex,
) -> Result<()> {
    let tool = tool_index.get_tool(tools).ok_or_else(|| {
        error::CliError::ToolExecutionFailed(format!(
            "Tool not found at index {}",
            tool_index as usize
        ))
    })?;

    if let Err(e) = crate::execute_tool_enhanced(config, tool, "FE") {
        match e {
            error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
            _ => print_error(&format!("Tool execution failed: {e}")),
        }
        return Ok(());
    }
    match crate::ui::show_post_execution_menu(tool.name())? {
        crate::ui::PostExecutionAction::Continue => Ok(()),
        crate::ui::PostExecutionAction::BackToMain => Err(error::CliError::GracefulExit),
        crate::ui::PostExecutionAction::Exit => {
            crate::ui::print_goodbye();
            std::process::exit(0);
        }
    }
}

/// Handle BE service loop (original logic)
pub fn handle_be_service_loop(config: &Config, tools: &[Box<dyn Tool>]) -> Result<()> {
    loop {
        match show_tool_selection_menu(2, "Select BE tool", tools)? {
            Some(tool) => {
                if let Err(e) = crate::execute_tool_enhanced(config, tool, "BE") {
                    match e {
                        error::CliError::GracefulExit => { /* Do nothing, just loop again */ }
                        _ => print_error(&format!("Tool execution failed: {e}")),
                    }
                }

                match show_post_execution_menu(tool.name())? {
                    PostExecutionAction::Continue => continue,
                    PostExecutionAction::BackToMain => return Ok(()),
                    PostExecutionAction::Exit => {
                        crate::ui::print_goodbye();
                        std::process::exit(0);
                    }
                }
            }
            None => return Ok(()), // "Back" was selected
        }
    }
}
