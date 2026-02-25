//! Transcription module
//!
//! Provides the `TranscriptionBackend` trait and implementations for speech-to-text.

mod whisper_cli;

pub use whisper_cli::WhisperCliTranscriber;

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Information about an available model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model name (e.g., "base.en", "tdt-0.6b")
    pub name: &'static str,
    /// Approximate size in bytes
    pub size_bytes: u64,
    /// Human-readable size
    pub size_human: &'static str,
    /// Description
    pub description: &'static str,
}

/// Trait for transcription backend implementations.
///
/// Each backend bundles transcription with its own model management.
#[allow(dead_code)]
pub trait TranscriptionBackend: Send + Sync {
    /// Backend identifier (e.g., "whisper", "parakeet")
    fn name(&self) -> &str;

    /// Transcribe audio file to text
    fn transcribe(&self, audio_path: &Path) -> Result<String>;

    /// List models available for download
    fn available_models(&self) -> &[ModelInfo];

    /// List models already downloaded locally
    fn installed_models(&self, models_dir: &Path) -> Result<Vec<String>>;

    /// Download a model by name to the backend's subdirectory within models_dir
    fn download_model(&self, name: &str, models_dir: &Path) -> Result<PathBuf>;

    /// Resolve a model name to a local path within the backend's subdirectory
    fn resolve_model_path(&self, name: &str, models_dir: &Path) -> Result<PathBuf>;
}

/// Legacy trait kept during migration
#[allow(dead_code)]
pub trait Transcriber {
    fn transcribe(&self, audio_path: &Path) -> Result<String>;
    fn model_name(&self) -> &str;
}
