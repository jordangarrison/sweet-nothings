//! Whisper CLI backend for transcription

use super::{ModelInfo, TranscriptionBackend};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Whisper model definitions
static WHISPER_MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tiny.en",
        size_bytes: 75_000_000,
        size_human: "~75 MB",
        description: "Fastest, lowest quality (English only)",
    },
    ModelInfo {
        name: "tiny",
        size_bytes: 75_000_000,
        size_human: "~75 MB",
        description: "Fastest, multilingual",
    },
    ModelInfo {
        name: "base.en",
        size_bytes: 142_000_000,
        size_human: "~142 MB",
        description: "Good balance, recommended (English only)",
    },
    ModelInfo {
        name: "base",
        size_bytes: 142_000_000,
        size_human: "~142 MB",
        description: "Good balance, multilingual",
    },
    ModelInfo {
        name: "small.en",
        size_bytes: 466_000_000,
        size_human: "~466 MB",
        description: "Better quality (English only)",
    },
    ModelInfo {
        name: "small",
        size_bytes: 466_000_000,
        size_human: "~466 MB",
        description: "Better quality, multilingual",
    },
    ModelInfo {
        name: "medium.en",
        size_bytes: 1_500_000_000,
        size_human: "~1.5 GB",
        description: "High quality (English only)",
    },
    ModelInfo {
        name: "medium",
        size_bytes: 1_500_000_000,
        size_human: "~1.5 GB",
        description: "High quality, multilingual",
    },
    ModelInfo {
        name: "large-v3",
        size_bytes: 3_100_000_000,
        size_human: "~3.1 GB",
        description: "Highest quality, multilingual",
    },
];

/// Transcription backend that uses the whisper-cpp CLI
pub struct WhisperCliBackend {
    /// Path to the whisper binary
    binary_path: PathBuf,
    /// Path to the model file
    model_path: PathBuf,
}

impl WhisperCliBackend {
    /// Create a new whisper backend with explicit paths
    pub fn new(binary_path: PathBuf, model_path: PathBuf) -> Self {
        Self {
            binary_path,
            model_path,
        }
    }

    /// Try to find whisper binary automatically and use the given model
    pub fn auto_detect(model_path: PathBuf) -> Result<Self> {
        let binary_path = find_whisper_binary()?;
        Ok(Self::new(binary_path, model_path))
    }

    /// Create from config: find binary (or use custom path), resolve model
    pub fn from_config(
        whisper_path: Option<&Path>,
        model_name: &str,
        models_dir: &Path,
    ) -> Result<Self> {
        let binary_path = match whisper_path {
            Some(p) => p.to_path_buf(),
            None => find_whisper_binary()?,
        };
        let model_path = Self::whisper_model_path(model_name, models_dir);
        Ok(Self::new(binary_path, model_path))
    }

    /// Resolve model name to path: models_dir/whisper/ggml-{name}.bin
    fn whisper_model_path(model_name: &str, models_dir: &Path) -> PathBuf {
        let filename = if model_name.starts_with("ggml-") {
            model_name.to_string()
        } else {
            format!("ggml-{}", model_name)
        };
        let filename = if filename.ends_with(".bin") {
            filename
        } else {
            format!("{}.bin", filename)
        };
        models_dir.join("whisper").join(filename)
    }
}

impl TranscriptionBackend for WhisperCliBackend {
    fn name(&self) -> &str {
        "whisper"
    }

    fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let output = Command::new(&self.binary_path)
            .arg("-m")
            .arg(&self.model_path)
            .arg("-f")
            .arg(audio_path)
            .arg("--no-timestamps")
            .arg("-nt")
            .output()
            .context("Failed to run whisper-cli")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("whisper-cli failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    fn available_models(&self) -> &[ModelInfo] {
        WHISPER_MODELS
    }

    fn installed_models(&self, models_dir: &Path) -> Result<Vec<String>> {
        let whisper_dir = models_dir.join("whisper");
        if !whisper_dir.exists() {
            return Ok(vec![]);
        }

        let mut models = vec![];
        for entry in std::fs::read_dir(&whisper_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "bin") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    let name = name.strip_prefix("ggml-").unwrap_or(name);
                    models.push(name.to_string());
                }
            }
        }
        Ok(models)
    }

    fn download_model(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        let info = WHISPER_MODELS
            .iter()
            .find(|m| m.name == name)
            .with_context(|| format!("Unknown whisper model: {}", name))?;

        let whisper_dir = models_dir.join("whisper");
        std::fs::create_dir_all(&whisper_dir)?;

        let filename = format!("ggml-{}.bin", name);
        let dest_path = whisper_dir.join(&filename);

        if dest_path.exists() {
            println!("Model already exists: {}", dest_path.display());
            return Ok(dest_path);
        }

        let url = format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            filename
        );

        crate::models::download_file(&url, &dest_path, info.name, info.size_human, info.size_bytes)?;
        Ok(dest_path)
    }

    fn resolve_model_path(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        Ok(Self::whisper_model_path(name, models_dir))
    }
}

/// Find the whisper binary in PATH
pub fn find_whisper_binary() -> Result<PathBuf> {
    let candidates = ["whisper-cli", "whisper-cpp", "whisper", "main"];

    for name in candidates {
        if let Ok(path) = which_binary(name) {
            return Ok(path);
        }
    }

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
