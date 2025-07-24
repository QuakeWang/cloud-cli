use crate::config::Config;
use crate::error::Result;
use crate::tools::ExecutionResult;
use crate::ui;
use std::fs;
use std::path::PathBuf;
use chrono::Utc;

/// Configuration for handling BE API responses
pub struct BeResponseHandler<'a> {
    pub success_message: &'a str,
    pub empty_warning: &'a str,
    pub error_context: &'a str,
    pub tips: &'a str,
}

impl<'a> BeResponseHandler<'a> {
    /// Handle response for console-only output (like be_vars)
    pub fn handle_console_result(
        &self,
        result: Result<String>,
        context: &str,
    ) -> Result<ExecutionResult> {
        match result {
            Ok(output) => {
                ui::print_success(self.success_message);
                println!();
                ui::print_info("Results:");

                if output.is_empty() {
                    ui::print_warning(&self.empty_warning.replace("{}", context));
                } else {
                    println!("{output}");
                }

                Ok(ExecutionResult {
                    output_path: PathBuf::from("console_output"),
                    message: format!("Query completed for: {context}"),
                })
            }
            Err(e) => {
                ui::print_error(&format!("{}: {e}.", self.error_context));
                ui::print_info(&format!("Tips: {}", self.tips));
                Err(e)
            }
        }
    }

    /// Handle response with file output (like pipeline_tasks)
    pub fn handle_file_result<F>(
        &self,
        config: &Config,
        result: Result<String>,
        file_prefix: &str,
        summary_fn: F,
    ) -> Result<ExecutionResult>
    where
        F: Fn(&str) -> String,
    {
        match result {
            Ok(output) => {
                ui::print_success(self.success_message);
                println!();
                ui::print_info("Results:");

                if output.trim().is_empty() {
                    ui::print_warning(self.empty_warning);

                    Ok(ExecutionResult {
                        output_path: PathBuf::from("console_output"),
                        message: "No data found".to_string(),
                    })
                } else {
                    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");

                    let filename = format!("{}_{}.txt", file_prefix, timestamp);
                    let output_path = config.output_dir.join(filename);

                    fs::write(&output_path, &output)?;

                    println!("{}", summary_fn(&output));

                    let message = format!(
                        "{} saved to {}",
                        file_prefix.replace('_', " ").to_title_case(),
                        output_path.display()
                    );

                    Ok(ExecutionResult {
                        output_path,
                        message,
                    })
                }
            }
            Err(e) => {
                ui::print_error(&format!("{}: {e}.", self.error_context));
                ui::print_info(&format!("Tips: {}", self.tips));
                Err(e)
            }
        }
    }
}

trait ToTitleCase {
    fn to_title_case(&self) -> String;
}

impl ToTitleCase for str {
    fn to_title_case(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
