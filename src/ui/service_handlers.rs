use crate::config::Config;
use crate::error::{self, Result};
use crate::tools::Tool;
use crate::ui::*;

fn index_by_name(tools: &[Box<dyn Tool>], name: &str) -> Option<usize> {
    tools.iter().position(|t| t.name() == name)
}

fn run_tool_with_post(
    config: &Config,
    tools: &[Box<dyn Tool>],
    index: usize,
    service: &str,
) -> Result<Option<()>> {
    let tool = &*tools[index];
    if let Err(e) = crate::execute_tool_enhanced(config, tool, service) {
        match e {
            error::CliError::GracefulExit => {}
            _ => print_error(&format!("Tool execution failed: {e}")),
        }
        return Ok(Some(()));
    }

    match show_post_execution_menu(tool.name())? {
        PostExecutionAction::Continue => Ok(Some(())),
        PostExecutionAction::BackToMain => Err(error::CliError::GracefulExit),
        PostExecutionAction::Exit => {
            crate::ui::print_goodbye();
            std::process::exit(0);
        }
    }
}

fn run_tool_by_name(
    config: &Config,
    tools: &[Box<dyn Tool>],
    name: &str,
    service: &str,
) -> Result<Option<()>> {
    let Some(index) = index_by_name(tools, name) else {
        print_error(&format!("Tool '{name}' not found for {service}."));
        return Ok(Some(()));
    };
    run_tool_with_post(config, tools, index, service)
}

fn run_jmap_submenu_by_names(
    config: &Config,
    tools: &[Box<dyn Tool>],
    dump_name: &str,
    histo_name: &str,
    service: &str,
) -> Result<Option<()>> {
    loop {
        match crate::ui::show_jmap_menu()? {
            crate::ui::JmapAction::Dump => {
                match run_tool_by_name(config, tools, dump_name, service) {
                    Err(error::CliError::GracefulExit) => return Ok(None),
                    _ => continue,
                }
            }
            crate::ui::JmapAction::Histo => {
                match run_tool_by_name(config, tools, histo_name, service) {
                    Err(error::CliError::GracefulExit) => return Ok(None),
                    _ => continue,
                }
            }
            crate::ui::JmapAction::Back => return Ok(Some(())),
        }
    }
}

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
                run_tool_by_name(config, tools, "fe-list", "FE").ok();
            }
            crate::ui::FeToolAction::Jmap => {
                match run_jmap_submenu_by_names(config, tools, "jmap-dump", "jmap-histo", "FE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::FeToolAction::Jstack => {
                match run_tool_by_name(config, tools, "jstack", "FE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::FeToolAction::FeProfiler => {
                match run_tool_by_name(config, tools, "fe-profiler", "FE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
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
        match crate::ui::show_be_tools_menu()? {
            crate::ui::BeToolAction::BeList => {
                match run_tool_by_name(config, tools, "be-list", "BE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::BeToolAction::Pstack => {
                match run_tool_by_name(config, tools, "pstack", "BE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::BeToolAction::BeVars => {
                match run_tool_by_name(config, tools, "get-be-vars", "BE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::BeToolAction::Jmap => {
                match run_jmap_submenu_by_names(config, tools, "jmap-dump", "jmap-histo", "BE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::BeToolAction::PipelineTasks => {
                match run_tool_by_name(config, tools, "pipeline-tasks", "BE") {
                    Err(error::CliError::GracefulExit) => return Ok(()),
                    _ => continue,
                }
            }
            crate::ui::BeToolAction::Memz => loop {
                match crate::ui::show_memz_menu()? {
                    crate::ui::MemzAction::Current => {
                        match run_tool_by_name(config, tools, "memz", "BE") {
                            Err(error::CliError::GracefulExit) => return Ok(()),
                            _ => continue,
                        }
                    }
                    crate::ui::MemzAction::Global => {
                        match run_tool_by_name(config, tools, "memz-global", "BE") {
                            Err(error::CliError::GracefulExit) => return Ok(()),
                            _ => continue,
                        }
                    }
                    crate::ui::MemzAction::Back => break,
                }
            },
            crate::ui::BeToolAction::Back => return Ok(()),
        }
    }
}
