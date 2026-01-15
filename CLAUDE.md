# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Development
cargo build
cargo run

# Release build
cargo build --release

# Run with arguments
cargo run -- --paste
cargo run -- models list

# Nix development shell (provides whisper-cpp and other dependencies)
nix develop
```

## Architecture

Sweet Nothings is a terminal-based dictation tool that records audio, transcribes it locally using whisper.cpp, and copies the result to the clipboard.

### Module Structure

- **`src/audio/`** - Audio capture using cpal. Defines `Recorder` trait with `CpalRecorder` implementation that writes WAV files to temp directory.
- **`src/tui/`** - Terminal UI using ratatui/crossterm. `App` manages state machine (recording → transcribing → done), `ui.rs` handles rendering.
- **`src/transcribe/`** - Whisper integration via CLI subprocess (`whisper-cli`).
- **`src/clipboard/`** - Clipboard operations using arboard, with optional paste simulation (wtype for Wayland, xdotool for X11).
- **`src/models/`** - Model management: listing, downloading from Hugging Face.
- **`src/config/`** - XDG-compliant configuration loading/saving with TOML.
- **`src/cli/`** - Clap-based CLI argument parsing.

### Data Flow

1. TUI starts recording immediately via `CpalRecorder`
2. User presses Enter/Esc to stop recording
3. Audio saved to temp WAV file
4. `whisper-cli` subprocess transcribes audio
5. Result copied to clipboard (and optionally auto-pasted)
6. TUI displays result, auto-exits after configurable delay

### Dependencies

- **Runtime**: whisper-cpp (whisper-cli binary), ALSA, wtype/xdotool for paste
- **Nix**: `flake.nix` provides dev shell and package with all dependencies wrapped
