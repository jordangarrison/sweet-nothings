# Sweet Nothings (Rust) - MVP Implementation Plan

A cross-platform, terminal-based dictation tool powered by whisper. Record speech via hotkey, transcribe locally, copy to clipboard.

## Overview

Sweet Nothings is a TUI application that:
1. Launches via hotkey (user configures in their window manager)
2. Immediately begins recording audio
3. User presses Enter to stop recording
4. Transcribes audio locally via whisper
5. Copies transcription to clipboard (optionally auto-pastes)
6. Exits after brief delay

The terminal IS the UI. Single binary distribution. Works anywhere.

## Why Rust for This Project

- **Single binary**: All dependencies compiled in (except whisper models)
- **cpal**: True cross-platform audio capture without shelling out
- **arboard**: Cross-platform clipboard without shelling out
- **Compiler feedback**: Strict type system helps coding agents self-correct
- **whisper-rs**: Option to embed whisper.cpp via bindings

## Architecture Principles

- **Trait-driven**: Platform-specific code behind traits for easy porting
- **Offline-first**: No cloud dependencies, all processing local
- **Minimal friction**: Auto-downloads models, sensible defaults, just works
- **Single binary**: No runtime dependencies except model files

## Project Structure

```
sweet-nothings/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, clap setup
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── record.rs        # Record command (default)
│   │   ├── models.rs        # Models subcommand
│   │   └── config.rs        # Config subcommand
│   ├── audio/
│   │   ├── mod.rs           # Recorder trait + cpal implementation
│   │   └── recorder.rs
│   ├── transcribe/
│   │   ├── mod.rs           # Transcriber trait
│   │   └── whisper.rs       # whisper-rs or CLI wrapper
│   ├── clipboard/
│   │   ├── mod.rs           # Clipboard trait + arboard
│   │   ├── paste_linux.rs   # Linux paste simulation
│   │   └── paste_macos.rs   # macOS paste simulation
│   ├── config/
│   │   ├── mod.rs           # Config struct, loading, XDG paths
│   │   └── paths.rs
│   ├── models/
│   │   └── download.rs      # Model download with progress
│   └── tui/
│       ├── mod.rs           # ratatui app
│       ├── app.rs           # Main app state machine
│       └── ui.rs            # Rendering
├── flake.nix
└── README.md
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "sweet-nothings"
version = "0.1.0"
edition = "2021"

[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# TUI
ratatui = "0.28"
crossterm = "0.28"

# Audio
cpal = "0.15"
hound = "3.5"  # WAV encoding

# Clipboard
arboard = "3"

# Transcription (choose one approach)
# Option A: whisper-rs bindings
whisper-rs = "0.11"
# Option B: shell out (remove whisper-rs, add this)
# (no dep needed, just std::process::Command)

# Config
serde = { version = "1", features = ["derive"] }
toml = "0.8"
directories = "5"  # XDG paths

# Model downloads
reqwest = { version = "0.12", features = ["blocking", "stream"] }
indicatif = "0.17"  # Progress bars

# Error handling
anyhow = "1"
thiserror = "1"

# Misc
tokio = { version = "1", features = ["rt", "macros"] }  # Only if async needed
```

## Core Traits

### Recorder

```rust
// src/audio/mod.rs

use std::path::PathBuf;
use anyhow::Result;

pub trait Recorder: Send {
    /// Start recording audio. Called from a separate thread.
    fn start(&mut self) -> Result<()>;
    
    /// Stop recording and return path to WAV file.
    fn stop(&mut self) -> Result<PathBuf>;
    
    /// Check if currently recording.
    fn is_recording(&self) -> bool;
    
    /// Get current audio level (0.0 - 1.0) for visualization.
    /// Optional - return 0.0 if not implemented.
    fn audio_level(&self) -> f32 {
        0.0
    }
}
```

### Transcriber

```rust
// src/transcribe/mod.rs

use std::path::Path;
use anyhow::Result;

pub trait Transcriber: Send {
    /// Transcribe audio file to text.
    fn transcribe(&self, audio_path: &Path) -> Result<String>;
}

// For future streaming support (not MVP)
pub trait StreamingTranscriber: Transcriber {
    fn transcribe_stream(
        &self, 
        audio_path: &Path
    ) -> Result<std::sync::mpsc::Receiver<String>>;
}
```

### Clipboard

```rust
// src/clipboard/mod.rs

use anyhow::Result;

pub trait Clipboard: Send {
    /// Copy text to system clipboard.
    fn copy(&mut self, text: &str) -> Result<()>;
    
    /// Simulate paste keystroke (Ctrl+V / Cmd+V).
    fn paste(&self) -> Result<()>;
    
    /// Whether paste simulation is supported on this platform.
    fn supports_paste(&self) -> bool;
}
```

## Configuration

### Config Struct

```rust
// src/config/mod.rs

use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Directory where whisper models are stored.
    /// Default: $XDG_DATA_HOME/sweet-nothings/models/
    pub model_path: Option<String>,
    
    /// Whisper model to use (e.g., "base.en", "small.en").
    pub model: String,
    
    /// Automatically simulate Ctrl+V after copying transcription.
    pub auto_paste: bool,
    
    /// How long to show the result before exiting.
    /// Set to 0 for immediate exit.
    #[serde(with = "humantime_serde")]
    pub exit_delay: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model_path: None,
            model: "base.en".to_string(),
            auto_paste: false,
            exit_delay: Duration::from_millis(1500),
        }
    }
}
```

Add to Cargo.toml:
```toml
humantime-serde = "1"
```

### Path Resolution

```rust
// src/config/paths.rs

use directories::ProjectDirs;
use std::path::PathBuf;

pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "sweet-nothings")
}

/// Returns model directory, checking in order:
/// 1. SWEET_NOTHINGS_MODEL_PATH env var
/// 2. Config file model_path value  
/// 3. $XDG_DATA_HOME/sweet-nothings/models/
pub fn model_dir(config: &Config) -> PathBuf {
    if let Ok(path) = std::env::var("SWEET_NOTHINGS_MODEL_PATH") {
        return PathBuf::from(path);
    }
    
    if let Some(ref path) = config.model_path {
        return PathBuf::from(path);
    }
    
    project_dirs()
        .map(|dirs| dirs.data_dir().join("models"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local/share/sweet-nothings/models")
        })
}

/// Returns path to config file.
pub fn config_file() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".config/sweet-nothings/config.toml")
        })
}

/// Returns full path to a specific model file.
pub fn model_path(config: &Config, model_name: &str) -> PathBuf {
    model_dir(config).join(format!("ggml-{}.bin", model_name))
}
```

Add to Cargo.toml:
```toml
dirs = "5"
```

### Config File Format

```toml
# ~/.config/sweet-nothings/config.toml

model = "base.en"
auto_paste = false
exit_delay = "1500ms"    # or "0ms", "2s", etc.
# model_path = "/custom/path/to/models"  # optional
```

## CLI Interface

### Using Clap Derive

```rust
// src/main.rs

use clap::{Parser, Subcommand};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "sweet-nothings")]
#[command(about = "Terminal-based whisper dictation")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    
    /// Whisper model to use
    #[arg(long, default_value = "base.en")]
    pub model: String,
    
    /// Directory containing whisper models
    #[arg(long)]
    pub model_path: Option<String>,
    
    /// Auto-paste after copying
    #[arg(long, default_value = "false")]
    pub paste: bool,
    
    /// Delay before exit after transcription
    #[arg(long, default_value = "1500ms", value_parser = parse_duration)]
    pub exit_delay: Duration,
    
    /// Enable verbose logging
    #[arg(long, short)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start recording (default if no command given)
    Record,
    
    /// Manage whisper models
    Models {
        #[command(subcommand)]
        action: ModelsAction,
    },
    
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ModelsAction {
    /// Download a model
    Download { model: String },
    /// List installed models
    List,
    /// List available models
    Available,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Get a config value
    Get { key: String },
    /// Set a config value
    Set { key: String, value: String },
    /// Show config file path
    Path,
}

fn parse_duration(s: &str) -> Result<Duration, humantime::DurationError> {
    humantime::parse_duration(s)
}
```

Add to Cargo.toml:
```toml
humantime = "2"
```

### Command Examples

```
sweet-nothings                              # Launch TUI, start recording
sweet-nothings record                       # Same as above
sweet-nothings --paste                      # Auto-paste after transcription
sweet-nothings --exit-delay 0ms             # Exit immediately
sweet-nothings --model small.en             # Use specific model

sweet-nothings models download base.en
sweet-nothings models list
sweet-nothings models available

sweet-nothings config get auto_paste
sweet-nothings config set auto_paste true
sweet-nothings config path

sweet-nothings --help
sweet-nothings --version
```

## Model Management

### Available Models

| Model | Filename | Size | Notes |
|-------|----------|------|-------|
| tiny.en | ggml-tiny.en.bin | ~75MB | Fastest, lower accuracy |
| base.en | ggml-base.en.bin | ~150MB | Good balance (default) |
| small.en | ggml-small.en.bin | ~500MB | Better accuracy |
| medium.en | ggml-medium.en.bin | ~1.5GB | Diminishing returns |

### Download Implementation

```rust
// src/models/download.rs

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub struct ModelInfo {
    pub name: &'static str,
    pub filename: &'static str,
    pub size_bytes: u64,
    pub size_human: &'static str,
}

pub const AVAILABLE_MODELS: &[ModelInfo] = &[
    ModelInfo { name: "tiny.en", filename: "ggml-tiny.en.bin", size_bytes: 75_000_000, size_human: "~75MB" },
    ModelInfo { name: "base.en", filename: "ggml-base.en.bin", size_bytes: 150_000_000, size_human: "~150MB" },
    ModelInfo { name: "small.en", filename: "ggml-small.en.bin", size_bytes: 500_000_000, size_human: "~500MB" },
    ModelInfo { name: "medium.en", filename: "ggml-medium.en.bin", size_bytes: 1_500_000_000, size_human: "~1.5GB" },
];

pub fn download_model(model_name: &str, dest_dir: &Path) -> Result<()> {
    let model = AVAILABLE_MODELS
        .iter()
        .find(|m| m.name == model_name)
        .context(format!("Unknown model: {}", model_name))?;
    
    let url = format!("{}/{}", BASE_URL, model.filename);
    let dest_path = dest_dir.join(model.filename);
    
    fs::create_dir_all(dest_dir)?;
    
    let client = reqwest::blocking::Client::new();
    let response = client.get(&url).send()?;
    let total_size = response.content_length().unwrap_or(model.size_bytes);
    
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n[{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("=>-"));
    pb.set_message(format!("Downloading {}", model_name));
    
    let mut file = File::create(&dest_path)?;
    let mut downloaded: u64 = 0;
    
    let mut response = client.get(&url).send()?;
    let mut buffer = [0u8; 8192];
    
    loop {
        use std::io::Read;
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;
        pb.set_position(downloaded);
    }
    
    pb.finish_with_message(format!("Downloaded {} to {:?}", model_name, dest_path));
    Ok(())
}
```

### First-Run Auto-Download Flow

```rust
// src/models/download.rs (continued)

use std::io::{self, Write};

pub fn prompt_download_model(model_name: &str, dest_dir: &Path) -> Result<bool> {
    let model = AVAILABLE_MODELS
        .iter()
        .find(|m| m.name == model_name)
        .context(format!("Unknown model: {}", model_name))?;
    
    println!("\nNo whisper model found.\n");
    print!("Download '{}' now? ({}) [Y/n]: ", model_name, model.size_human);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    let input = input.trim().to_lowercase();
    if input.is_empty() || input == "y" || input == "yes" {
        download_model(model_name, dest_dir)?;
        println!("\nTip: For better accuracy, run: sweet-nothings models download small.en\n");
        Ok(true)
    } else {
        println!("\nA whisper model is required. Options:\n");
        println!("  sweet-nothings models download base.en     # Recommended, ~150MB");
        println!("  sweet-nothings models download small.en    # Better accuracy, ~500MB");
        println!("  sweet-nothings --model-path /path/to/models # Use existing models");
        println!("\nSee https://github.com/youruser/sweet-nothings for more info.\n");
        Ok(false)
    }
}
```

## Audio Recording with cpal

```rust
// src/audio/recorder.rs

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct CpalRecorder {
    recording: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    temp_path: PathBuf,
    stream: Option<cpal::Stream>,
}

impl CpalRecorder {
    pub fn new() -> Result<Self> {
        let temp_path = std::env::temp_dir().join(format!(
            "sweet-nothings-{}.wav",
            std::process::id()
        ));
        
        Ok(Self {
            recording: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            sample_rate: 16000, // Whisper expects 16kHz
            temp_path,
            stream: None,
        })
    }
}

impl super::Recorder for CpalRecorder {
    fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;
        
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        self.sample_rate = sample_rate;
        
        let recording = self.recording.clone();
        let samples = self.samples.clone();
        
        recording.store(true, Ordering::SeqCst);
        samples.lock().unwrap().clear();
        
        let err_fn = |err| eprintln!("Audio stream error: {}", err);
        
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    if recording.load(Ordering::SeqCst) {
                        samples.lock().unwrap().extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    if recording.load(Ordering::SeqCst) {
                        let floats: Vec<f32> = data.iter().map(|&s| s.to_float_sample()).collect();
                        samples.lock().unwrap().extend(floats);
                    }
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    if recording.load(Ordering::SeqCst) {
                        let floats: Vec<f32> = data.iter().map(|&s| s.to_float_sample()).collect();
                        samples.lock().unwrap().extend(floats);
                    }
                },
                err_fn,
                None,
            )?,
            _ => anyhow::bail!("Unsupported sample format"),
        };
        
        stream.play()?;
        self.stream = Some(stream);
        
        Ok(())
    }
    
    fn stop(&mut self) -> Result<PathBuf> {
        self.recording.store(false, Ordering::SeqCst);
        
        // Drop stream to stop recording
        self.stream.take();
        
        // Write samples to WAV file
        let samples = self.samples.lock().unwrap();
        let spec = WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        
        let mut writer = WavWriter::create(&self.temp_path, spec)?;
        for sample in samples.iter() {
            writer.write_sample(*sample)?;
        }
        writer.finalize()?;
        
        Ok(self.temp_path.clone())
    }
    
    fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }
    
    fn audio_level(&self) -> f32 {
        let samples = self.samples.lock().unwrap();
        if samples.is_empty() {
            return 0.0;
        }
        // Return RMS of last 1024 samples
        let recent: Vec<f32> = samples.iter().rev().take(1024).copied().collect();
        let sum_sq: f32 = recent.iter().map(|s| s * s).sum();
        (sum_sq / recent.len() as f32).sqrt()
    }
}
```

## Transcription Options

### Option A: whisper-rs Bindings (Single Binary)

```rust
// src/transcribe/whisper.rs

use anyhow::{Context, Result};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperTranscriber {
    ctx: WhisperContext,
}

impl WhisperTranscriber {
    pub fn new(model_path: &Path) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().context("Invalid model path")?,
            WhisperContextParameters::default(),
        ).context("Failed to load whisper model")?;
        
        Ok(Self { ctx })
    }
}

impl super::Transcriber for WhisperTranscriber {
    fn transcribe(&self, audio_path: &Path) -> Result<String> {
        // Read WAV file
        let mut reader = hound::WavReader::open(audio_path)?;
        let samples: Vec<f32> = reader
            .samples::<f32>()
            .map(|s| s.unwrap_or(0.0))
            .collect();
        
        // Configure whisper
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some("en"));
        
        // Run inference
        let mut state = self.ctx.create_state()?;
        state.full(params, &samples)?;
        
        // Collect results
        let num_segments = state.full_n_segments()?;
        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }
        
        Ok(text.trim().to_string())
    }
}
```

### Option B: Shell to whisper.cpp CLI (Simpler Build)

```rust
// src/transcribe/whisper_cli.rs

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct WhisperCliTranscriber {
    binary_path: PathBuf,
    model_path: PathBuf,
}

impl WhisperCliTranscriber {
    pub fn new(binary_path: PathBuf, model_path: PathBuf) -> Self {
        Self { binary_path, model_path }
    }
    
    /// Try to find whisper.cpp binary in PATH
    pub fn from_path(model_path: PathBuf) -> Result<Self> {
        let binary = which::which("whisper-cpp")
            .or_else(|_| which::which("main"))
            .context("whisper.cpp binary not found in PATH")?;
        
        Ok(Self::new(binary, model_path))
    }
}

impl super::Transcriber for WhisperCliTranscriber {
    fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let output = Command::new(&self.binary_path)
            .arg("-m")
            .arg(&self.model_path)
            .arg("-f")
            .arg(audio_path)
            .arg("--no-timestamps")
            .arg("-otxt")
            .output()
            .context("Failed to run whisper.cpp")?;
        
        if !output.status.success() {
            anyhow::bail!(
                "whisper.cpp failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
```

Add to Cargo.toml for Option B:
```toml
which = "6"
```

**Recommendation**: Start with Option B (CLI) for faster iteration, switch to Option A (whisper-rs) for final single-binary distribution.

## Clipboard Implementation

### Cross-Platform Copy with arboard

```rust
// src/clipboard/mod.rs

use anyhow::Result;

pub trait Clipboard: Send {
    fn copy(&mut self, text: &str) -> Result<()>;
    fn paste(&self) -> Result<()>;
    fn supports_paste(&self) -> bool;
}

pub struct SystemClipboard {
    clipboard: arboard::Clipboard,
    paste_impl: Box<dyn PasteSimulator>,
}

impl SystemClipboard {
    pub fn new() -> Result<Self> {
        let clipboard = arboard::Clipboard::new()?;
        let paste_impl: Box<dyn PasteSimulator> = {
            #[cfg(target_os = "linux")]
            { Box::new(LinuxPaste::new()) }
            #[cfg(target_os = "macos")]
            { Box::new(MacOsPaste) }
            #[cfg(target_os = "windows")]
            { Box::new(WindowsPaste) }
        };
        
        Ok(Self { clipboard, paste_impl })
    }
}

impl Clipboard for SystemClipboard {
    fn copy(&mut self, text: &str) -> Result<()> {
        self.clipboard.set_text(text)?;
        Ok(())
    }
    
    fn paste(&self) -> Result<()> {
        self.paste_impl.paste()
    }
    
    fn supports_paste(&self) -> bool {
        self.paste_impl.supported()
    }
}

trait PasteSimulator: Send {
    fn paste(&self) -> Result<()>;
    fn supported(&self) -> bool;
}
```

### Linux Paste Simulation

```rust
// src/clipboard/paste_linux.rs

use anyhow::{Context, Result};
use std::process::Command;

pub struct LinuxPaste {
    wayland: bool,
}

impl LinuxPaste {
    pub fn new() -> Self {
        let wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
        Self { wayland }
    }
}

impl super::PasteSimulator for LinuxPaste {
    fn paste(&self) -> Result<()> {
        if self.wayland {
            // wtype for Wayland
            Command::new("wtype")
                .args(["-M", "ctrl", "v", "-m", "ctrl"])
                .status()
                .context("Failed to run wtype")?;
        } else {
            // xdotool for X11
            Command::new("xdotool")
                .args(["key", "ctrl+v"])
                .status()
                .context("Failed to run xdotool")?;
        }
        Ok(())
    }
    
    fn supported(&self) -> bool {
        if self.wayland {
            which::which("wtype").is_ok()
        } else {
            which::which("xdotool").is_ok()
        }
    }
}
```

### macOS Paste Simulation

```rust
// src/clipboard/paste_macos.rs

use anyhow::{Context, Result};
use std::process::Command;

pub struct MacOsPaste;

impl super::PasteSimulator for MacOsPaste {
    fn paste(&self) -> Result<()> {
        Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to keystroke "v" using command down"#,
            ])
            .status()
            .context("Failed to run osascript")?;
        Ok(())
    }
    
    fn supported(&self) -> bool {
        true // Always available on macOS
    }
}
```

### Windows Paste Simulation

```rust
// src/clipboard/paste_windows.rs

use anyhow::{Context, Result};
use std::process::Command;

pub struct WindowsPaste;

impl super::PasteSimulator for WindowsPaste {
    fn paste(&self) -> Result<()> {
        Command::new("powershell")
            .args([
                "-Command",
                r#"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')"#,
            ])
            .status()
            .context("Failed to run PowerShell")?;
        Ok(())
    }
    
    fn supported(&self) -> bool {
        true // PowerShell is always available on Windows
    }
}
```

## TUI with ratatui

### App State Machine

```rust
// src/tui/app.rs

use std::time::{Duration, Instant};
use crate::audio::Recorder;
use crate::transcribe::Transcriber;
use crate::clipboard::Clipboard;
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Recording,
    Transcribing,
    Done,
    Error,
}

pub struct App {
    pub state: AppState,
    pub start_time: Instant,
    pub elapsed: Duration,
    pub transcript: String,
    pub error: Option<String>,
    pub should_quit: bool,
    
    pub recorder: Box<dyn Recorder>,
    pub transcriber: Box<dyn Transcriber>,
    pub clipboard: Box<dyn Clipboard>,
    pub config: Config,
}

impl App {
    pub fn new(
        recorder: Box<dyn Recorder>,
        transcriber: Box<dyn Transcriber>,
        clipboard: Box<dyn Clipboard>,
        config: Config,
    ) -> Self {
        Self {
            state: AppState::Recording,
            start_time: Instant::now(),
            elapsed: Duration::ZERO,
            transcript: String::new(),
            error: None,
            should_quit: false,
            recorder,
            transcriber,
            clipboard,
            config,
        }
    }
    
    pub fn tick(&mut self) {
        if self.state == AppState::Recording {
            self.elapsed = self.start_time.elapsed();
        }
    }
    
    pub fn stop_recording(&mut self) {
        self.state = AppState::Transcribing;
    }
    
    pub fn set_transcript(&mut self, text: String) {
        self.transcript = text;
        self.state = AppState::Done;
    }
    
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.state = AppState::Error;
    }
    
    pub fn formatted_elapsed(&self) -> String {
        let secs = self.elapsed.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}
```

### UI Rendering

```rust
// src/tui/ui.rs

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::app::{App, AppState};

pub fn render(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 40, frame.area());
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(match app.state {
            AppState::Recording => Color::Red,
            AppState::Transcribing => Color::Yellow,
            AppState::Done => Color::Green,
            AppState::Error => Color::Red,
        }))
        .title(" Sweet Nothings ");
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    let content = match app.state {
        AppState::Recording => render_recording(app),
        AppState::Transcribing => render_transcribing(),
        AppState::Done => render_done(app),
        AppState::Error => render_error(app),
    };
    
    frame.render_widget(content, inner);
}

fn render_recording(app: &App) -> Paragraph<'static> {
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("● ", Style::default().fg(Color::Red)),
            Span::raw("Recording... "),
            Span::styled(
                app.formatted_elapsed(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to stop",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    
    Paragraph::new(lines).alignment(Alignment::Center)
}

fn render_transcribing() -> Paragraph<'static> {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "⏳ Transcribing...",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ];
    
    Paragraph::new(lines).alignment(Alignment::Center)
}

fn render_done(app: &App) -> Paragraph<'static> {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "✓ Copied to clipboard!",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("\"{}\"", truncate(&app.transcript, 50)),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];
    
    Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
}

fn render_error(app: &App) -> Paragraph<'static> {
    let error_msg = app.error.as_deref().unwrap_or("Unknown error");
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "✗ Error",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::raw(error_msg)),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to exit",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    
    Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
```

### Main Event Loop

```rust
// src/tui/mod.rs

pub mod app;
pub mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use app::{App, AppState};

pub fn run(mut app: App) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Start recording
    app.recorder.start()?;
    
    // Channel for transcription result
    let (tx, rx) = mpsc::channel();
    
    let result = run_loop(&mut terminal, &mut app, tx, rx);
    
    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    tx: mpsc::Sender<Result<String, String>>,
    rx: mpsc::Receiver<Result<String, String>>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        
        // Check for transcription result
        if let Ok(result) = rx.try_recv() {
            match result {
                Ok(text) => {
                    app.clipboard.copy(&text)?;
                    if app.config.auto_paste && app.clipboard.supports_paste() {
                        // Small delay for terminal to close
                        thread::sleep(Duration::from_millis(100));
                        app.clipboard.paste()?;
                    }
                    app.set_transcript(text);
                }
                Err(e) => app.set_error(e),
            }
        }
        
        // Handle exit delay
        if app.state == AppState::Done {
            if app.config.exit_delay.is_zero() {
                break;
            }
            thread::sleep(app.config.exit_delay);
            break;
        }
        
        // Handle input
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.state {
                        AppState::Recording => {
                            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                                let audio_path = app.recorder.stop()?;
                                app.stop_recording();
                                
                                // Spawn transcription in background
                                // Note: This is a simplification. In real code,
                                // you'd need to handle the transcriber ownership better.
                                let tx = tx.clone();
                                thread::spawn(move || {
                                    // Transcription would happen here
                                    // tx.send(transcriber.transcribe(&audio_path).map_err(|e| e.to_string()))
                                });
                            }
                        }
                        AppState::Error => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        
        app.tick();
        
        if app.should_quit {
            break;
        }
    }
    
    Ok(())
}
```

## NixOS Flake

```nix
{
  description = "Sweet Nothings - Terminal-based whisper dictation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
        
        # Common build inputs for whisper-rs
        whisperDeps = with pkgs; [
          cmake
          pkg-config
          openssl
        ] ++ lib.optionals stdenv.isLinux [
          alsa-lib
        ] ++ lib.optionals stdenv.isDarwin [
          darwin.apple_sdk.frameworks.CoreAudio
          darwin.apple_sdk.frameworks.AudioToolbox
        ];
        
        # Runtime deps for paste simulation
        runtimeDeps = with pkgs; lib.optionals stdenv.isLinux [
          # Wayland
          wl-clipboard
          wtype
          # X11
          xclip
          xdotool
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "sweet-nothings";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          
          nativeBuildInputs = whisperDeps ++ [ pkgs.makeWrapper ];
          buildInputs = whisperDeps;
          
          # Set OPENSSL for reqwest
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          
          postInstall = ''
            wrapProgram $out/bin/sweet-nothings \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
          '';
          
          meta = with pkgs.lib; {
            description = "Terminal-based whisper dictation tool";
            homepage = "https://github.com/youruser/sweet-nothings";
            license = licenses.mit;
            platforms = platforms.unix;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.rust-analyzer
          ] ++ whisperDeps ++ runtimeDeps;
          
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      });
}
```

## Build Phases

### Phase 1: Project Setup + Proof of Concept

Goal: Validate audio capture and whisper work.

1. Initialize project with `cargo new sweet-nothings`
2. Add dependencies to Cargo.toml
3. Implement basic cpal recorder (no trait yet)
4. Implement whisper CLI transcriber (shell out)
5. Simple main.rs that:
   - Records for 5 seconds
   - Transcribes
   - Prints to stdout

Test: `cargo run` should capture your voice and print transcription.

### Phase 2: Traits and Config

Goal: Clean abstractions.

1. Extract Recorder trait
2. Extract Transcriber trait
3. Extract Clipboard trait with arboard
4. Implement config loading (XDG, TOML)
5. Add clap CLI structure

Test: `cargo run --model small.en` should work with different models.

### Phase 3: Model Management

Goal: Zero-friction first run.

1. Implement model download with progress bar
2. Implement first-run prompt (blocking, before TUI)
3. Add `models` subcommand
4. Test on fresh system (delete models, verify prompt)

Test: Delete models, run `cargo run`, should prompt to download.

### Phase 4: TUI

Goal: The actual product.

1. Set up ratatui with crossterm
2. Implement app state machine
3. Implement UI rendering
4. Wire up recorder → transcriber → clipboard flow
5. Handle exit delay

Test: `cargo run` launches TUI, records, transcribes, copies, exits.

### Phase 5: Polish and Distribution

Goal: Ready for users.

1. Add `config` subcommand
2. Add `--paste` flag
3. Error handling everywhere
4. Write README
5. Complete flake.nix
6. Test on fresh NixOS system
7. Test on macOS (if available)

## Window Manager Integration Examples

### Niri

```kdl
// ~/.config/niri/config.kdl
binds {
    Mod+Shift+D { spawn "alacritty" "-e" "sweet-nothings" "--paste"; }
}

window-rules {
    match app-id="sweet-nothings" {
        open-floating true
        default-column-width { fixed 400; }
    }
}
```

### Hyprland

```conf
# ~/.config/hyprland/hyprland.conf
bind = $mod SHIFT, D, exec, alacritty -e sweet-nothings --paste

windowrulev2 = float,class:(Alacritty),title:(sweet-nothings)
windowrulev2 = size 400 150,class:(Alacritty),title:(sweet-nothings)
windowrulev2 = center,class:(Alacritty),title:(sweet-nothings)
```

### Sway

```conf
# ~/.config/sway/config
bindsym $mod+Shift+d exec alacritty -e sweet-nothings --paste

for_window [app_id="Alacritty" title="sweet-nothings"] floating enable
for_window [app_id="Alacritty" title="sweet-nothings"] resize set 400 150
```

### i3 (X11)

```conf
# ~/.config/i3/config
bindsym $mod+Shift+d exec --no-startup-id alacritty -e sweet-nothings --paste

for_window [class="Alacritty" title="sweet-nothings"] floating enable
for_window [class="Alacritty" title="sweet-nothings"] resize set 400 150
for_window [class="Alacritty" title="sweet-nothings"] move position center
```

## Key Differences from Go Version

| Aspect | Go Version | Rust Version |
|--------|------------|--------------|
| Audio capture | Shell to pw-record | cpal (native) |
| Clipboard copy | Shell to wl-copy/xclip | arboard (native) |
| Clipboard paste | Shell to wtype/xdotool | Shell to wtype/xdotool (same) |
| Transcription | Shell to whisper.cpp | whisper-rs OR shell |
| Distribution | Binary + runtime deps | Single binary (mostly) |
| Build complexity | Simple | Medium (C deps for whisper-rs) |
| Cross-compile | Easy | Harder |

## Future Enhancements (Post-MVP)

- [ ] Streaming transcription
- [ ] Audio level visualization in TUI
- [ ] Windows support
- [ ] GPU acceleration for whisper-rs (CUDA, Metal)
- [ ] Voice activity detection (auto-stop on silence)
- [ ] Multiple language support
- [ ] Custom hotkey handling (run as daemon)
- [ ] Audio input device selection
