//! Shared download utilities for model files

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Download a file from a URL to a destination path with a progress bar.
///
/// This is a shared utility used by all backends for model downloads.
pub fn download_file(
    url: &str,
    dest_path: &Path,
    display_name: &str,
    size_human: &str,
    estimated_size: u64,
) -> Result<()> {
    if size_human.is_empty() {
        println!("Downloading {}...", display_name);
    } else {
        println!("Downloading {} ({})...", display_name, size_human);
    }
    println!("From: {}", url);
    println!("To: {}", dest_path.display());
    println!();

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(url)
        .send()
        .context("Failed to start download")?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("Download failed: HTTP {} for {}", status, url);
    }

    let total_size = response.content_length().unwrap_or(estimated_size);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Download to temp file first
    let temp_path = dest_path.with_extension("part");
    let mut file = File::create(&temp_path)?;
    let mut downloaded: u64 = 0;
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

    std::fs::rename(&temp_path, dest_path)?;

    println!();
    println!("Saved to: {}", dest_path.display());

    Ok(())
}
