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
        size_bytes: 2_550_000_000,
        size_human: "~2.5 GB",
        description: "TDT 0.6B - recommended, good speed/accuracy (multilingual)",
    },
    ModelInfo {
        name: "tdt-1.1b",
        size_bytes: 4_300_000_000,
        size_human: "~4.3 GB",
        description: "TDT 1.1B - higher accuracy, slower (multilingual)",
    },
    ModelInfo {
        name: "ctc-0.6b",
        size_bytes: 2_436_000_000,
        size_human: "~2.4 GB",
        description: "CTC 0.6B - English only, fast",
    },
];

/// Download configuration for a model: (repo, list of (remote_path, local_filename))
fn model_download_config(
    name: &str,
) -> Option<(&'static str, &'static [(&'static str, &'static str)])> {
    match name {
        "tdt-0.6b" => Some((
            "istupakov/parakeet-tdt-0.6b-v3-onnx",
            &[
                ("encoder-model.onnx", "encoder-model.onnx"),
                ("encoder-model.onnx.data", "encoder-model.onnx.data"),
                ("decoder_joint-model.onnx", "decoder_joint-model.onnx"),
                ("vocab.txt", "vocab.txt"),
            ],
        )),
        "tdt-1.1b" => Some((
            "dtgagnon/parakeet-tdt-1.1b-onnx",
            &[
                ("encoder-model.onnx", "encoder-model.onnx"),
                ("encoder-model.onnx.data", "encoder-model.onnx.data"),
                ("decoder_joint-model.onnx", "decoder_joint-model.onnx"),
                (
                    "decoder_joint-model.onnx.data",
                    "decoder_joint-model.onnx.data",
                ),
                ("vocab.txt", "vocab.txt"),
            ],
        )),
        "ctc-0.6b" => Some((
            "onnx-community/parakeet-ctc-0.6b-ONNX",
            &[
                ("onnx/model.onnx", "model.onnx"),
                ("onnx/model.onnx_data", "model.onnx_data"),
                ("tokenizer.json", "tokenizer.json"),
            ],
        )),
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
            if path.is_dir() {
                // TDT models have encoder-model.onnx, CTC models have model.onnx
                if path.join("encoder-model.onnx").exists() || path.join("model.onnx").exists() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        models.push(name.to_string());
                    }
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

        let (repo, files) = model_download_config(name)
            .with_context(|| format!("No download configuration for model: {}", name))?;

        let model_dir = models_dir.join("parakeet").join(name);
        std::fs::create_dir_all(&model_dir)?;

        println!(
            "Downloading {} model ({}, {} files)...",
            name,
            info.size_human,
            files.len()
        );
        println!();

        for (remote_path, local_name) in files {
            let dest = model_dir.join(local_name);
            if dest.exists() {
                println!("  {} already exists, skipping", local_name);
                continue;
            }
            let url = format!(
                "https://huggingface.co/{}/resolve/main/{}",
                repo, remote_path
            );
            crate::models::download_file(&url, &dest, local_name, "", 0)?;
        }

        Ok(model_dir)
    }

    fn resolve_model_path(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        Ok(models_dir.join("parakeet").join(name))
    }
}
