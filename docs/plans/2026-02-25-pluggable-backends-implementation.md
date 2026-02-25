# Pluggable Transcription Backends Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor Sweet Nothings from a Whisper-only dictation tool into a pluggable transcription backend system, then add Parakeet as the first additional backend.

**Architecture:** A `TranscriptionBackend` trait replaces the existing `Transcriber` trait, bundling transcription with model management. Backends are compile-time Cargo features. A registry maps backend names to implementations. Config and CLI gain a `backend` field. A shared Nix module supports both NixOS and Home Manager.

**Tech Stack:** Rust, parakeet-rs (ONNX Runtime), clap, serde/toml, Nix flakes

**Design doc:** `docs/plans/2026-02-25-pluggable-transcription-backends-design.md`

---

### Task 1: Add `backend` field to Config

**Files:**
- Modify: `src/config/mod.rs`

**Step 1: Add backend field to Config struct**

In `src/config/mod.rs`, add a `backend` field to the `Config` struct. It defaults to `"whisper"` for backwards compatibility with existing config files.

```rust
/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Transcription backend to use (e.g., "whisper", "parakeet")
    pub backend: String,

    /// Model to use (interpreted by the active backend)
    pub model: String,

    /// Whether to auto-paste after transcription
    pub auto_paste: bool,

    /// Delay before exiting after showing result
    #[serde(with = "humantime_serde")]
    pub exit_delay: Duration,

    /// Path to custom whisper binary (optional, only for whisper backend)
    pub whisper_path: Option<PathBuf>,

    /// Path to custom models directory (optional)
    pub models_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: "whisper".to_string(),
            model: "base.en".to_string(),
            auto_paste: false,
            exit_delay: Duration::from_secs(2),
            whisper_path: None,
            models_dir: None,
        }
    }
}
```

**Step 2: Add `backend` to config get/set in `src/main.rs`**

In `handle_config_command`, add handling for the `"backend"` key in both `Get` and `Set` arms:

```rust
// In ConfigAction::Get:
"backend" => println!("{}", config.backend),

// In ConfigAction::Set:
"backend" => config.backend = value,
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles with no errors. Existing configs without `backend` field still load (serde default kicks in).

**Step 4: Commit**

```bash
git add src/config/mod.rs src/main.rs
git commit -m "feat: add backend field to config"
```

---

### Task 2: Create TranscriptionBackend trait and move ModelInfo

**Files:**
- Modify: `src/transcribe/mod.rs`
- Modify: `src/models/mod.rs`
- Modify: `src/models/download.rs`

**Step 1: Define the new trait and ModelInfo in `src/transcribe/mod.rs`**

Replace the existing `Transcriber` trait with `TranscriptionBackend`. Move `ModelInfo` from `src/models/download.rs` to `src/transcribe/mod.rs` so it's shared across all backends. Keep the old `Transcriber` trait temporarily (it's unused outside `whisper_cli.rs`).

```rust
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

/// Legacy trait kept for backwards compatibility during migration
#[allow(dead_code)]
pub trait Transcriber {
    fn transcribe(&self, audio_path: &Path) -> Result<String>;
    fn model_name(&self) -> &str;
}
```

**Step 2: Update `src/models/download.rs` to use shared ModelInfo**

Change `ModelInfo` in `download.rs` to import from `crate::transcribe::ModelInfo` and remove the local struct definition. Since the old `ModelInfo` had a `filename` field that the new one doesn't (filename is now backend-internal), keep the old struct as `WhisperModelInfo` temporarily or just keep `filename` as a local concern in the whisper backend.

For now, **don't change `download.rs` yet** — it will be fully refactored in Task 5 when we rebuild model management. Just add the new trait to `mod.rs`.

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles. The new trait exists but nothing implements it yet.

**Step 4: Commit**

```bash
git add src/transcribe/mod.rs
git commit -m "feat: add TranscriptionBackend trait with ModelInfo"
```

---

### Task 3: Implement TranscriptionBackend for Whisper

**Files:**
- Modify: `src/transcribe/whisper_cli.rs`
- Modify: `src/transcribe/mod.rs`

**Step 1: Refactor WhisperCliTranscriber to implement TranscriptionBackend**

Rename `WhisperCliTranscriber` to `WhisperCliBackend` and implement the full `TranscriptionBackend` trait. The Whisper backend owns its model list and download logic. Keep `find_whisper_binary()` and `which_binary()` as internal helpers.

Replace the entire contents of `src/transcribe/whisper_cli.rs`:

```rust
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
    /// Create a new whisper backend with a specific model loaded
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
```

**Step 2: Update `src/transcribe/mod.rs` exports**

```rust
//! Transcription module
//!
//! Provides the `TranscriptionBackend` trait and implementations for speech-to-text.

mod whisper_cli;

pub use whisper_cli::WhisperCliBackend;

use anyhow::Result;
use std::path::{Path, PathBuf};

// ... (ModelInfo and TranscriptionBackend trait from Task 2, remove old Transcriber trait)
```

Remove the old `Transcriber` trait and the `WhisperCliTranscriber` re-export.

**Step 3: Build and verify**

Run: `cargo build`
Expected: Build errors in `src/tui/app.rs` and `src/main.rs` because they still reference `WhisperCliTranscriber` and old `Transcriber`. That's expected — we'll fix those in later tasks.

**Step 4: Commit**

```bash
git add src/transcribe/
git commit -m "feat: implement TranscriptionBackend for whisper CLI"
```

---

### Task 4: Create the backend registry

**Files:**
- Create: `src/transcribe/registry.rs`
- Modify: `src/transcribe/mod.rs`

**Step 1: Create `src/transcribe/registry.rs`**

```rust
//! Backend registry
//!
//! Maps backend names to implementations, gated by feature flags.

use super::TranscriptionBackend;
use super::WhisperCliBackend;
use anyhow::{bail, Result};
use std::path::Path;

/// List all backend names that are compiled in
pub fn available_backend_names() -> Vec<&'static str> {
    let mut backends = vec!["whisper"];
    #[cfg(feature = "parakeet")]
    backends.push("parakeet");
    backends
}

/// Create a backend instance by name.
///
/// For whisper: `model_path` is the resolved path to the ggml model file.
/// The whisper binary is auto-detected.
pub fn create_backend(
    backend_name: &str,
    model_path: &std::path::PathBuf,
) -> Result<Box<dyn TranscriptionBackend>> {
    match backend_name {
        "whisper" => {
            let backend = WhisperCliBackend::auto_detect(model_path.clone())?;
            Ok(Box::new(backend))
        }
        #[cfg(feature = "parakeet")]
        "parakeet" => {
            let backend = super::parakeet::ParakeetBackend::new(model_path)?;
            Ok(Box::new(backend))
        }
        _ => {
            let available = available_backend_names().join(", ");
            bail!(
                "Unknown backend: '{}'. Available backends: {}",
                backend_name,
                available
            )
        }
    }
}
```

**Step 2: Register in `src/transcribe/mod.rs`**

Add `mod registry;` and `pub use registry::{available_backend_names, create_backend};` to the module file.

Full updated `src/transcribe/mod.rs`:

```rust
//! Transcription module
//!
//! Provides the `TranscriptionBackend` trait and implementations for speech-to-text.

mod registry;
mod whisper_cli;

#[cfg(feature = "parakeet")]
mod parakeet;

pub use registry::{available_backend_names, create_backend};
pub use whisper_cli::WhisperCliBackend;

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
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: May warn about the `#[cfg(feature = "parakeet")] mod parakeet;` since the file doesn't exist yet. If so, remove that line for now and add it back in Task 9. Otherwise compiles.

**Step 4: Commit**

```bash
git add src/transcribe/
git commit -m "feat: add backend registry with feature-gated dispatch"
```

---

### Task 5: Refactor models module into shared download utility

**Files:**
- Modify: `src/models/mod.rs`
- Modify: `src/models/download.rs`

**Step 1: Convert `src/models/download.rs` into a shared download utility**

Remove the hardcoded `AVAILABLE_MODELS`, `ModelInfo` struct, and `get_model_info()`. Keep `download_file()` as a general-purpose HTTP download function with progress bar. Remove `prompt_download()` — prompting will be handled in `main.rs` using the backend trait.

```rust
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
    println!("Downloading {} ({})...", display_name, size_human);
    println!("From: {}", url);
    println!("To: {}", dest_path.display());
    println!();

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(url)
        .send()
        .context("Failed to start download")?;

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
    println!("Model saved to: {}", dest_path.display());

    Ok(())
}
```

**Step 2: Update `src/models/mod.rs`**

```rust
//! Model management module
//!
//! Provides shared download utilities used by transcription backends.

mod download;

pub use download::download_file;
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Build errors in `src/main.rs` because it still references `models::AVAILABLE_MODELS`, `models::download_model`, and `models::prompt_download`. That's expected — we fix `main.rs` in Task 7.

**Step 4: Commit**

```bash
git add src/models/
git commit -m "refactor: convert models module to shared download utility"
```

---

### Task 6: Update CLI with --backend flag

**Files:**
- Modify: `src/cli/mod.rs`

**Step 1: Add `--backend` to Cli struct and ModelsAction variants**

```rust
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
```

**Step 2: Build and verify**

Run: `cargo build`
Expected: Build errors in `main.rs` due to `ModelsAction` variants now having fields. Expected — fixed in Task 7.

**Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add --backend flag to CLI and models subcommands"
```

---

### Task 7: Update config/paths.rs and main.rs to use backend system

**Files:**
- Modify: `src/config/paths.rs`
- Modify: `src/config/mod.rs`
- Modify: `src/main.rs`

**Step 1: Update `src/config/paths.rs`**

Remove the whisper-specific `model_path()` function. The `models_dir()` function stays. Path resolution is now the backend's job.

```rust
//! XDG-compliant path resolution

use directories::ProjectDirs;
use std::path::PathBuf;

const APP_NAME: &str = "sweet-nothings";

/// Get the project directories for XDG paths
fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", APP_NAME)
}

/// Get the configuration file path
/// Returns: $XDG_CONFIG_HOME/sweet-nothings/config.toml
pub fn config_path() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config/sweet-nothings/config.toml")
        })
}

/// Get the data directory
/// Returns: $XDG_DATA_HOME/sweet-nothings
pub fn data_dir() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".local/share/sweet-nothings")
        })
}

/// Get the models directory
/// Returns: $XDG_DATA_HOME/sweet-nothings/models
pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}
```

**Step 2: Update `src/config/mod.rs`**

Remove the `model_path()` method from `Config` (backends handle path resolution now). Keep `models_dir()`.

```rust
pub use paths::{config_path, models_dir};
```

Remove the `model_path` import from `pub use paths::{...}` and remove the `model_path` method from the `Config` impl:

```rust
impl Config {
    pub fn load() -> Result<Self> {
        let config_file = config_path();
        if config_file.exists() {
            let contents = std::fs::read_to_string(&config_file)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_file = config_path();
        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_file, contents)?;
        Ok(())
    }

    /// Get the models directory
    pub fn models_dir(&self) -> PathBuf {
        self.models_dir
            .clone()
            .unwrap_or_else(models_dir)
    }
}
```

**Step 3: Rewrite `src/main.rs`**

This is the biggest single change. The main function now:
1. Resolves the backend name from CLI or config
2. Creates a backend-agnostic model resolver for prompt-to-download
3. Uses the backend trait for model commands

```rust
//! Sweet Nothings - Terminal-based dictation tool

mod audio;
mod cli;
mod clipboard;
mod config;
mod models;
mod transcribe;
mod tui;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Commands, ConfigAction, ModelsAction};
use config::Config;
use transcribe::{available_backend_names, TranscriptionBackend, WhisperCliBackend};

fn main() -> Result<()> {
    let args = Cli::parse();

    // Handle subcommands
    if let Some(command) = args.command {
        return handle_command(command, &args);
    }

    // Load config, applying CLI overrides
    let mut config = Config::load()?;

    if let Some(backend) = args.backend {
        config.backend = backend;
    }
    if args.model != "base.en" {
        config.model = args.model;
    }
    if args.paste {
        config.auto_paste = true;
    }
    if let Some(delay) = args.exit_delay {
        config.exit_delay = delay;
    }
    if let Some(path) = args.whisper_path {
        config.whisper_path = Some(path);
    }
    if let Some(dir) = args.models_dir {
        config.models_dir = Some(dir);
    }

    // Resolve model path via backend
    let backend = resolve_backend_for_model_check(&config)?;
    let models_dir = config.models_dir();
    let model_path = backend.resolve_model_path(&config.model, &models_dir)?;

    // Check for model, prompt to download if missing
    if !model_path.exists() {
        let info = backend.available_models().iter().find(|m| m.name == config.model);
        if let Some(info) = info {
            println!("Model '{}' not found.", config.model);
            println!();
            println!(
                "Download '{}' now? ({}) [Y/n]: ",
                info.name, info.size_human
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();
            if input.is_empty() || input == "y" || input == "yes" {
                backend.download_model(&config.model, &models_dir)?;
            } else {
                println!();
                println!("To download manually, run:");
                println!(
                    "  sweet-nothings models download {} --backend {}",
                    config.model,
                    config.backend
                );
                std::process::exit(1);
            }
        } else {
            anyhow::bail!(
                "Unknown model '{}' for backend '{}'. Run 'sweet-nothings models available --backend {}' to see options.",
                config.model,
                config.backend,
                config.backend
            );
        }
    }

    // Run the TUI
    tui::run(&config)
}

/// Create a backend instance just for model resolution (not transcription).
/// This is used before the TUI starts to check if models exist.
fn resolve_backend_for_model_check(config: &Config) -> Result<Box<dyn TranscriptionBackend>> {
    match config.backend.as_str() {
        "whisper" => {
            // For model checking, we need a backend but not necessarily a working binary.
            // Use auto_detect which may fail if whisper isn't installed, but that's OK
            // because we just need resolve_model_path and available_models.
            // Create a minimal instance with a dummy model path for now.
            let models_dir = config.models_dir();
            let model_path = models_dir.join("whisper").join("dummy");
            let backend = WhisperCliBackend::auto_detect(model_path)
                .or_else(|_| {
                    // If whisper binary not found, create with a placeholder path
                    // We only need model resolution, not transcription
                    Ok::<_, anyhow::Error>(WhisperCliBackend::new(
                        std::path::PathBuf::from("whisper-cli"),
                        models_dir.join("whisper").join("dummy"),
                    ))
                })?;
            Ok(Box::new(backend))
        }
        #[cfg(feature = "parakeet")]
        "parakeet" => {
            // Will be implemented in Task 9
            anyhow::bail!("Parakeet backend not yet implemented")
        }
        _ => {
            let available = available_backend_names().join(", ");
            anyhow::bail!(
                "Unknown backend: '{}'. Available: {}",
                config.backend,
                available
            )
        }
    }
}

fn handle_command(command: Commands, args: &Cli) -> Result<()> {
    match command {
        Commands::Models { action } => handle_models_command(action, args),
        Commands::Config { action } => handle_config_command(action),
    }
}

fn handle_models_command(action: ModelsAction, args: &Cli) -> Result<()> {
    let config = Config::load()?;

    match action {
        ModelsAction::List { backend } => {
            let backend_name = backend
                .as_deref()
                .or(args.backend.as_deref())
                .unwrap_or(&config.backend);
            let models_dir = config.models_dir();

            let backend = resolve_backend_for_model_check(&Config {
                backend: backend_name.to_string(),
                ..config
            })?;

            println!("Backend: {}", backend_name);
            println!("Models directory: {}", models_dir.display());
            println!();

            let installed = backend.installed_models(&models_dir)?;
            if installed.is_empty() {
                println!("No models installed.");
            } else {
                for model in &installed {
                    println!("  {}", model);
                }
            }
            Ok(())
        }
        ModelsAction::Available { backend } => {
            let backend_name = backend
                .as_deref()
                .or(args.backend.as_deref())
                .unwrap_or(&config.backend);

            let backend = resolve_backend_for_model_check(&Config {
                backend: backend_name.to_string(),
                ..config
            })?;

            println!("Available models for '{}' backend:", backend_name);
            println!();
            for model in backend.available_models() {
                println!(
                    "  {:12} ({:>8}) - {}",
                    model.name, model.size_human, model.description
                );
            }
            println!();
            println!(
                "Download with: sweet-nothings models download <model> --backend {}",
                backend_name
            );
            Ok(())
        }
        ModelsAction::Download { model, backend } => {
            let backend_name = backend
                .as_deref()
                .or(args.backend.as_deref())
                .unwrap_or(&config.backend);
            let models_dir = config.models_dir();

            let backend = resolve_backend_for_model_check(&Config {
                backend: backend_name.to_string(),
                ..config
            })?;

            backend.download_model(&model, &models_dir)?;
            Ok(())
        }
    }
}

fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = Config::load()?;
            println!("{}", toml::to_string_pretty(&config)?);
            Ok(())
        }
        ConfigAction::Path => {
            println!("{}", config::config_path().display());
            Ok(())
        }
        ConfigAction::Get { key } => {
            let config = Config::load()?;
            match key.as_str() {
                "backend" => println!("{}", config.backend),
                "model" => println!("{}", config.model),
                "auto_paste" => println!("{}", config.auto_paste),
                "exit_delay" => println!("{:?}", config.exit_delay),
                "whisper_path" => println!("{:?}", config.whisper_path),
                "models_dir" => println!("{:?}", config.models_dir),
                _ => anyhow::bail!("Unknown config key: {}", key),
            }
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut config = Config::load()?;
            match key.as_str() {
                "backend" => config.backend = value,
                "model" => config.model = value,
                "auto_paste" => {
                    config.auto_paste = value.parse().context("Invalid boolean value")?
                }
                "exit_delay" => {
                    config.exit_delay =
                        humantime::parse_duration(&value).context("Invalid duration")?
                }
                _ => anyhow::bail!("Unknown or read-only config key: {}", key),
            }
            config.save()?;
            println!("Configuration updated.");
            Ok(())
        }
    }
}
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Build errors in `src/tui/app.rs` — it still references `WhisperCliTranscriber`. Fixed in Task 8.

**Step 5: Commit**

```bash
git add src/config/ src/main.rs
git commit -m "refactor: wire up backend system in config and main"
```

---

### Task 8: Update TUI to use backend system

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`

**Step 1: Update `src/tui/app.rs` to use TranscriptionBackend**

Replace the direct `WhisperCliTranscriber` usage with the backend trait. The `App` stores config needed to create a backend in the background thread.

```rust
//! App state machine

use anyhow::Result;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::audio::{CpalRecorder, Recorder};
use crate::clipboard::{Clipboard, SystemClipboard};
use crate::config::Config;
use crate::transcribe;

/// App state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Recording,
    Transcribing,
    Done,
    Error,
}

/// Main application
pub struct App {
    state: AppState,
    recorder: CpalRecorder,
    config: Config,
    clipboard: SystemClipboard,
    recording_start: Option<Instant>,
    audio_path: Option<PathBuf>,
    transcription_rx: Option<Receiver<Result<String>>>,
    transcription: Option<String>,
    error: Option<String>,
    result_shown_at: Option<Instant>,
    should_paste_on_exit: bool,
}

impl App {
    pub fn new(
        recorder: CpalRecorder,
        clipboard: SystemClipboard,
        config: Config,
    ) -> Self {
        Self {
            state: AppState::Recording,
            recorder,
            config,
            clipboard,
            recording_start: None,
            audio_path: None,
            transcription_rx: None,
            transcription: None,
            error: None,
            result_shown_at: None,
            should_paste_on_exit: false,
        }
    }

    pub fn state(&self) -> AppState {
        self.state
    }

    pub fn is_recording(&self) -> bool {
        self.state == AppState::Recording
    }

    #[allow(dead_code)]
    pub fn is_transcribing(&self) -> bool {
        self.state == AppState::Transcribing
    }

    pub fn is_done(&self) -> bool {
        self.state == AppState::Done
    }

    pub fn is_error(&self) -> bool {
        self.state == AppState::Error
    }

    pub fn recording_duration(&self) -> Duration {
        self.recording_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    pub fn audio_level(&self) -> f32 {
        self.recorder.audio_level()
    }

    pub fn transcription(&self) -> Option<&str> {
        self.transcription.as_deref()
    }

    pub fn transcription_result(&self) -> Option<&str> {
        self.transcription.as_deref()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[allow(dead_code)]
    pub fn model_name(&self) -> &str {
        &self.config.model
    }

    pub fn should_exit(&self) -> bool {
        if self.state == AppState::Done {
            if let Some(shown_at) = self.result_shown_at {
                return shown_at.elapsed() >= self.config.exit_delay;
            }
        }
        false
    }

    pub fn should_paste_on_exit(&self) -> bool {
        self.should_paste_on_exit
    }

    pub fn do_paste(&self) -> Result<()> {
        self.clipboard.paste()
    }

    pub fn start_recording(&mut self) -> Result<()> {
        self.recorder.start()?;
        self.recording_start = Some(Instant::now());
        self.state = AppState::Recording;
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<()> {
        let audio_path = self.recorder.stop()?;
        self.audio_path = Some(audio_path.clone());
        self.state = AppState::Transcribing;

        // Clone what the background thread needs
        let backend_name = self.config.backend.clone();
        let model_name = self.config.model.clone();
        let models_dir = self.config.models_dir();
        let whisper_path = self.config.whisper_path.clone();

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let result = (|| -> Result<String> {
                // Create backend in the thread
                let backend: Box<dyn transcribe::TranscriptionBackend> = match backend_name.as_str() {
                    "whisper" => {
                        let model_path = transcribe::WhisperCliBackend::from_config(
                            whisper_path.as_deref(),
                            &model_name,
                            &models_dir,
                        )?;
                        Box::new(model_path)
                    }
                    #[cfg(feature = "parakeet")]
                    "parakeet" => {
                        let model_path = models_dir.join("parakeet").join(&model_name);
                        let backend = transcribe::create_backend("parakeet", &model_path)?;
                        backend
                    }
                    _ => anyhow::bail!("Unknown backend: {}", backend_name),
                };
                backend.transcribe(&audio_path)
            })();
            let _ = tx.send(result);
        });

        self.transcription_rx = Some(rx);
        Ok(())
    }

    pub fn check_transcription_result(&mut self) {
        if let Some(rx) = &self.transcription_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(text) => {
                        if let Err(e) = self.clipboard.copy(&text) {
                            self.error = Some(format!("Failed to copy to clipboard: {}", e));
                            self.state = AppState::Error;
                            return;
                        }
                        if self.config.auto_paste {
                            self.should_paste_on_exit = true;
                        }
                        self.transcription = Some(text);
                        self.state = AppState::Done;
                        self.result_shown_at = Some(Instant::now());
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        self.state = AppState::Error;
                    }
                }
                if let Some(path) = &self.audio_path {
                    let _ = std::fs::remove_file(path);
                }
                self.transcription_rx = None;
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(path) = &self.audio_path {
            let _ = std::fs::remove_file(path);
        }
    }
}
```

**Step 2: Build and verify the full project compiles**

Run: `cargo build`
Expected: Clean compile. The entire backend refactoring is now wired up for the whisper backend.

**Step 3: Test manually (if whisper-cli is available)**

Run: `cargo run -- models available`
Expected: Lists the 9 whisper models.

Run: `cargo run -- config show`
Expected: Shows config including `backend = "whisper"`.

**Step 4: Commit**

```bash
git add src/tui/
git commit -m "refactor: update TUI to use backend trait for transcription"
```

---

### Task 9: Add Cargo feature flags and implement Parakeet backend

**Files:**
- Modify: `Cargo.toml`
- Create: `src/transcribe/parakeet.rs`
- Modify: `src/transcribe/mod.rs` (ensure `#[cfg(feature = "parakeet")] mod parakeet;` is present)

**Step 1: Add feature flags and dependencies to `Cargo.toml`**

```toml
[package]
name = "sweet-nothings"
version = "0.1.0"
edition = "2021"
description = "Terminal-based dictation tool"
license = "MIT"

[features]
default = ["whisper"]
whisper = []
parakeet = ["dep:parakeet-rs"]

[dependencies]
# Audio capture
cpal = "0.15"
hound = "3.5"

# Clipboard
arboard = { version = "3", features = ["wayland-data-control"] }

# CLI
clap = { version = "4", features = ["derive"] }

# TUI
ratatui = "0.28"
crossterm = "0.28"

# Model downloads
reqwest = { version = "0.12", features = ["blocking", "stream"] }
indicatif = "0.17"

# Configuration
serde = { version = "1", features = ["derive"] }
toml = "0.8"
directories = "5"
humantime = "2"
humantime-serde = "1"

# Error handling
anyhow = "1"
thiserror = "1"

# Parakeet backend (optional)
parakeet-rs = { version = "0.1", optional = true }
```

Note: Check the latest version of `parakeet-rs` on crates.io before implementing. The version may differ.

**Step 2: Create `src/transcribe/parakeet.rs`**

```rust
//! Parakeet backend for transcription
//!
//! Uses parakeet-rs (ONNX Runtime) for native Rust speech-to-text.

use super::{ModelInfo, TranscriptionBackend};
use anyhow::{Context, Result};
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
        // Determine if this is a TDT or CTC model based on the name
        if self.model_name.starts_with("tdt") {
            let mut model = parakeet_rs::ParakeetTDT::from_pretrained(
                self.model_dir.to_str().unwrap_or("."),
                None,
            )
            .context("Failed to load Parakeet TDT model")?;

            let result = model
                .transcribe_file(audio_path.to_str().unwrap_or(""))
                .context("Parakeet TDT transcription failed")?;

            Ok(result.text)
        } else {
            let mut model = parakeet_rs::Parakeet::from_pretrained(
                self.model_dir.to_str().unwrap_or("."),
                None,
            )
            .context("Failed to load Parakeet CTC model")?;

            let result = model
                .transcribe_file(audio_path.to_str().unwrap_or(""), None)
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
                // Check if model.onnx exists in the subdirectory
                if path.join("model.onnx").exists() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        models.push(name.to_string());
                    }
                }
            }
        }
        Ok(models)
    }

    fn download_model(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        let _info = PARAKEET_MODELS
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
                "ONNX model",
                _info.size_bytes,
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
                "tokenizer",
                1_000_000, // ~1MB estimate
            )?;
        }

        Ok(model_dir)
    }

    fn resolve_model_path(&self, name: &str, models_dir: &Path) -> Result<PathBuf> {
        Ok(models_dir.join("parakeet").join(name))
    }
}
```

**Note:** The exact `parakeet-rs` API (especially `ParakeetTDT::transcribe_file` signature) should be verified against the actual crate docs when implementing. The `transcribe_file` method on `ParakeetTDT` may not take a `TimestampMode` parameter (the context7 docs showed it without one for TDT). Adjust accordingly.

**Step 3: Ensure `src/transcribe/mod.rs` has the cfg-gated module**

Verify this line is present:
```rust
#[cfg(feature = "parakeet")]
mod parakeet;
```

And export it in the registry. This was already done in Task 4.

**Step 4: Update the registry to handle parakeet model path**

In `src/transcribe/registry.rs`, the `#[cfg(feature = "parakeet")]` arm should work since `parakeet::ParakeetBackend::new()` takes a `&Path`.

**Step 5: Build with default features (whisper only)**

Run: `cargo build`
Expected: Compiles. Parakeet code is gated behind feature flag.

**Step 6: Build with parakeet feature**

Run: `cargo build --features parakeet`
Expected: Downloads `parakeet-rs` and `ort` crates, compiles. If there are API mismatches with `parakeet-rs`, adjust the code to match the actual API.

**Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/transcribe/parakeet.rs src/transcribe/mod.rs
git commit -m "feat: add parakeet transcription backend behind feature flag"
```

---

### Task 10: Update flake.nix with parameterized build

**Files:**
- Modify: `flake.nix`

**Step 1: Add parameterized build function to flake.nix**

Update the flake to support building with different feature sets. Add `onnxruntime` to build inputs when parakeet is enabled.

```nix
{
  description = "Sweet Nothings - Terminal-based dictation tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Build dependencies (always needed)
        baseBuildInputs = with pkgs; [
          alsa-lib
          openssl
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
        ];

        # Native build dependencies
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
          makeWrapper
        ];

        # Runtime deps per backend
        whisperRuntimeDeps = with pkgs; [ whisper-cpp ];
        parakeetRuntimeDeps = with pkgs; [ onnxruntime ];

        commonRuntimeDeps = with pkgs; [
          wtype
          xdotool
          wl-clipboard
          xclip
        ];

        # Parameterized build function
        buildSweetNothings = { features ? [ "whisper" ] }:
          let
            hasWhisper = builtins.elem "whisper" features;
            hasParakeet = builtins.elem "parakeet" features;
            featureFlags = builtins.concatStringsSep "," features;
            runtimeDeps = commonRuntimeDeps
              ++ (if hasWhisper then whisperRuntimeDeps else [])
              ++ (if hasParakeet then parakeetRuntimeDeps else []);
            extraBuildInputs = if hasParakeet then [ pkgs.onnxruntime ] else [];
          in
          pkgs.rustPlatform.buildRustPackage {
            pname = "sweet-nothings";
            version = "0.1.0";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };

            buildInputs = baseBuildInputs ++ extraBuildInputs;
            inherit nativeBuildInputs;

            buildNoDefaultFeatures = true;
            buildFeatures = features;

            postInstall = ''
              wrapProgram $out/bin/sweet-nothings \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
            '';

            meta = with pkgs.lib; {
              description = "Terminal-based dictation tool";
              license = licenses.mit;
            };
          };

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = baseBuildInputs;
          inherit nativeBuildInputs;

          packages = with pkgs; [
            whisper-cpp
          ] ++ commonRuntimeDeps ++ whisperRuntimeDeps;

          shellHook = ''
            echo "Sweet Nothings dev shell"
            echo "========================"
            echo "whisper-cli: $(which whisper-cli)"
            echo ""
          '';

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          PKG_CONFIG_PATH = "${pkgs.alsa-lib.dev}/lib/pkgconfig";
        };

        packages = {
          default = buildSweetNothings { features = [ "whisper" ]; };
          full = buildSweetNothings { features = [ "whisper" "parakeet" ]; };
          whisper-only = buildSweetNothings { features = [ "whisper" ]; };
          parakeet-only = buildSweetNothings { features = [ "parakeet" ]; };
        };
      }
    );
}
```

**Step 2: Verify flake evaluates**

Run: `nix flake check --no-build`
Expected: Flake evaluates without errors. (Actual builds may require ONNX Runtime to be available.)

**Step 3: Commit**

```bash
git add flake.nix
git commit -m "feat: parameterized nix build with backend feature flags"
```

---

### Task 11: Add Nix module for NixOS and Home Manager

**Files:**
- Create: `nix/module.nix`
- Modify: `flake.nix` (add module outputs)

**Step 1: Create `nix/module.nix`**

```nix
# Shared module for NixOS and Home Manager
# Works in both contexts by using programs.* namespace
flake:
{ config, lib, pkgs, ... }:

let
  cfg = config.programs.sweet-nothings;
  system = pkgs.stdenv.hostPlatform.system;
  sweetPkgs = flake.packages.${system};

  # Build the package with selected backends
  package = (flake.lib.${system}.buildSweetNothings or
    (args: sweetPkgs.default)) { features = cfg.backends; };

  # Generate config.toml content
  configContent = lib.generators.toINI {} {
    "" = {
      backend = cfg.defaultBackend;
      model = cfg.model;
    } // cfg.settings;
  };

  tomlContent = ''
    backend = "${cfg.defaultBackend}"
    model = "${cfg.model}"
    auto_paste = ${lib.boolToString (cfg.settings.auto_paste or false)}
    exit_delay = "${cfg.settings.exit_delay or "2s"}"
  '';

in
{
  options.programs.sweet-nothings = {
    enable = lib.mkEnableOption "sweet-nothings dictation tool";

    package = lib.mkOption {
      type = lib.types.package;
      default = package;
      description = "The sweet-nothings package to use";
    };

    backends = lib.mkOption {
      type = lib.types.listOf (lib.types.enum [ "whisper" "parakeet" ]);
      default = [ "whisper" ];
      description = "Transcription backends to compile in";
    };

    defaultBackend = lib.mkOption {
      type = lib.types.str;
      default = "whisper";
      description = "Default backend when not specified via CLI";
    };

    model = lib.mkOption {
      type = lib.types.str;
      default = "base.en";
      description = "Default model name";
    };

    settings = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = {};
      description = "Additional config.toml settings";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."sweet-nothings/config.toml" = lib.mkIf (cfg.settings != {} || cfg.defaultBackend != "whisper" || cfg.model != "base.en") {
      text = tomlContent;
    };
  };
}
```

**Note:** This module uses `home.packages` and `xdg.configFile` which are Home Manager-specific. For pure NixOS, you'd use `environment.systemPackages` instead. A more robust implementation would detect the context. For the initial version, target Home Manager since this is a user-level tool. Add NixOS support in a follow-up if needed.

**Step 2: Add module outputs to `flake.nix`**

Add these to the outputs (outside `eachDefaultSystem` since modules aren't system-specific, or inside with system-specific package references):

```nix
# Add to flake outputs, inside eachDefaultSystem block:
homeManagerModules.default = import ./nix/module.nix self;
```

Since `eachDefaultSystem` wraps everything per-system, and modules need to work across systems, you may need to restructure slightly. The module receives `pkgs` from the consumer's system, so it resolves `flake.packages.${system}` at evaluation time.

For simplicity, add the module output **outside** `eachDefaultSystem`:

```nix
outputs = { self, nixpkgs, flake-utils, rust-overlay }:
  {
    homeManagerModules.default = import ./nix/module.nix self;
    nixosModules.default = import ./nix/module.nix self;
  } //
  flake-utils.lib.eachDefaultSystem (system:
    # ... existing per-system outputs
  );
```

**Step 3: Verify flake evaluates**

Run: `nix flake check --no-build`
Expected: Evaluates without errors.

**Step 4: Commit**

```bash
git add nix/module.nix flake.nix
git commit -m "feat: add Nix module for NixOS and Home Manager integration"
```

---

### Task 12: Model directory migration for existing whisper models

**Files:**
- Modify: `src/main.rs`

**Step 1: Add migration logic for existing model files**

Existing users have models in `~/.local/share/sweet-nothings/models/ggml-*.bin` (flat directory). The new layout puts them in `models/whisper/`. Add a one-time migration at startup.

Add this function to `src/main.rs`:

```rust
/// Migrate models from flat directory to backend subdirectories.
/// Moves ggml-*.bin files from models/ to models/whisper/.
fn migrate_model_directory(models_dir: &std::path::Path) -> Result<()> {
    // Check if there are ggml-*.bin files directly in models_dir
    if !models_dir.exists() {
        return Ok(());
    }

    let whisper_dir = models_dir.join("whisper");
    let mut migrated = false;

    for entry in std::fs::read_dir(models_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("ggml-") && name.ends_with(".bin") {
                    std::fs::create_dir_all(&whisper_dir)?;
                    let dest = whisper_dir.join(name);
                    if !dest.exists() {
                        std::fs::rename(&path, &dest)?;
                        if !migrated {
                            println!("Migrating models to new directory layout...");
                            migrated = true;
                        }
                        println!("  Moved {} -> whisper/{}", name, name);
                    }
                }
            }
        }
    }

    if migrated {
        println!("Migration complete.");
        println!();
    }

    Ok(())
}
```

Call `migrate_model_directory(&config.models_dir())` early in `main()`, before model resolution.

**Step 2: Build and verify**

Run: `cargo build`
Expected: Compiles.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add one-time migration for model directory layout"
```

---

### Task 13: Final integration test

**Step 1: Verify default build (whisper only)**

Run: `cargo build`
Expected: Clean compile with only whisper feature.

**Step 2: Verify full build (whisper + parakeet)**

Run: `cargo build --features parakeet`
Expected: Clean compile with both features.

**Step 3: Verify CLI commands**

Run the following and verify output:

```bash
cargo run -- config show
# Should show backend = "whisper"

cargo run -- models available
# Should list 9 whisper models

cargo run -- models available --backend whisper
# Same as above

cargo run -- --backend nonexistent models available
# Should error: "Unknown backend: 'nonexistent'"
```

With parakeet feature:
```bash
cargo run --features parakeet -- models available --backend parakeet
# Should list parakeet models (tdt-0.6b, tdt-1.1b, ctc-0.6b, ctc-1.1b)
```

**Step 4: Commit any fixes**

If any issues are found during testing, fix and commit:

```bash
git add -A
git commit -m "fix: integration test fixes for pluggable backends"
```
