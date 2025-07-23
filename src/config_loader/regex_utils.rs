use once_cell::sync::Lazy;
use regex::Regex;

pub fn extract_env_var(environ_output: &str, key: &str) -> Option<String> {
    let pattern = format!(r"(?m)^{}=([^\n]+)", regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    re.captures(environ_output)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_pid_from_output(output: &str, regex_pattern: &str, first_only: bool) -> Option<u32> {
    let re = Regex::new(regex_pattern).ok()?;

    if first_only {
        // Process only the first line
        output
            .lines()
            .next()
            .and_then(|line| re.captures(line))
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
    } else {
        // Process all lines and return the first valid PID
        output.lines().find_map(|line| {
            re.captures(line)
                .and_then(|caps| caps.get(1))
                .and_then(|m| m.as_str().parse::<u32>().ok())
        })
    }
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
