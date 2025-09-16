use crate::error::{CliError, Result};

pub struct InputHelper;

impl InputHelper {
    pub fn prompt_non_empty(prompt: &str) -> Result<String> {
        let input = crate::ui::dialogs::input_text(prompt, "")?;
        let input = input.trim().to_string();
        if input.is_empty() {
            return Err(CliError::InvalidInput("Input cannot be empty".into()));
        }
        Ok(input)
    }

    pub fn prompt_number_with_default(prompt: &str, default: i64, min: i64) -> Result<i64> {
        let input_str = crate::ui::dialogs::input_text(prompt, &default.to_string())?;

        let value: i64 = input_str.trim().parse().unwrap_or(default).max(min);
        Ok(value)
    }
}

pub struct FormatHelper;

impl FormatHelper {
    pub fn fmt_int(v: u64) -> String {
        Self::group_digits(&v.to_string())
    }
    pub fn fmt_int_u128(v: u128) -> String {
        Self::group_digits(&v.to_string())
    }

    fn group_digits(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = String::with_capacity(s.len() + s.len() / 3);
        let mut count = 0;
        for i in (0..bytes.len()).rev() {
            out.push(bytes[i] as char);
            count += 1;
            if count % 3 == 0 && i != 0 {
                out.push(',');
            }
        }
        out.chars().rev().collect()
    }

    pub fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }
}
