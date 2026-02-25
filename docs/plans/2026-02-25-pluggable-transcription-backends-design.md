# Pluggable Transcription Backends Design

**Date:** 2026-02-25
**Branch:** parakeet-support

## Summary

Add a pluggable backend system for transcription engines, starting with Parakeet (via `parakeet-rs`) alongside the existing Whisper CLI integration. Each backend is a compile-time Cargo feature. A Nix module supports both NixOS and Home Manager for declarative configuration.

## Decisions

| Decision | Choice |
|---|---|
| Plugin style | Compile-time backends (Rust modules) |
| Parakeet integration | Native via `parakeet-rs` crate (ONNX Runtime) |
| Whisper integration | Keep existing CLI subprocess approach |
| Backend selection UX | `backend` config field + `--backend` CLI flag |
| Model storage | Backend subdirectories under models dir |
| Build gating | Cargo feature flag per backend |
| Nix integration | Shared module for NixOS and Home Manager |

## Architecture

### Backend Trait

The existing `Transcriber` trait evolves into `TranscriptionBackend`, which bundles transcription with model management:

```rust
// src/transcribe/mod.rs

pub trait TranscriptionBackend: Send + Sync {
    /// Backend identifier (e.g., "whisper", "parakeet")
    fn name(&self) -> &str;

    /// Transcribe audio file to text
    fn transcribe(&self, audio_path: &Path) -> Result<String>;

    /// List models available for download
    fn available_models(&self) -> &[ModelInfo];

    /// List models already downloaded locally
    fn installed_models(&self, models_dir: &Path) -> Result<Vec<String>>;

    /// Download a model by name
    fn download_model(&self, name: &str, models_dir: &Path) -> Result<PathBuf>;

    /// Resolve a model name to a local file path
    fn resolve_model_path(&self, name: &str, models_dir: &Path) -> Result<PathBuf>;
}
```

### Registry

A registry maps backend names to implementations, gated by feature flags:

```rust
// src/transcribe/registry.rs

pub fn create_backend(name: &str, model_path: &Path) -> Result<Box<dyn TranscriptionBackend>> {
    match name {
        "whisper" => Ok(Box::new(WhisperCliBackend::new(model_path)?)),
        #[cfg(feature = "parakeet")]
        "parakeet" => Ok(Box::new(ParakeetBackend::new(model_path)?)),
        _ => bail!("Unknown backend: {name}"),
    }
}

pub fn available_backend_names() -> Vec<&'static str> {
    let mut backends = vec!["whisper"];
    #[cfg(feature = "parakeet")]
    backends.push("parakeet");
    backends
}
```

### File Structure

```
src/transcribe/
├── mod.rs              # TranscriptionBackend trait, ModelInfo, re-exports
├── registry.rs         # create_backend(), available_backend_names()
├── whisper_cli.rs      # WhisperCliBackend (existing, refactored)
└── parakeet.rs         # ParakeetBackend (new, behind #[cfg(feature = "parakeet")])
```

## Configuration

### Config File

```toml
# ~/.config/sweet-nothings/config.toml
backend = "whisper"          # NEW: defaults to "whisper" for backwards compat
model = "base.en"
auto_paste = false
exit_delay = "2s"
whisper_path = "/usr/bin/whisper-cli"  # optional, only used when backend = "whisper"
```

The `backend` field defaults to `"whisper"`, so existing configs work unchanged.

### CLI

```bash
# New --backend flag
sweet-nothings --backend parakeet --model tdt-0.6b
sweet-nothings --backend whisper --model base.en
sweet-nothings --model base.en                     # uses config default

# Models subcommand gains --backend
sweet-nothings models list                          # current backend
sweet-nothings models list --backend parakeet
sweet-nothings models available --backend parakeet
sweet-nothings models download tdt-0.6b --backend parakeet

# Config
sweet-nothings config set backend parakeet
```

### Model Storage

Backend-specific subdirectories under the models dir:

```
~/.local/share/sweet-nothings/models/
├── whisper/
│   ├── ggml-base.en.bin
│   └── ggml-small.bin
└── parakeet/
    └── tdt-0.6b/
        ├── model.onnx
        └── tokenizer.json
```

## Cargo Feature Flags

```toml
[features]
default = ["whisper"]
whisper = []
parakeet = ["dep:parakeet-rs", "dep:ort"]

[dependencies]
parakeet-rs = { version = "...", optional = true }
ort = { version = "...", optional = true }
```

Adding a future backend follows the same pattern:

```toml
future-backend = ["dep:whatever"]
```

## Parakeet Backend

### Integration via parakeet-rs

```rust
#[cfg(feature = "parakeet")]
pub struct ParakeetBackend {
    model: parakeet_rs::Model,
}
```

Key properties:
- Native Rust inference via ONNX Runtime (no subprocess)
- Accepts 16kHz mono WAV (same format Sweet Nothings already produces)
- Adds punctuation and capitalization automatically
- CPU execution provider works out of the box; GPU optional
- Model loaded once at backend creation, reused per transcription

### Available Parakeet Models

| Model | Parameters | Notes |
|---|---|---|
| tdt-0.6b | 600M | Recommended default, good speed/accuracy balance |
| tdt-1.1b | 1.1B | Higher accuracy, slower |
| rnnt-0.6b | 600M | RNN-Transducer variant |
| rnnt-1.1b | 1.1B | RNN-Transducer variant |
| ctc-0.6b | 600M | CTC variant |
| ctc-1.1b | 1.1B | CTC variant |

### Parakeet vs Whisper

| | Parakeet | Whisper |
|---|---|---|
| Speed (RTFx) | ~3380 | 2-5 |
| English WER | 18.56% | 19.96% |
| Languages | 25 European (v3) | 100+ |
| Punctuation | Automatic | None |
| Integration | Native Rust (ONNX) | CLI subprocess |

## Nix Module

### Flake Outputs

```nix
{
  packages.default = buildSweetNothings { features = ["whisper"]; };
  packages.full = buildSweetNothings { features = ["whisper" "parakeet"]; };

  nixosModules.default = import ./nix/module.nix self;
  homeManagerModules.default = import ./nix/module.nix self;
}
```

### Shared Module (nix/module.nix)

Works in both NixOS and Home Manager contexts:

```nix
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.sweet-nothings;
in {
  options.programs.sweet-nothings = {
    enable = lib.mkEnableOption "sweet-nothings dictation tool";

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
    };

    settings = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = {};
      description = "Extra config.toml settings";
    };
  };

  config = lib.mkIf cfg.enable {
    # Builds package with selected Cargo features
    # Wraps binary with conditional runtime deps
    # Generates config.toml from settings
  };
};
```

### Usage

```nix
# NixOS (configuration.nix)
imports = [ sweet-nothings.nixosModules.default ];
programs.sweet-nothings = {
  enable = true;
  backends = [ "whisper" "parakeet" ];
  defaultBackend = "parakeet";
  model = "tdt-0.6b";
};

# Home Manager (home.nix) — identical API
imports = [ sweet-nothings.homeManagerModules.default ];
programs.sweet-nothings = {
  enable = true;
  backends = [ "parakeet" ];
  defaultBackend = "parakeet";
  model = "tdt-0.6b";
};

# Plain nix (no module system)
nix run github:user/sweet-nothings#full
```

The module conditionally includes runtime deps: whisper-cpp only when whisper is in `backends`, ONNX runtime libs when parakeet is in `backends`.

## Refactoring Plan

### Files to Modify

1. **`src/transcribe/mod.rs`** — Replace `Transcriber` trait with `TranscriptionBackend`
2. **`src/transcribe/whisper_cli.rs`** — Rename `WhisperCliTranscriber` to `WhisperCliBackend`, implement new trait methods (model management)
3. **`src/models/`** — Remove hardcoded model list and download URL; model management delegates to backend trait
4. **`src/models/download.rs`** — Becomes shared HTTP download utility (progress bar, fetch) that backends call
5. **`src/config/mod.rs`** — Add `backend: String` field (default: "whisper")
6. **`src/config/paths.rs`** — Remove `ggml-{name}.bin` normalization; add backend subdirectory to model path
7. **`src/cli/mod.rs`** — Add `--backend` global option and `--backend` on models subcommands
8. **`src/tui/app.rs`** — Use `create_backend()` instead of direct `WhisperCliTranscriber` construction
9. **`Cargo.toml`** — Add feature flags, optional parakeet-rs/ort dependencies
10. **`flake.nix`** — Parameterized build function, conditional runtime deps

### New Files

1. **`src/transcribe/registry.rs`** — Backend registry
2. **`src/transcribe/parakeet.rs`** — Parakeet backend implementation
3. **`nix/module.nix`** — Shared NixOS/Home Manager module

### Files Unchanged

- `src/audio/` — Audio capture is backend-agnostic
- `src/clipboard/` — Clipboard operations are backend-agnostic
- `src/tui/ui.rs` — Rendering is backend-agnostic
