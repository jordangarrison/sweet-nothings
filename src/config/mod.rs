//! Configuration module
//!
//! Handles loading and saving configuration from XDG-compliant paths.

mod paths;

pub use paths::{config_path, models_dir};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Transcription backend to use (e.g., "whisper", "parakeet")
    pub backend: String,

    /// Model to use (interpreted by the active backend)
    pub model: String,

    /// Whether to auto-paste after transcription
    pub auto_paste: bool,

    /// Delay before exiting after showing result
    #[serde(with = "humantime_serde")]
    pub exit_delay: Duration,

    /// Path to custom whisper binary (optional, only for whisper backend)
    pub whisper_path: Option<PathBuf>,

    /// Path to custom models directory (optional)
    pub models_dir: Option<PathBuf>,

    /// Words whose spelling should be preferred in transcription output
    pub preferred_words: Vec<String>,
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
            preferred_words: Vec::new(),
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
            config.validate()?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save configuration to the default config file
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let config_file = config_path();

        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_file, contents)?;
        Ok(())
    }

    /// Get the models directory
    pub fn models_dir(&self) -> PathBuf {
        self.models_dir.clone().unwrap_or_else(models_dir)
    }

    /// Validate configuration values that serde cannot constrain.
    pub fn validate(&self) -> Result<()> {
        let mut seen = HashSet::new();

        for word in &self.preferred_words {
            let trimmed = word.trim();
            anyhow::ensure!(!trimmed.is_empty(), "preferred_words cannot contain blanks");

            let folded = trimmed.to_lowercase();
            anyhow::ensure!(
                seen.insert(folded),
                "preferred_words contains a case-insensitive duplicate: {word}"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn preferred_words_default_to_empty() {
        assert!(Config::default().preferred_words.is_empty());
    }

    #[test]
    fn old_config_without_preferred_words_still_deserializes() {
        let config: Config = toml::from_str(
            r#"
backend = "whisper"
model = "base.en"
auto_paste = false
exit_delay = "2s"
"#,
        )
        .unwrap();

        assert!(config.preferred_words.is_empty());
        config.validate().unwrap();
    }

    #[test]
    fn preferred_words_round_trip_through_toml() {
        let config = Config {
            preferred_words: vec!["Mikayla".into(), "Isla".into()],
            ..Config::default()
        };

        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.preferred_words, ["Mikayla", "Isla"]);
    }

    #[test]
    fn blank_preferred_word_is_rejected() {
        let config = Config {
            preferred_words: vec![" ".into()],
            ..Config::default()
        };

        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "preferred_words cannot contain blanks"
        );
    }

    #[test]
    fn case_insensitive_duplicate_preferred_word_is_rejected() {
        let config = Config {
            preferred_words: vec!["Mikayla".into(), "mikayla".into()],
            ..Config::default()
        };

        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("case-insensitive duplicate"));
    }
}
