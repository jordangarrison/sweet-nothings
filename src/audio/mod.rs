//! Audio recording module
//!
//! Provides the `Recorder` trait and implementations for audio capture.

mod recorder;

pub use recorder::CpalRecorder;

use anyhow::Result;
use std::path::PathBuf;

/// Trait for audio recording implementations
pub trait Recorder {
    /// Start recording audio
    fn start(&mut self) -> Result<()>;

    /// Stop recording and return path to the audio file
    fn stop(&mut self) -> Result<PathBuf>;

    /// Check if currently recording
    fn is_recording(&self) -> bool;

    /// Get current audio level (0.0 - 1.0) for visualization
    fn audio_level(&self) -> f32 {
        0.0
    }
}
