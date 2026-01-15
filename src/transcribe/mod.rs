//! Transcription module
//!
//! Provides the `Transcriber` trait and implementations for speech-to-text.

mod whisper_cli;

pub use whisper_cli::WhisperCliTranscriber;

use anyhow::Result;
use std::path::Path;

/// Trait for transcription implementations
#[allow(dead_code)]
pub trait Transcriber {
    /// Transcribe audio file to text
    fn transcribe(&self, audio_path: &Path) -> Result<String>;

    /// Get the name of the model being used
    fn model_name(&self) -> &str;
}
