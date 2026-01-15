//! Model download functionality

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

/// Information about an available model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model name (e.g., "base.en")
    pub name: &'static str,
    /// Filename on disk
    pub filename: &'static str,
    /// Approximate size in bytes
    pub size_bytes: u64,
    /// Human-readable size
    pub size_human: &'static str,
    /// Description
    pub description: &'static str,
}

/// Available models for download
pub static AVAILABLE_MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tiny.en",
        filename: "ggml-tiny.en.bin",
        size_bytes: 75_000_000,
        size_human: "~75 MB",
        description: "Fastest, lowest quality (English only)",
    },
    ModelInfo {
        name: "tiny",
        filename: "ggml-tiny.bin",
        size_bytes: 75_000_000,
        size_human: "~75 MB",
        description: "Fastest, multilingual",
    },
    ModelInfo {
        name: "base.en",
        filename: "ggml-base.en.bin",
        size_bytes: 142_000_000,
        size_human: "~142 MB",
        description: "Good balance, recommended (English only)",
    },
    ModelInfo {
        name: "base",
        filename: "ggml-base.bin",
        size_bytes: 142_000_000,
        size_human: "~142 MB",
        description: "Good balance, multilingual",
    },
    ModelInfo {
        name: "small.en",
        filename: "ggml-small.en.bin",
        size_bytes: 466_000_000,
        size_human: "~466 MB",
        description: "Better quality (English only)",
    },
    ModelInfo {
        name: "small",
        filename: "ggml-small.bin",
        size_bytes: 466_000_000,
        size_human: "~466 MB",
        description: "Better quality, multilingual",
    },
    ModelInfo {
        name: "medium.en",
        filename: "ggml-medium.en.bin",
        size_bytes: 1_500_000_000,
        size_human: "~1.5 GB",
        description: "High quality (English only)",
    },
    ModelInfo {
        name: "medium",
        filename: "ggml-medium.bin",
        size_bytes: 1_500_000_000,
        size_human: "~1.5 GB",
        description: "High quality, multilingual",
    },
    ModelInfo {
        name: "large-v3",
        filename: "ggml-large-v3.bin",
        size_bytes: 3_100_000_000,
        size_human: "~3.1 GB",
        description: "Highest quality, multilingual",
    },
];

/// Get model info by name
pub fn get_model_info(name: &str) -> Option<&'static ModelInfo> {
    AVAILABLE_MODELS.iter().find(|m| m.name == name)
}

/// Download a model to the specified directory
pub fn download_model(model_name: &str, dest_dir: &Path) -> Result<()> {
    let info = get_model_info(model_name)
        .with_context(|| format!("Unknown model: {}", model_name))?;

    // Create destination directory
    std::fs::create_dir_all(dest_dir)?;

    let dest_path = dest_dir.join(info.filename);

    // Check if already exists
    if dest_path.exists() {
        println!("Model already exists: {:?}", dest_path);
        return Ok(());
    }

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        info.filename
    );

    println!("Downloading {} ({})...", info.name, info.size_human);
    println!("From: {}", url);
    println!("To: {:?}", dest_path);
    println!();

    // Download with progress bar
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .send()
        .context("Failed to start download")?;

    let total_size = response.content_length().unwrap_or(info.size_bytes);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Create a temporary file first
    let temp_path = dest_path.with_extension("bin.part");
    let mut file = File::create(&temp_path)?;
    let mut downloaded: u64 = 0;

    // Read in chunks
    let mut reader = response;
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("Download complete!");

    // Rename to final destination
    std::fs::rename(&temp_path, &dest_path)?;

    println!();
    println!("Model saved to: {:?}", dest_path);

    Ok(())
}

/// Prompt user to download a model if not present
/// Returns true if user wants to proceed (model exists or was downloaded)
pub fn prompt_download(model_name: &str, dest_dir: &Path) -> Result<bool> {
    let info = match get_model_info(model_name) {
        Some(info) => info,
        None => {
            eprintln!("Unknown model: {}", model_name);
            return Ok(false);
        }
    };

    let dest_path = dest_dir.join(info.filename);

    if dest_path.exists() {
        return Ok(true);
    }

    println!("Model '{}' not found.", model_name);
    println!();
    println!("Download '{}' now? ({}) [Y/n]: ", info.name, info.size_human);

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() || input == "y" || input == "yes" {
        download_model(model_name, dest_dir)?;
        Ok(true)
    } else {
        println!();
        println!("To download manually, run:");
        println!("  sweet-nothings models download {}", model_name);
        Ok(false)
    }
}
