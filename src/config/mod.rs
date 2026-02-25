//! Configuration module
//!
//! Handles loading and saving configuration from XDG-compliant paths.

mod paths;

pub use paths::{config_path, model_path, models_dir};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Transcription backend to use (e.g., "whisper", "parakeet")
    pub backend: String,

    /// Whisper model to use (e.g., "base.en", "small.en")
    pub model: String,

    /// Whether to auto-paste after transcription
    pub auto_paste: bool,

    /// Delay before exiting after showing result
    #[serde(with = "humantime_serde")]
    pub exit_delay: Duration,

    /// Path to custom whisper binary (optional)
    pub whisper_path: Option<PathBuf>,

    /// Path to custom models directory (optional)
    pub models_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: "whisper".to_string(),
            model: "base.en".to_string(),
            auto_paste: false,
            exit_delay: Duration::from_secs(2),
            whisper_path: None,
            models_dir: None,
        }
    }
}

impl Config {
    /// Load configuration from the default config file
    pub fn load() -> Result<Self> {
        let config_file = config_path();

        if config_file.exists() {
            let contents = std::fs::read_to_string(&config_file)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save configuration to the default config file
    pub fn save(&self) -> Result<()> {
        let config_file = config_path();

        // Ensure parent directory exists
        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_file, contents)?;
        Ok(())
    }

    /// Get the path to the model file
    pub fn model_path(&self) -> PathBuf {
        model_path(&self.model, self.models_dir.as_deref())
    }

    /// Get the models directory
    pub fn models_dir(&self) -> PathBuf {
        self.models_dir
            .clone()
            .unwrap_or_else(models_dir)
    }
}
