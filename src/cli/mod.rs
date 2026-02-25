//! CLI module
//!
//! Command-line interface using clap.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

/// Sweet Nothings - Terminal-based dictation tool
#[derive(Parser, Debug)]
#[command(name = "sweet-nothings")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Transcription backend to use (e.g., "whisper", "parakeet")
    #[arg(short, long)]
    pub backend: Option<String>,

    /// Model to use (interpreted by the active backend)
    #[arg(short, long, default_value = "base.en")]
    pub model: String,

    /// Transcribe an audio file directly (skip recording TUI)
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Auto-paste transcription after completion
    #[arg(short, long)]
    pub paste: bool,

    /// Delay before exiting after showing result (e.g., "2s", "500ms")
    #[arg(long, value_parser = parse_duration)]
    pub exit_delay: Option<Duration>,

    /// Path to whisper-cli binary (only for whisper backend)
    #[arg(long)]
    pub whisper_path: Option<PathBuf>,

    /// Path to models directory
    #[arg(long)]
    pub models_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage transcription models
    Models {
        #[command(subcommand)]
        action: ModelsAction,
    },
    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModelsAction {
    /// List installed models
    List {
        /// Backend to list models for (defaults to configured backend)
        #[arg(long)]
        backend: Option<String>,
    },
    /// Show available models for download
    Available {
        /// Backend to show models for (defaults to configured backend)
        #[arg(long)]
        backend: Option<String>,
    },
    /// Download a model
    Download {
        /// Model name (e.g., "base.en", "tdt-0.6b")
        model: String,
        /// Backend the model belongs to (defaults to configured backend)
        #[arg(long)]
        backend: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Show configuration file path
    Path,
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| e.to_string())
}
