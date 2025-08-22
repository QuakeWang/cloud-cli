use crate::error::Result;
use std::fs;
use std::path::Path;

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
