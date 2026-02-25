//! Sweet Nothings - Terminal-based whisper dictation
//!
//! A simple TUI tool that records audio, transcribes it with whisper,
//! and copies the result to the clipboard.

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

fn main() -> Result<()> {
    let args = Cli::parse();

    // Handle subcommands
    if let Some(command) = args.command {
        return handle_command(command);
    }

    // Load config, applying CLI overrides
    let mut config = Config::load()?;

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

    // Check for model, prompt to download if missing
    let model_path = config.model_path();
    if !model_path.exists() {
        if !models::prompt_download(&config.model, &config.models_dir())? {
            std::process::exit(1);
        }
    }

    // Run the TUI
    tui::run(&config)
}

fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Models { action } => handle_models_command(action),
        Commands::Config { action } => handle_config_command(action),
    }
}

fn handle_models_command(action: ModelsAction) -> Result<()> {
    match action {
        ModelsAction::List => {
            let models_dir = config::models_dir();
            println!("Models directory: {:?}", models_dir);
            println!();

            if !models_dir.exists() {
                println!("No models installed.");
                return Ok(());
            }

            let entries = std::fs::read_dir(&models_dir)?;
            let mut found = false;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "bin") {
                    let size = std::fs::metadata(&path)?.len();
                    let size_mb = size as f64 / (1024.0 * 1024.0);
                    println!(
                        "  {} ({:.1} MB)",
                        path.file_name().unwrap().to_string_lossy(),
                        size_mb
                    );
                    found = true;
                }
            }

            if !found {
                println!("No models installed.");
            }

            Ok(())
        }
        ModelsAction::Available => {
            println!("Available models:");
            println!();
            for model in models::AVAILABLE_MODELS {
                println!(
                    "  {:12} ({:>8}) - {}",
                    model.name, model.size_human, model.description
                );
            }
            println!();
            println!("Download with: sweet-nothings models download <model>");
            Ok(())
        }
        ModelsAction::Download { model } => {
            let models_dir = config::models_dir();
            models::download_model(&model, &models_dir)?;
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
            println!("{:?}", config::config_path());
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

