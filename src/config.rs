use crate::config_loader;
use crate::error::{CliError, Result};
use std::env;
use std::path::PathBuf;

/// Configuration for the cloud-cli application
#[derive(Debug, Clone)]
pub struct Config {
    pub jdk_path: PathBuf,
    pub output_dir: PathBuf,
    pub timeout_seconds: u64,
    pub no_progress_animation: bool,
}

// Environment variable names
const ENV_JDK_PATH: &str = "JDK_PATH";
const ENV_OUTPUT_DIR: &str = "OUTPUT_DIR";
const ENV_TIMEOUT: &str = "CLOUD_CLI_TIMEOUT";
const ENV_NO_PROGRESS: &str = "CLOUD_CLI_NO_PROGRESS";

impl Default for Config {
    fn default() -> Self {
        Self {
            jdk_path: PathBuf::from("/opt/jdk"),
            output_dir: PathBuf::from("/opt/selectdb/information"),
            timeout_seconds: 60,
            no_progress_animation: false,
        }
    }
}

impl Config {
    /// Creates a new configuration instance from dynamic config loader
    /// or falls back to environment variables if that fails
    pub fn new() -> Self {
        // Try to load configuration dynamically, fall back to default if it fails
        let mut config = config_loader::load_config()
            .map(config_loader::to_app_config)
            .unwrap_or_else(|_| Self::default());

        // Allow environment variables to override config
        config.load_from_env();
        config
    }

    /// Loads configuration from environment variables
    fn load_from_env(&mut self) {
        if let Ok(jdk_path) = env::var(ENV_JDK_PATH) {
            self.jdk_path = PathBuf::from(jdk_path);
        }

        if let Ok(output_dir) = env::var(ENV_OUTPUT_DIR) {
            self.output_dir = PathBuf::from(output_dir);
        }

        if let Ok(timeout) = env::var(ENV_TIMEOUT) {
            if let Ok(timeout) = timeout.parse::<u64>() {
                self.timeout_seconds = timeout;
            }
        }

        self.no_progress_animation = env::var(ENV_NO_PROGRESS)
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
    }

    pub fn with_jdk_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.jdk_path = path.into();
        self
    }

    pub fn with_output_dir<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.output_dir = path.into();
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    pub fn with_progress_animation(mut self, enable: bool) -> Self {
        self.no_progress_animation = !enable;
        self
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_jdk_path()?;
        self.validate_output_dir()?;
        self.validate_timeout()?;
        Ok(())
    }

    fn validate_jdk_path(&self) -> Result<()> {
        if !self.jdk_path.exists() {
            return Err(CliError::ConfigError(format!(
                "JDK path does not exist: {}. Set {ENV_JDK_PATH} environment variable or ensure default path exists.",
                self.jdk_path.display()
            )));
        }

        let jmap_path = self.get_jmap_path();
        let jstack_path = self.get_jstack_path();

        if !jmap_path.exists() {
            return Err(CliError::ConfigError(format!(
                "jmap not found: {}. Please verify JDK installation.",
                jmap_path.display()
            )));
        }

        if !jstack_path.exists() {
            return Err(CliError::ConfigError(format!(
                "jstack not found: {}. Please verify JDK installation.",
                jstack_path.display()
            )));
        }

        Ok(())
    }

    fn validate_output_dir(&self) -> Result<()> {
        if self.output_dir.exists() {
            let test_file = self.output_dir.join(".write_test");
            match std::fs::File::create(&test_file) {
                Ok(_) => {
                    let _ = std::fs::remove_file(test_file);
                }
                Err(e) => {
                    return Err(CliError::ConfigError(format!(
                        "Output directory is not writable: {}. Error: {e}",
                        self.output_dir.display()
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_timeout(&self) -> Result<()> {
        if self.timeout_seconds == 0 {
            return Err(CliError::ConfigError("Timeout cannot be zero".to_string()));
        }
        if self.timeout_seconds > 3600 {
            return Err(CliError::ConfigError(
                "Timeout cannot exceed 3600 seconds (1 hour)".to_string(),
            ));
        }
        Ok(())
    }

    pub fn ensure_output_dir(&self) -> Result<()> {
        if let Err(e) = std::fs::create_dir_all(&self.output_dir) {
            return Err(CliError::ConfigError(format!(
                "Failed to create output directory: {}. Error: {e}",
                self.output_dir.display()
            )));
        }
        Ok(())
    }

    pub fn get_jmap_path(&self) -> PathBuf {
        self.jdk_path.join("bin/jmap")
    }

    pub fn get_jstack_path(&self) -> PathBuf {
        self.jdk_path.join("bin/jstack")
    }

    pub fn get_timeout_millis(&self) -> u64 {
        self.timeout_seconds * 1000
    }
}
