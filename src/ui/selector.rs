use console::{Key, Term, style};

use crate::error::{CliError, Result};

pub trait ItemFormatter<T> {
    fn format_item(&self, item: &T) -> String;
}

pub struct InteractiveSelector<T> {
    items: Vec<T>,
    title: String,
    page_size: usize,
}

impl<T> InteractiveSelector<T> {
    pub fn new(items: Vec<T>, title: String) -> Self {
        Self {
            items,
            title,
            page_size: 30,
        }
    }

    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size.max(1);
        self
    }

    pub fn select(&self) -> Result<&T>
    where
        Self: ItemFormatter<T>,
    {
        if self.items.is_empty() {
            return Err(CliError::InvalidInput("No items to select from".into()));
        }

        let term = Term::stdout();
        let mut selection: usize = 0;
        let mut last_drawn_lines: usize;

        let header_lines = 2usize;
        crate::ui::print_info("");
        crate::ui::print_info(&self.title.to_string());
        crate::ui::print_info("Use ↑/↓, ←/→, 1-9, Enter");

        term.hide_cursor()
            .map_err(|e| CliError::InvalidInput(e.to_string()))?;

        last_drawn_lines = self.render_selection_list(&term, selection)?;

        loop {
            match term
                .read_key()
                .map_err(|e| CliError::InvalidInput(e.to_string()))?
            {
                Key::Enter => {
                    term.show_cursor()
                        .map_err(|e| CliError::InvalidInput(e.to_string()))?;
                    term.clear_last_lines(last_drawn_lines + header_lines + 1)
                        .ok();
                    break;
                }
                Key::ArrowUp => {
                    selection = if selection == 0 {
                        self.items.len() - 1
                    } else {
                        selection - 1
                    };
                }
                Key::ArrowDown => {
                    selection = if selection + 1 >= self.items.len() {
                        0
                    } else {
                        selection + 1
                    };
                }
                Key::ArrowLeft => {
                    if !self.items.is_empty() {
                        let page_size = self.page_size.min(self.items.len()).max(1);
                        let current_page = selection / page_size;
                        if current_page > 0 {
                            selection = (current_page - 1) * page_size;
                        }
                    }
                }
                Key::ArrowRight => {
                    if !self.items.is_empty() {
                        let page_size = self.page_size.min(self.items.len()).max(1);
                        let total_pages = self.items.len().div_ceil(page_size);
                        let current_page = selection / page_size;
                        if current_page + 1 < total_pages {
                            selection = (current_page + 1) * page_size;
                            if selection >= self.items.len() {
                                selection = self.items.len() - 1;
                            }
                        }
                    }
                }
                Key::Char(c) => {
                    if let Some(d) = c.to_digit(10) {
                        let page_size = self.page_size.min(self.items.len()).max(1);
                        let current_page = selection / page_size;
                        let page_start = current_page * page_size;
                        let idx_in_page = d.saturating_sub(1) as usize;
                        let target = page_start + idx_in_page;
                        if target < self.items.len() {
                            selection = target;
                        }
                    }
                }
                _ => {}
            }

            term.clear_last_lines(last_drawn_lines).ok();
            last_drawn_lines = self.render_selection_list(&term, selection)?;
        }

        Ok(&self.items[selection])
    }

    fn render_selection_list(&self, term: &Term, selection: usize) -> Result<usize>
    where
        Self: ItemFormatter<T>,
    {
        let total = self.items.len();
        let page_size = self.page_size.min(total).max(1);
        let total_pages = total.div_ceil(page_size);
        let current_page = selection / page_size;
        let start = current_page * page_size;
        let end = (start + page_size).min(total);

        let mut lines_drawn = 0usize;
        let page_title = format!(
            "Page {}/{}  ({} items)",
            current_page + 1,
            total_pages,
            total
        );
        term.clear_line()?;
        term.write_line(&page_title)
            .map_err(|e| CliError::InvalidInput(e.to_string()))?;
        lines_drawn += 1;

        for (i, item) in self.items[start..end].iter().enumerate() {
            let global_index = start + i;
            term.clear_line()?;
            let arrow = if global_index == selection {
                style(">").cyan().bold().to_string()
            } else {
                " ".to_string()
            };
            let line = format!("{arrow} {}. {}", global_index + 1, self.format_item(item));
            term.write_line(&line)
                .map_err(|e| CliError::InvalidInput(e.to_string()))?;
            lines_drawn += 1;
        }
        Ok(lines_drawn)
    }
}

impl ItemFormatter<String> for InteractiveSelector<String> {
    fn format_item(&self, item: &String) -> String {
        item.clone()
    }
}

impl ItemFormatter<crate::tools::fe::routine_load::RoutineLoadJob>
    for InteractiveSelector<crate::tools::fe::routine_load::RoutineLoadJob>
{
    fn format_item(&self, job: &crate::tools::fe::routine_load::RoutineLoadJob) -> String {
        let name = crate::ui::FormatHelper::truncate_string(&job.name, 32);
        format!("{} - {} ({})", job.id, name, job.state)
    }
}
