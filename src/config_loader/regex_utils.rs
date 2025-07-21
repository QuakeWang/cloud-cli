use once_cell::sync::Lazy;
use regex::Regex;
use std::path::PathBuf;

pub fn extract_env_var(environ_output: &str, key: &str) -> Option<String> {
    let pattern = format!(r"^{}=([^\n]+)", regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    re.captures(environ_output)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_path_from_command(command: &str, binary_name: &str) -> Option<PathBuf> {
    let be_lib_pattern = format!(r"(.*)/be/lib/{}", regex::escape(binary_name));
    let re1 = Regex::new(&be_lib_pattern).ok()?;
    if let Some(caps) = re1.captures(command) {
        return caps.get(1).map(|m| PathBuf::from(m.as_str()));
    }

    let binary_pattern = format!(r"(.*?)/[^/]*{}", regex::escape(binary_name));
    let re2 = Regex::new(&binary_pattern).ok()?;
    if let Some(caps) = re2.captures(command) {
        if let Some(path_match) = caps.get(1) {
            let path = path_match.as_str();
            let path_buf = PathBuf::from(path);
            if path.ends_with("/lib") || path.ends_with("/bin") {
                return path_buf.parent().map(|p| p.to_path_buf());
            }
            return Some(path_buf);
        }
    }

    None
}

pub fn extract_pid_from_output(output: &str, regex_pattern: &str, first_only: bool) -> Option<u32> {
    let re = Regex::new(regex_pattern).ok()?;

    if first_only {
        if let Some(line) = output.lines().next() {
            if let Some(caps) = re.captures(line) {
                if let Some(pid_match) = caps.get(1) {
                    return pid_match.as_str().parse::<u32>().ok();
                }
            }
        }
    } else {
        let current_user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let user_re = Regex::new(r"^(\S+)\s+(\d+)").ok()?;

        for line in output.lines() {
            if let Some(caps) = user_re.captures(line) {
                if let (Some(user_match), Some(pid_match)) = (caps.get(1), caps.get(2)) {
                    let user = user_match.as_str();
                    if user.starts_with(&current_user) || user == current_user {
                        if let Ok(pid) = pid_match.as_str().parse::<u32>() {
                            return Some(pid);
                        }
                    }
                }
            }
        }

        if let Some(line) = output.lines().next() {
            if let Some(caps) = re.captures(line) {
                if let Some(pid_match) = caps.get(1) {
                    return pid_match.as_str().parse::<u32>().ok();
                }
            }
        }
    }

    None
}

pub fn extract_value_from_line(line: &str) -> Option<String> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*[^=\s]+\s*=\s*(.*?)\s*$").unwrap());
    RE.captures(line).and_then(|caps| {
        caps.get(1)
            .map(|m| m.as_str().trim().trim_matches('"').to_string())
    })
}

pub fn extract_key_value(line: &str, key: &str) -> Option<String> {
    let pattern = format!(r"^\s*{}\s*=\s*(.*?)\s*$", regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    re.captures(line).and_then(|caps| {
        caps.get(1)
            .map(|m| m.as_str().trim().trim_matches('"').to_string())
    })
}
