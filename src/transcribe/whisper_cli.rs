//! Whisper CLI wrapper for transcription

use super::Transcriber;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Transcriber that uses the whisper-cpp CLI
#[allow(dead_code)]
pub struct WhisperCliTranscriber {
    /// Path to the whisper binary
    binary_path: PathBuf,
    /// Path to the model file
    model_path: PathBuf,
    /// Model name for display
    model_name: String,
}

impl WhisperCliTranscriber {
    /// Create a new transcriber with explicit paths
    pub fn new(binary_path: PathBuf, model_path: PathBuf) -> Self {
        let model_name = model_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.strip_prefix("ggml-").unwrap_or(s))
            .map(|s| s.strip_suffix(".bin").unwrap_or(s))
            .unwrap_or("unknown")
            .to_string();

        Self {
            binary_path,
            model_path,
            model_name,
        }
    }

    /// Try to find whisper binary and model automatically
    pub fn auto_detect(model_path: PathBuf) -> Result<Self> {
        let binary_path = find_whisper_binary()?;
        Ok(Self::new(binary_path, model_path))
    }
}

impl Transcriber for WhisperCliTranscriber {
    fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let output = Command::new(&self.binary_path)
            .arg("-m")
            .arg(&self.model_path)
            .arg("-f")
            .arg(audio_path)
            .arg("--no-timestamps")
            .arg("-nt") // No timestamps in output text
            .output()
            .context("Failed to run whisper-cli")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("whisper-cli failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// Find the whisper binary in PATH
pub fn find_whisper_binary() -> Result<PathBuf> {
    // Try common binary names (whisper-cli is the NixOS name)
    let candidates = ["whisper-cli", "whisper-cpp", "whisper", "main"];

    for name in candidates {
        if let Ok(path) = which_binary(name) {
            return Ok(path);
        }
    }

    // Check if WHISPER_CPP_PATH is set
    if let Ok(path) = std::env::var("WHISPER_CPP_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "whisper-cli binary not found. Install it via:\n\
         - nix develop (uses flake.nix)\n\
         - Or set WHISPER_CPP_PATH environment variable"
    )
}

fn which_binary(name: &str) -> Result<PathBuf> {
    let output = Command::new("which")
        .arg(name)
        .output()
        .context("Failed to run 'which'")?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        Ok(PathBuf::from(path.trim()))
    } else {
        anyhow::bail!("Binary '{}' not found in PATH", name)
    }
}
