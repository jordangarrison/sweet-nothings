//! Parakeet backend for transcription
//!
//! Uses parakeet-rs (ONNX Runtime) for native Rust speech-to-text.

use super::{ModelInfo, TranscriptionBackend};
use anyhow::{Context, Result};
use parakeet_rs::Transcriber as _;
use std::path::{Path, PathBuf};

/// Parakeet model definitions
static PARAKEET_MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tdt-0.6b",
        size_bytes: 600_000_000,
        size_human: "~600 MB",
        description: "TDT 0.6B - recommended, good speed/accuracy (multilingual)",
    },
    ModelInfo {
        name: "tdt-1.1b",
        size_bytes: 1_100_000_000,
        size_human: "~1.1 GB",
        description: "TDT 1.1B - higher accuracy, slower (multilingual)",
    },
    ModelInfo {
        name: "ctc-0.6b",
        size_bytes: 600_000_000,
        size_human: "~600 MB",
        description: "CTC 0.6B - English only, fast",
    },
    ModelInfo {
        name: "ctc-1.1b",
        size_bytes: 1_100_000_000,
        size_human: "~1.1 GB",
        description: "CTC 1.1B - English only, higher accuracy",
    },
];

/// Map model name to its Hugging Face repo for ONNX download
fn model_hf_repo(name: &str) -> Option<&'static str> {
    match name {
        "tdt-0.6b" => Some("nvidia/parakeet-tdt-0.6b-v2"),
        "tdt-1.1b" => Some("nvidia/parakeet-tdt-1.1b"),
        "ctc-0.6b" => Some("nvidia/parakeet-ctc-0.6b"),
        "ctc-1.1b" => Some("nvidia/parakeet-ctc-1.1b"),
        _ => None,
    }
}

/// Transcription backend using parakeet-rs (ONNX Runtime)
pub struct ParakeetBackend {
    /// Model directory path
    model_dir: PathBuf,
    /// Model variant name
    model_name: String,
}

impl ParakeetBackend {
    /// Create a new Parakeet backend pointing at a model directory
    pub fn new(model_dir: &Path) -> Result<Self> {
        let model_name = model_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            model_dir: model_dir.to_path_buf(),
            model_name,
        })
    }
}

impl TranscriptionBackend for ParakeetBackend {
    fn name(&self) -> &str {
        "parakeet"
    }

    fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let audio_str = audio_path
            .to_str()
            .context("Audio path is not valid UTF-8")?;
        let model_str = self
            .model_dir
            .to_str()
            .context("Model path is not valid UTF-8")?;

        if self.model_name.starts_with("tdt") {
            let mut model = parakeet_rs::ParakeetTDT::from_pretrained(model_str, None)
                .context("Failed to load Parakeet TDT model")?;

            let result = model
                .transcribe_file(audio_str, None)
                .context("Parakeet TDT transcription failed")?;

            Ok(result.text)
        } else {
            let mut model = parakeet_rs::Parakeet::from_pretrained(model_str, None)
                .context("Failed to load Parakeet CTC model")?;

            let result = model
                .transcribe_file(audio_str, None)
                .context("Parakeet CTC transcription failed")?;

            Ok(result.text)
        }
    }

    fn available_models(&self) -> &[ModelInfo] {
        PARAKEET_MODELS
    }

    fn installed_models(&self, models_dir: &Path) -> Result<Vec<String>> {
        let parakeet_dir = models_dir.join("parakeet");
        if !parakeet_dir.exists() {
            return Ok(vec![]);
        }

        let mut models = vec![];
        for entry in std::fs::read_dir(&parakeet_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && path.join("model.onnx").exists() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    models.push(name.to_string());
                }
            }
        }
        Ok(models)
    }

    fn download_model(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        let info = PARAKEET_MODELS
            .iter()
            .find(|m| m.name == name)
            .with_context(|| format!("Unknown parakeet model: {}", name))?;

        let repo = model_hf_repo(name)
            .with_context(|| format!("No download URL for model: {}", name))?;

        let model_dir = models_dir.join("parakeet").join(name);
        std::fs::create_dir_all(&model_dir)?;

        // Download model.onnx
        let onnx_path = model_dir.join("model.onnx");
        if !onnx_path.exists() {
            let url = format!(
                "https://huggingface.co/{}/resolve/main/model.onnx",
                repo
            );
            crate::models::download_file(
                &url,
                &onnx_path,
                &format!("{} (model)", name),
                info.size_human,
                info.size_bytes,
            )?;
        }

        // Download tokenizer.json
        let tokenizer_path = model_dir.join("tokenizer.json");
        if !tokenizer_path.exists() {
            let url = format!(
                "https://huggingface.co/{}/resolve/main/tokenizer.json",
                repo
            );
            crate::models::download_file(
                &url,
                &tokenizer_path,
                &format!("{} (tokenizer)", name),
                "~1 MB",
                1_000_000,
            )?;
        }

        Ok(model_dir)
    }

    fn resolve_model_path(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        Ok(models_dir.join("parakeet").join(name))
    }
}
