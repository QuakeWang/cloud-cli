use crate::error::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// A generic utility to serialize a struct to a TOML file.
pub fn save_toml_to_file<T: serde::Serialize>(obj: &T, file_path: &Path) -> Result<()> {
    let toml_str = toml::to_string_pretty(obj).map_err(|e| {
        crate::error::CliError::ConfigError(format!("Failed to serialize to TOML: {e}"))
    })?;
    ensure_dir_exists(file_path)?;
    fs::write(file_path, toml_str).map_err(|e| {
        crate::error::CliError::ConfigError(format!("Failed to write to file: {e}"))
    })?;
    Ok(())
}

/// Ensures that the directory for a given path exists, creating it if necessary.
pub fn ensure_dir_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|e| {
            crate::error::CliError::ConfigError(format!("Failed to create directory: {e}"))
        })?;
    }
    Ok(())
}

/// Gets the path to the user's configuration directory for this application.
pub fn get_user_config_dir() -> Result<std::path::PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".config").join("cloud-cli"))
        .ok_or_else(|| {
            crate::error::CliError::ConfigError(
                "Could not determine the user's home directory".to_string(),
            )
        })
}

/// Reads the content of a file into a string, with error handling.
pub fn read_file_content(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|e| crate::error::CliError::ConfigError(format!("Failed to read file: {e}")))
}

pub fn collect_log_files(dir: &Path, log_prefix: &str) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Err(crate::error::CliError::ConfigError(format!(
            "Log directory does not exist: {}",
            dir.display()
        )));
    }

    if !dir.is_dir() {
        return Err(crate::error::CliError::ConfigError(format!(
            "Path is not a directory: {}",
            dir.display()
        )));
    }

    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(crate::error::CliError::IoError)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Only accept log files with the specified prefix and exclude compressed archives
            name.starts_with(log_prefix)
                && !name.ends_with(".gz")
                && !name.ends_with(".zip")
                && !name.ends_with(".tar")
                && !name.ends_with(".tar.gz")
        })
        .collect();

    if files.is_empty() {
        return Err(crate::error::CliError::ConfigError(format!(
            "No {} files found in directory: {}",
            log_prefix,
            dir.display()
        )));
    }

    // Sort by modification time (newest first)
    files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    files.reverse();

    Ok(files)
}

pub fn collect_fe_logs(dir: &Path) -> Result<Vec<PathBuf>> {
    collect_log_files(dir, "fe.log")
}

pub fn collect_be_logs(dir: &Path) -> Result<Vec<PathBuf>> {
    collect_log_files(dir, "be.INFO")
}
