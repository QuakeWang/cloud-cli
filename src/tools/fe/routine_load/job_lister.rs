use super::job_manager::RoutineLoadJobManager;
use super::models::RoutineLoadJob;
use crate::config::Config;
use crate::config_loader;
use crate::error::{CliError, Result};
use crate::tools::mysql::MySQLTool;
use crate::tools::{ExecutionResult, Tool};
use crate::ui;
use chrono::Utc;
use console::{Key, Term, style};
use dialoguer::Input;

/// Routine Load Job Lister
pub struct RoutineLoadJobLister;

impl Tool for RoutineLoadJobLister {
    fn name(&self) -> &str {
        "routine_load_job_lister"
    }

    fn description(&self) -> &str {
        "List and select Routine Load jobs"
    }

    fn requires_pid(&self) -> bool {
        false
    }

    fn execute(&self, _config: &Config, _pid: u32) -> Result<ExecutionResult> {
        let database = self.prompt_database_name()?;
        let jobs = self.query_routine_load_jobs(&database)?;
        self.display_jobs(&jobs)?;
        let selected_job = self.prompt_job_selection(&jobs)?;
        self.save_selected_job(selected_job, &database)?;
        // 直接在终端输出关键信息，不再保存文件
        let report = self.generate_selection_report(selected_job)?;
        println!("\n{}", report);
        Ok(ExecutionResult {
            output_path: std::path::PathBuf::from("console_output"),
            message: format!("Job ID '{}' selected and saved in memory", selected_job.id),
        })
    }
}

impl RoutineLoadJobLister {
    fn prompt_database_name(&self) -> Result<String> {
        ui::print_info("Please enter the database name:");

        let database: String = Input::new()
            .with_prompt("Database name")
            .allow_empty(false)
            .interact()?;

        let database = database.trim().to_string();

        if database.is_empty() {
            return Err(CliError::InvalidInput(
                "Database name cannot be empty".into(),
            ));
        }

        Ok(database)
    }

    fn query_routine_load_jobs(&self, database: &str) -> Result<Vec<RoutineLoadJob>> {
        let doris_config = config_loader::load_config()?;

        let sql = format!("USE `{}`; SHOW ALL ROUTINE LOAD \\G", database);
        let output = MySQLTool::query_sql_with_config(&doris_config, &sql)?;

        let job_manager = RoutineLoadJobManager;
        let jobs = job_manager.parse_routine_load_output(&output)?;

        if jobs.is_empty() {
            ui::print_warning(&format!(
                "No Routine Load jobs found in database '{}'",
                database
            ));
            ui::print_info("This could mean:");
            ui::print_info("  - The database name is incorrect");
            ui::print_info("  - No Routine Load jobs have been created");
            ui::print_info("  - All jobs have been stopped or deleted");
            return Err(CliError::ToolExecutionFailed(format!(
                "No Routine Load jobs found in database '{}'",
                database
            )));
        }

        Ok(jobs)
    }

    fn display_jobs(&self, jobs: &[RoutineLoadJob]) -> Result<()> {
        println!("\nRoutine Load Jobs in Database:");
        println!("{}", "=".repeat(100));

        println!(
            "{:<4} {:<20} {:<32} {:<12} {:<15}",
            "No.", "Job ID", "Name", "State", "Table"
        );
        println!("{}", "-".repeat(100));

        for (index, job) in jobs.iter().enumerate() {
            let number = index + 1;
            let name = self.truncate_string(&job.name, 32);
            let table = self.truncate_string(&job.table_name, 13);

            println!(
                "{:<4} {:<20} {:<32} {:<12} {:<15}",
                number, job.id, name, job.state, table
            );
        }

        println!("{}", "=".repeat(100));

        let running_count = jobs.iter().filter(|j| j.state == "RUNNING").count();
        let paused_count = jobs.iter().filter(|j| j.state == "PAUSED").count();
        let stopped_count = jobs.iter().filter(|j| j.state == "STOPPED").count();

        println!(
            "Summary: {} total jobs ({} running, {} paused, {} stopped)",
            jobs.len(),
            running_count,
            paused_count,
            stopped_count
        );

        Ok(())
    }

    fn prompt_job_selection<'a>(&self, jobs: &'a [RoutineLoadJob]) -> Result<&'a RoutineLoadJob> {
        let term = Term::stdout();
        let mut selection: usize = 0;

        println!("\nUse ↑/↓ or press number, then Enter to select:");
        term.hide_cursor()
            .map_err(|e| CliError::InvalidInput(e.to_string()))?;

        self.render_selection_list(&term, jobs, selection)?;

        loop {
            match term
                .read_key()
                .map_err(|e| CliError::InvalidInput(e.to_string()))?
            {
                Key::Enter => {
                    term.show_cursor()
                        .map_err(|e| CliError::InvalidInput(e.to_string()))?;
                    term.clear_last_lines(jobs.len()).ok();
                    break;
                }
                Key::ArrowUp => {
                    selection = if selection == 0 {
                        jobs.len() - 1
                    } else {
                        selection - 1
                    };
                }
                Key::ArrowDown => {
                    selection = if selection + 1 >= jobs.len() {
                        0
                    } else {
                        selection + 1
                    };
                }
                Key::Char(c) => {
                    if let Some(d) = c.to_digit(10) {
                        let idx = d.saturating_sub(1) as usize;
                        if idx < jobs.len() {
                            selection = idx;
                        }
                    }
                }
                _ => {}
            }

            term.move_cursor_up(jobs.len()).ok();
            self.render_selection_list(&term, jobs, selection)?;
        }

        Ok(&jobs[selection])
    }

    fn render_selection_list(
        &self,
        term: &Term,
        jobs: &[RoutineLoadJob],
        selection: usize,
    ) -> Result<()> {
        for (i, job) in jobs.iter().enumerate() {
            let arrow = if i == selection {
                style(">").cyan().bold().to_string()
            } else {
                " ".to_string()
            };
            let name = self.truncate_string(&job.name, 32);
            let line = format!("{arrow} {}. {} - {} ({})", i + 1, job.id, name, job.state);
            term.write_line(&line)
                .map_err(|e| CliError::InvalidInput(e.to_string()))?;
        }
        Ok(())
    }

    fn save_selected_job(&self, job: &RoutineLoadJob, database: &str) -> Result<()> {
        let job_manager = RoutineLoadJobManager;

        job_manager.save_job_id(job.id.clone(), job.name.clone(), database.to_string())?;

        job_manager.update_job_cache(vec![job.clone()])?;

        ui::print_success(&format!("Job ID '{}' saved in memory", job.id));

        Ok(())
    }

    fn generate_selection_report(&self, job: &RoutineLoadJob) -> Result<String> {
        let mut report = String::new();
        report.push_str("Routine Load Job Selection Report\n");
        report.push_str("=================================\n\n");
        report.push_str(&format!("Selected Job ID: {}\n", job.id));
        report.push_str(&format!("Job Name: {}\n", job.name));
        report.push_str(&format!("State: {}\n", job.state));
        report.push_str(&format!("Database: {}\n", job.db_name));
        report.push_str(&format!("Table: {}\n", job.table_name));
        report.push_str(&format!("Create Time: {}\n", job.create_time));

        if let Some(ref pause_time) = job.pause_time {
            report.push_str(&format!("Pause Time: {}\n", pause_time));
        }

        if let Some(ref stat) = job.statistic {
            report.push_str("\nStatistics:\n");
            report.push_str(&format!("  Loaded Rows: {}\n", stat.loaded_rows));
            report.push_str(&format!("  Error Rows: {}\n", stat.error_rows));
            report.push_str(&format!("  Received Bytes: {}\n", stat.received_bytes));
        }

        if let Some(ref progress) = job.progress {
            report.push_str("\nProgress:\n");
            for (partition, offset) in progress {
                report.push_str(&format!("  Partition {partition}: {offset}\n"));
            }
        }

        if let Some(ref lag) = job.lag {
            report.push_str("\nLag:\n");
            for (partition, lag_value) in lag {
                report.push_str(&format!("  Partition {partition}: {lag_value}\n"));
            }
        }

        report.push_str(&format!(
            "\nSelection Time: {}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        Ok(report)
    }

    fn truncate_string(&self, s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }
}
