//! Sweet Nothings - Terminal-based dictation tool
//!
//! A simple TUI tool that records audio, transcribes it with a configurable
//! backend, and copies the result to the clipboard.

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

    // Load config first, applying CLI overrides
    let mut config = Config::load()?;

    let file_path = args.file.clone();

    if let Some(ref backend) = args.backend {
        config.backend = backend.clone();
    }
    if args.model != "base.en" {
        config.model = args.model.clone();
    }
    if args.paste {
        config.auto_paste = true;
    }
    if let Some(delay) = args.exit_delay {
        config.exit_delay = delay;
    }
    if let Some(ref path) = args.whisper_path {
        config.whisper_path = Some(path.clone());
    }
    if let Some(ref dir) = args.models_dir {
        config.models_dir = Some(dir.clone());
    }

    // Handle subcommands
    if let Some(command) = args.command {
        return handle_command(command, &config);
    }

    // Migrate old flat model directory to backend subdirectories
    migrate_model_directory(&config.models_dir())?;

    // Resolve model path via backend
    let backend = resolve_backend_for_model_check(&config)?;
    let models_dir = config.models_dir();
    let model_path = backend.resolve_model_path(&config.model, &models_dir)?;

    // Check for model, prompt to download if missing
    if !model_path.exists() {
        let info = backend
            .available_models()
            .iter()
            .find(|m| m.name == config.model);
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
                    config.model, config.backend
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

    // Direct file transcription mode (skip TUI)
    if let Some(ref file) = file_path {
        return transcribe_file(file, &config);
    }

    // Run the TUI
    tui::run(&config)
}

/// Create a backend instance for model resolution (not transcription).
fn resolve_backend_for_model_check(config: &Config) -> Result<Box<dyn TranscriptionBackend>> {
    match config.backend.as_str() {
        "whisper" => {
            let models_dir = config.models_dir();
            // Create with a placeholder — we only need resolve_model_path and available_models
            let backend = WhisperCliBackend::auto_detect(
                models_dir.join("whisper").join("dummy"),
            )
            .or_else(|_| {
                Ok::<_, anyhow::Error>(WhisperCliBackend::new(
                    std::path::PathBuf::from("whisper-cli"),
                    models_dir.join("whisper").join("dummy"),
                ))
            })?;
            Ok(Box::new(backend))
        }
        #[cfg(feature = "parakeet")]
        "parakeet" => {
            let models_dir = config.models_dir();
            let model_path = models_dir.join("parakeet").join(&config.model);
            let backend = transcribe::parakeet::ParakeetBackend::new(&model_path)?;
            Ok(Box::new(backend))
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

fn handle_command(command: Commands, config: &Config) -> Result<()> {
    match command {
        Commands::Models { action } => handle_models_command(action, config),
        Commands::Config { action } => handle_config_command(action),
    }
}

fn handle_models_command(action: ModelsAction, config: &Config) -> Result<()> {
    match action {
        ModelsAction::List { backend } => {
            let backend_name = backend.as_deref().unwrap_or(&config.backend);
            let models_dir = config.models_dir();

            let backend = resolve_backend_for_model_check(&Config {
                backend: backend_name.to_string(),
                ..config.clone()
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
            let backend_name = backend.as_deref().unwrap_or(&config.backend);

            let backend = resolve_backend_for_model_check(&Config {
                backend: backend_name.to_string(),
                ..config.clone()
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
            let backend_name = backend.as_deref().unwrap_or(&config.backend);
            let models_dir = config.models_dir();

            let backend = resolve_backend_for_model_check(&Config {
                backend: backend_name.to_string(),
                ..config.clone()
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

/// Transcribe an audio file directly, print result, optionally copy to clipboard.
fn transcribe_file(file: &std::path::Path, config: &Config) -> Result<()> {
    if !file.exists() {
        anyhow::bail!("File not found: {}", file.display());
    }

    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    #[cfg(feature = "ffmpeg")]
    let (transcribe_path, _temp_file): (std::path::PathBuf, Option<tempfile::NamedTempFile>) =
        if ext == "wav" {
            (file.to_path_buf(), None)
        } else {
            let temp = convert_to_wav(file)?;
            let path = temp.path().to_path_buf();
            (path, Some(temp))
        };

    #[cfg(not(feature = "ffmpeg"))]
    let transcribe_path: std::path::PathBuf = if ext == "wav" {
        file.to_path_buf()
    } else {
        anyhow::bail!(
            "Unsupported format '.{}'. Only WAV files are supported.\n\
             Convert with: ffmpeg -i {:?} -ar 16000 -ac 1 output.wav\n\n\
             Or rebuild with ffmpeg support: cargo build --features ffmpeg",
            ext,
            file
        );
    };

    // Create the transcription backend
    let models_dir = config.models_dir();
    let backend: Box<dyn TranscriptionBackend> = match config.backend.as_str() {
        "whisper" => Box::new(WhisperCliBackend::from_config(
            config.whisper_path.as_deref(),
            &config.model,
            &models_dir,
        )?),
        #[cfg(feature = "parakeet")]
        "parakeet" => {
            let model_path = models_dir.join("parakeet").join(&config.model);
            transcribe::create_backend("parakeet", &model_path)?
        }
        _ => {
            let available = available_backend_names().join(", ");
            anyhow::bail!(
                "Unknown backend: '{}'. Available: {}",
                config.backend,
                available
            );
        }
    };

    let text = backend.transcribe(&transcribe_path)?;

    // Print result
    println!("{}", text);

    // Copy to clipboard
    let mut clip = clipboard::SystemClipboard::new()?;
    clipboard::Clipboard::copy(&mut clip, &text)?;
    eprintln!("(copied to clipboard)");

    // Auto-paste if requested
    if config.auto_paste {
        clipboard::Clipboard::paste(&clip)?;
    }

    Ok(())
}

/// Convert an audio file to 16kHz mono WAV using ffmpeg.
#[cfg(feature = "ffmpeg")]
fn convert_to_wav(input: &std::path::Path) -> Result<tempfile::NamedTempFile> {
    use std::process::Command;

    let temp = tempfile::Builder::new()
        .suffix(".wav")
        .tempfile()
        .context("Failed to create temp file")?;

    eprintln!(
        "Converting {} to WAV...",
        input.file_name().unwrap_or_default().to_string_lossy()
    );

    let status = Command::new("ffmpeg")
        .args([
            "-i",
            input.to_str().context("Invalid input path")?,
            "-ar",
            "16000",
            "-ac",
            "1",
            "-y",
            temp.path().to_str().context("Invalid temp path")?,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to run ffmpeg. Is it installed?")?;

    if !status.success() {
        anyhow::bail!("ffmpeg conversion failed (exit code: {:?})", status.code());
    }

    Ok(temp)
}

/// Migrate models from flat directory to backend subdirectories.
/// Moves ggml-*.bin files from models/ to models/whisper/.
fn migrate_model_directory(models_dir: &std::path::Path) -> Result<()> {
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
