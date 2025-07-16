use crate::error::{CliError, Result};
use crate::tools::Tool;
use crate::ui;
use console::{Key, Term, style};
use dialoguer::Select;

use super::{format_menu_item, print_step};

struct MenuOption<T> {
    action: T,
    key: String,
    name: String,
    description: String,
}

struct Menu<T> {
    step: u8,
    title: String,
    options: Vec<MenuOption<T>>,
}

impl<T: Copy> Menu<T> {
    fn show(&self) -> Result<T> {
        let items: Vec<String> = self
            .options
            .iter()
            .map(|o| format_menu_item(&o.key, &o.name, &o.description))
            .collect();

        let selection = show_interactive_menu(self.step, &self.title, &items)?;
        Ok(self.options[selection].action)
    }
}

fn show_interactive_menu(step: u8, title: &str, items: &[String]) -> Result<usize> {
    let term = Term::stdout();
    let mut selection = 0;

    if step > 0 {
        print_step(step, title);
    } else if !title.is_empty() {
        println!("{}", style(title).bold());
    }

    term.hide_cursor()?;

    for (i, item) in items.iter().enumerate() {
        let line = if i == selection {
            format!("{} {}", style(">").cyan().bold(), style(item).cyan())
        } else {
            format!("  {item}")
        };
        term.write_line(&line)?;
    }

    loop {
        let key = term.read_key()?;

        match key {
            Key::Enter => {
                term.show_cursor()?;
                term.clear_last_lines(items.len())?;
                return Ok(selection);
            }
            Key::ArrowUp => {
                selection = if selection == 0 {
                    items.len() - 1
                } else {
                    selection - 1
                }
            }
            Key::ArrowDown => {
                selection = if selection == items.len() - 1 {
                    0
                } else {
                    selection + 1
                }
            }
            Key::Char(c) => {
                if let Some(digit) = c.to_digit(10) {
                    let index = (digit as usize).saturating_sub(1);
                    if index < items.len() {
                        selection = index;
                    }
                }
            }
            _ => {} // Ignore other keys
        }

        // Redraw in place
        term.move_cursor_up(items.len())?;
        for (i, item) in items.iter().enumerate() {
            term.clear_line()?;
            let line = if i == selection {
                format!("{} {}", style(">").cyan().bold(), style(item).cyan())
            } else {
                format!("  {item}")
            };
            term.write_line(&line)?;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MainMenuAction {
    Fe,
    Be,
    Exit,
}

pub fn show_main_menu() -> Result<MainMenuAction> {
    let menu = Menu {
        step: 1,
        title: "Select service type".to_string(),
        options: vec![
            MenuOption {
                action: MainMenuAction::Fe,
                key: "[1]".to_string(),
                name: "FE".to_string(),
                description: "Frontend operations".to_string(),
            },
            MenuOption {
                action: MainMenuAction::Be,
                key: "[2]".to_string(),
                name: "BE".to_string(),
                description: "Backend operations".to_string(),
            },
            MenuOption {
                action: MainMenuAction::Exit,
                key: "[3]".to_string(),
                name: "Exit".to_string(),
                description: "Exit the application".to_string(),
            },
        ],
    };
    menu.show()
}

pub fn show_tool_selection_menu<'a>(
    step: u8,
    title: &str,
    tools: &'a [Box<dyn Tool>],
) -> Result<Option<&'a dyn Tool>> {
    let mut items: Vec<String> = tools
        .iter()
        .enumerate()
        .map(|(i, tool)| format_menu_item(&format!("[{}]", i + 1), tool.name(), tool.description()))
        .collect();

    let back_index = items.len();
    items.push(format_menu_item(
        &format!("[{}]", back_index + 1),
        "← Back",
        "Return to main menu",
    ));
    let exit_index = items.len();
    items.push(format_menu_item(
        &format!("[{}]", exit_index + 1),
        "Exit",
        "Exit the application",
    ));

    let selection = show_interactive_menu(step, title, &items)?;

    if selection < tools.len() {
        Ok(Some(&*tools[selection]))
    } else if selection == back_index {
        Ok(None)
    } else {
        ui::print_goodbye();
        std::process::exit(0);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PostExecutionAction {
    Continue,
    BackToMain,
    Exit,
}

pub fn show_post_execution_menu(tool_name: &str) -> Result<PostExecutionAction> {
    let menu = Menu {
        step: 4,
        title: format!("{tool_name} completed - What's next?"),
        options: vec![
            MenuOption {
                action: PostExecutionAction::Continue,
                key: "[1]".to_string(),
                name: "Continue".to_string(),
                description: "Run another tool".to_string(),
            },
            MenuOption {
                action: PostExecutionAction::BackToMain,
                key: "[2]".to_string(),
                name: "← Back to Main".to_string(),
                description: "Return to service selection".to_string(),
            },
            MenuOption {
                action: PostExecutionAction::Exit,
                key: "[3]".to_string(),
                name: "Exit".to_string(),
                description: "Exit the application".to_string(),
            },
        ],
    };
    menu.show()
}

pub fn ask_continue(prompt: &str) -> Result<bool> {
    println!();
    let options = vec!["Yes", "No"];
    let selection = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| CliError::InvalidInput(format!("Continue selection failed: {e}")))?;
    Ok(selection == 0)
}
