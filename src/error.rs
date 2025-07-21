use dialoguer;
use std::fmt;

#[derive(Debug)]
pub enum CliError {
    ProcessNotFound(String),
    ProcessExecutionFailed(String),
    ToolExecutionFailed(String),
    IoError(std::io::Error),
    InvalidInput(String),
    ConfigError(String),
    GracefulExit,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::ProcessNotFound(msg) => write!(f, "Process not found: {msg}"),
            CliError::ProcessExecutionFailed(msg) => write!(f, "Process execution failed: {msg}"),
            CliError::ToolExecutionFailed(msg) => write!(f, "Tool execution failed: {msg}"),
            CliError::IoError(err) => write!(f, "IO error: {err}"),
            CliError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            CliError::ConfigError(msg) => write!(f, "Configuration error: {msg}"),
            CliError::GracefulExit => write!(f, "Graceful exit"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::IoError(err)
    }
}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        CliError::ToolExecutionFailed(err.to_string())
    }
}

impl From<dialoguer::Error> for CliError {
    fn from(err: dialoguer::Error) -> Self {
        CliError::InvalidInput(err.to_string())
    }
}

impl From<toml::ser::Error> for CliError {
    fn from(err: toml::ser::Error) -> Self {
        CliError::ConfigError(format!("Failed to serialize config: {err}"))
    }
}

impl From<toml::de::Error> for CliError {
    fn from(err: toml::de::Error) -> Self {
        CliError::ConfigError(format!("Failed to parse config: {err}"))
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
