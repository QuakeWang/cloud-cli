use crate::config::Config;
use crate::error::{self, Result};
use crate::tools::{Tool, ToolRegistry};
use crate::ui::*;

fn run_tool_with_post(config: &mut Config, tool: &dyn Tool, service: &str) -> Result<Option<()>> {
    let status = crate::ui::tool_executor::execute_tool_enhanced(config, tool, service)?;
    if let Some(updated) = status.updated_config {
        *config = updated;
    }
    if !status.completed {
        return Ok(Some(()));
    }

    match show_post_execution_menu(tool.name())? {
        PostExecutionAction::Continue => Ok(Some(())),
        PostExecutionAction::BackToMain => Err(error::CliError::GracefulExit),
        PostExecutionAction::Exit => Err(error::CliError::UserExit),
    }
}

fn should_back_to_main(result: Result<Option<()>>) -> Result<bool> {
    match result {
        Ok(_) => Ok(false),
        Err(error::CliError::GracefulExit) => Ok(true),
        Err(e) => Err(e),
    }
}

fn run_tool_by_name(
    config: &mut Config,
    registry: &ToolRegistry,
    name: &str,
    service: &str,
) -> Result<Option<()>> {
    let Some(tool) = registry.get_tool(service, name) else {
        print_error(&format!("Tool '{name}' not found for {service}."));
        return Ok(Some(()));
    };
    run_tool_with_post(config, tool, service)
}

fn run_jmap_submenu_by_names(
    config: &mut Config,
    registry: &ToolRegistry,
    dump_name: &str,
    histo_name: &str,
    service: &str,
) -> Result<Option<()>> {
    loop {
        match crate::ui::show_jmap_menu()? {
            crate::ui::JmapAction::Dump => {
                if should_back_to_main(run_tool_by_name(config, registry, dump_name, service))? {
                    return Err(error::CliError::GracefulExit);
                }
                continue;
            }
            crate::ui::JmapAction::Histo => {
                if should_back_to_main(run_tool_by_name(config, registry, histo_name, service))? {
                    return Err(error::CliError::GracefulExit);
                }
                continue;
            }
            crate::ui::JmapAction::Back => return Ok(Some(())),
        }
    }
}

/// Generic loop for handling a service type (FE or BE).
pub fn handle_service_loop(
    config: &mut Config,
    service_name: &str,
    registry: &ToolRegistry,
) -> Result<()> {
    if service_name == "FE" {
        handle_fe_service_loop(config, registry)
    } else {
        handle_be_service_loop(config, registry)
    }
}

/// Handle FE service loop with nested menu structure
pub fn handle_fe_service_loop(config: &mut Config, registry: &ToolRegistry) -> Result<()> {
    loop {
        match crate::ui::show_fe_tools_menu()? {
            crate::ui::FeToolAction::FeList => {
                if should_back_to_main(run_tool_by_name(config, registry, "fe-list", "FE"))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::FeToolAction::Jmap => {
                if should_back_to_main(run_jmap_submenu_by_names(
                    config,
                    registry,
                    "jmap-dump",
                    "jmap-histo",
                    "FE",
                ))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::FeToolAction::Jstack => {
                if should_back_to_main(run_tool_by_name(config, registry, "jstack", "FE"))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::FeToolAction::FeProfiler => {
                if should_back_to_main(run_tool_by_name(config, registry, "fe-profiler", "FE"))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::FeToolAction::TableInfo => {
                match crate::tools::fe::table_info::browser::run_interactive(config) {
                    Ok(crate::tools::fe::table_info::browser::BrowserAction::BackToFeMenu) => {}
                    Ok(crate::tools::fe::table_info::browser::BrowserAction::ExitApp) => {
                        return Err(error::CliError::UserExit);
                    }
                    Err(e) => {
                        print_error(&format!("Table info browse failed: {e}"));
                    }
                }
            }
            crate::ui::FeToolAction::RoutineLoad => {
                if let Err(e) = handle_routine_load_loop(config, registry) {
                    match e {
                        error::CliError::GracefulExit => return Ok(()),
                        error::CliError::UserExit => return Err(e),
                        _ => print_error(&format!("Routine Load error: {e}")),
                    }
                }
            }
            crate::ui::FeToolAction::FeAuditTopSql => {
                if should_back_to_main(run_tool_by_name(config, registry, "fe-audit-topsql", "FE"))?
                {
                    return Ok(());
                }
                continue;
            }
            crate::ui::FeToolAction::Back => return Ok(()),
        }
    }
}

/// Handle Routine Load sub-menu loop
pub fn handle_routine_load_loop(config: &mut Config, registry: &ToolRegistry) -> Result<()> {
    loop {
        match crate::ui::show_routine_load_menu()? {
            crate::ui::RoutineLoadAction::GetJobId => {
                run_tool_by_name(config, registry, "routine_load_job_lister", "FE")?;
            }
            crate::ui::RoutineLoadAction::Performance => {
                run_tool_by_name(config, registry, "routine_load_performance_analyzer", "FE")?;
            }
            crate::ui::RoutineLoadAction::Traffic => {
                run_tool_by_name(config, registry, "routine_load_traffic_monitor", "FE")?;
            }
            crate::ui::RoutineLoadAction::Back => return Ok(()),
        }
    }
}

/// Handle BE service loop (original logic)
pub fn handle_be_service_loop(config: &mut Config, registry: &ToolRegistry) -> Result<()> {
    loop {
        match crate::ui::show_be_tools_menu()? {
            crate::ui::BeToolAction::BeList => {
                if should_back_to_main(run_tool_by_name(config, registry, "be-list", "BE"))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::BeToolAction::Pstack => {
                if should_back_to_main(run_tool_by_name(config, registry, "pstack", "BE"))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::BeToolAction::BeConfig => loop {
                match crate::ui::show_be_config_menu()? {
                    crate::ui::BeConfigAction::GetVars => {
                        if should_back_to_main(run_tool_by_name(
                            config,
                            registry,
                            "get-be-vars",
                            "BE",
                        ))? {
                            return Ok(());
                        }
                        continue;
                    }
                    crate::ui::BeConfigAction::UpdateConfig => {
                        if should_back_to_main(run_tool_by_name(
                            config,
                            registry,
                            "set-be-config",
                            "BE",
                        ))? {
                            return Ok(());
                        }
                        continue;
                    }
                    crate::ui::BeConfigAction::Back => break,
                }
            },
            crate::ui::BeToolAction::Jmap => {
                if should_back_to_main(run_jmap_submenu_by_names(
                    config,
                    registry,
                    "jmap-dump",
                    "jmap-histo",
                    "BE",
                ))? {
                    return Ok(());
                }
                continue;
            }
            crate::ui::BeToolAction::PipelineTasks => {
                if should_back_to_main(run_tool_by_name(config, registry, "pipeline-tasks", "BE"))?
                {
                    return Ok(());
                }
                continue;
            }
            crate::ui::BeToolAction::Memz => loop {
                match crate::ui::show_memz_menu()? {
                    crate::ui::MemzAction::Current => {
                        if should_back_to_main(run_tool_by_name(config, registry, "memz", "BE"))? {
                            return Ok(());
                        }
                        continue;
                    }
                    crate::ui::MemzAction::Global => {
                        if should_back_to_main(run_tool_by_name(
                            config,
                            registry,
                            "memz-global",
                            "BE",
                        ))? {
                            return Ok(());
                        }
                        continue;
                    }
                    crate::ui::MemzAction::Back => break,
                }
            },
            crate::ui::BeToolAction::Back => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_back_to_main_cases() {
        let graceful =
            should_back_to_main(Err(error::CliError::GracefulExit)).expect("must succeed");
        assert!(graceful, "graceful exit should trigger back-to-main");

        let success = should_back_to_main(Ok(Some(()))).expect("must succeed");
        assert!(!success, "successful execution should keep current menu");

        let user_exit = should_back_to_main(Err(error::CliError::UserExit));
        assert!(
            matches!(user_exit, Err(error::CliError::UserExit)),
            "user exit should be propagated"
        );
    }
}
