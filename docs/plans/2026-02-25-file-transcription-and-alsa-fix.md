# File Transcription and ALSA Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `--file` flag for direct file transcription (bypassing TUI/recording), fix ALSA routing through PipeWire in the dev shell, and add an optional `ffmpeg` feature for automatic audio format conversion.

**Architecture:** The `--file` path is a separate code path in `main.rs` that runs after model resolution but before `tui::run()`. It creates the backend, transcribes the file, prints the result, and optionally copies to clipboard/pastes. The `ffmpeg` feature adds a conversion step that shells out to `ffmpeg` to convert non-WAV files to 16kHz mono WAV in a temp file before transcription. The ALSA fix adds `alsa-plugins` and `pipewire` to the nix dev shell with the `ALSA_PLUGIN_DIR` env var.

**Tech Stack:** Rust, clap, std::process::Command (for ffmpeg), Nix flakes

---

### Task 1: Fix ALSA PipeWire Routing in Dev Shell

**Files:**
- Modify: `flake.nix:94-112` (devShells.default)

**Step 1: Add PipeWire ALSA packages to dev shell**

In `flake.nix`, update the `devShells.default` section. Add `alsa-plugins` and `pipewire` to the packages list, and set `ALSA_PLUGIN_DIR` so ALSA can find the PipeWire PCM plugin:

```nix
devShells.default = pkgs.mkShell {
  buildInputs = baseBuildInputs;
  inherit nativeBuildInputs;

  packages = with pkgs; [
    whisper-cpp
    alsa-plugins
    pipewire
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
  ALSA_PLUGIN_DIR = "${pkgs.alsa-plugins}/lib/alsa-lib";
};
```

**Step 2: Test the fix**

Run:
```bash
nix develop --command cargo run
```
Expected: TUI starts recording without ALSA errors. Press Esc to stop (transcription will fail without a model, that's fine — the ALSA error should be gone).

**Step 3: Commit**

```bash
git add flake.nix
git commit -m "fix: add PipeWire ALSA routing to dev shell"
```

---

### Task 2: Add `--file` CLI Argument

**Files:**
- Modify: `src/cli/mod.rs:13-39` (Cli struct)

**Step 1: Add `file` field to `Cli` struct**

Add the `--file` option to the `Cli` struct in `src/cli/mod.rs`, after the `paste` field:

```rust
/// Transcribe an audio file directly (skip recording TUI)
#[arg(short, long)]
pub file: Option<PathBuf>,
```

**Step 2: Verify it compiles**

Run:
```bash
nix develop --command cargo check
```
Expected: Compiles with no errors.

**Step 3: Verify help output**

Run:
```bash
nix develop --command cargo run -- --help
```
Expected: Shows `-f, --file <FILE>` option in the help text.

**Step 4: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add --file CLI argument for direct file transcription"
```

---

### Task 3: Implement `--file` Transcription Path

**Files:**
- Modify: `src/main.rs:46-97` (between subcommand handling and TUI run)

**Step 1: Add the file transcription path**

In `src/main.rs`, after the model existence check (line ~94) and before `tui::run(&config)`, add the `--file` handling. Also add `args.file` to the CLI override section at the top by storing the file path on config or passing it through.

The simplest approach: check `args.file` after model resolution. If set, transcribe directly and return.

First, store the file path before `args.command` is consumed. In the CLI overrides section (around line 27), add:

```rust
let file_path = args.file.clone();
```

Then after the model download prompt block (after line 94), before `tui::run(&config)`:

```rust
// Direct file transcription mode (skip TUI)
if let Some(ref file) = file_path {
    return transcribe_file(file, &config);
}
```

**Step 2: Implement `transcribe_file` function**

Add this function to `src/main.rs`:

```rust
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

    let (transcribe_path, _temp_file) = if ext == "wav" {
        (file.to_path_buf(), None)
    } else {
        #[cfg(feature = "ffmpeg")]
        {
            let temp = convert_to_wav(file)?;
            let path = temp.path().to_path_buf();
            (path, Some(temp))
        }
        #[cfg(not(feature = "ffmpeg"))]
        {
            anyhow::bail!(
                "Unsupported format '.{}'. Only WAV files are supported.\n\
                 Convert with: ffmpeg -i {:?} -ar 16000 -ac 1 output.wav\n\n\
                 Or rebuild with ffmpeg support: cargo build --features ffmpeg",
                ext,
                file
            );
        }
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
    let mut clipboard = crate::clipboard::SystemClipboard::new()?;
    crate::clipboard::Clipboard::copy(&mut clipboard, &text)?;
    eprintln!("(copied to clipboard)");

    // Auto-paste if requested
    if config.auto_paste {
        crate::clipboard::Clipboard::paste(&clipboard)?;
    }

    Ok(())
}
```

**Step 3: Verify it compiles**

Run:
```bash
nix develop --command cargo check
```
Expected: Compiles with no errors.

**Step 4: Test with a WAV file**

Create a test WAV file with speech (or use any existing WAV). Run:
```bash
nix develop --command cargo run -- --file test.wav
```
Expected: Transcription printed to stdout, "(copied to clipboard)" on stderr.

**Step 5: Test with non-WAV file (no ffmpeg feature)**

Run:
```bash
nix develop --command cargo run -- --file test.mp3
```
Expected: Error message showing the manual ffmpeg conversion command and the feature flag hint.

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add --file mode for direct file transcription"
```

---

### Task 4: Add `ffmpeg` Feature Flag and Conversion

**Files:**
- Modify: `Cargo.toml:8-11` (features section)
- Modify: `src/main.rs` (add `convert_to_wav` function)

**Step 1: Add `ffmpeg` feature to `Cargo.toml`**

In the `[features]` section of `Cargo.toml`, add:

```toml
[features]
default = ["whisper"]
whisper = []
parakeet = ["dep:parakeet-rs"]
ffmpeg = []
```

The `ffmpeg` feature is a marker — no extra crate dependencies. It just gates the code that shells out to `ffmpeg`.

**Step 2: Add `convert_to_wav` function to `src/main.rs`**

Add this function, gated behind the `ffmpeg` feature:

```rust
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
```

**Step 3: Add `tempfile` dependency to `Cargo.toml`**

Add `tempfile` as an optional dependency gated by the `ffmpeg` feature:

```toml
[features]
default = ["whisper"]
whisper = []
parakeet = ["dep:parakeet-rs"]
ffmpeg = ["dep:tempfile"]

[dependencies]
# ... existing deps ...
tempfile = { version = "3", optional = true }
```

**Step 4: Verify default build (no ffmpeg)**

Run:
```bash
nix develop --command cargo check
```
Expected: Compiles. The `convert_to_wav` function is not compiled.

**Step 5: Verify ffmpeg build**

Run:
```bash
nix develop --command cargo check --features ffmpeg
```
Expected: Compiles with the conversion function included.

**Step 6: Test ffmpeg conversion (requires ffmpeg in PATH)**

Run:
```bash
nix develop --command cargo run --features ffmpeg -- --file test.mp3
```
Expected: "Converting test.mp3 to WAV..." then transcription output.

**Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "feat: add ffmpeg feature for automatic audio format conversion"
```

---

### Task 5: Update Nix Flake for ffmpeg Feature

**Files:**
- Modify: `flake.nix` (buildSweetNothings, packages, devShell)

**Step 1: Add ffmpeg to runtime deps and build variants**

In `flake.nix`, add `ffmpeg` handling to `buildSweetNothings`:

```nix
buildSweetNothings = { features ? [ "whisper" ] }:
  let
    hasWhisper = builtins.elem "whisper" features;
    hasParakeet = builtins.elem "parakeet" features;
    hasFfmpeg = builtins.elem "ffmpeg" features;
    runtimeDeps = commonRuntimeDeps
      ++ (if hasWhisper then whisperRuntimeDeps else [])
      ++ (if hasParakeet then parakeetRuntimeDeps else [])
      ++ (if hasFfmpeg then [ pkgs.ffmpeg ] else []);
    extraBuildInputs = if hasParakeet then [ pkgs.onnxruntime ] else [];
  in
  # ... rest unchanged
```

Add `ffmpeg` to the `full` package and add a convenience variant:

```nix
packages = {
  default = buildSweetNothings { features = [ "whisper" ]; };
  full = buildSweetNothings { features = [ "whisper" "parakeet" "ffmpeg" ]; };
  whisper-only = buildSweetNothings { features = [ "whisper" ]; };
  parakeet-only = buildSweetNothings { features = [ "parakeet" ]; };
};
```

Also add `ffmpeg` to the dev shell packages so it's available during development:

```nix
packages = with pkgs; [
  whisper-cpp
  alsa-plugins
  pipewire
  ffmpeg
] ++ commonRuntimeDeps ++ whisperRuntimeDeps;
```

**Step 2: Update module to support ffmpeg backend option**

In `nix/module.nix`, update the backends enum to include `ffmpeg`:

```nix
backends = lib.mkOption {
  type = lib.types.listOf (lib.types.enum [ "whisper" "parakeet" "ffmpeg" ]);
  default = [ "whisper" ];
  description = "Transcription backends and features to compile in.";
};
```

**Step 3: Verify**

Run:
```bash
nix develop --command cargo check --features ffmpeg
```
Expected: Compiles. ffmpeg is available in PATH inside the dev shell.

**Step 4: Commit**

```bash
git add flake.nix nix/module.nix
git commit -m "feat: add ffmpeg feature to nix flake and module"
```

---

### Task 6: Update README and PR Description

**Files:**
- Modify: `README.md`

**Step 1: Add `--file` usage to README**

In the "Basic Usage" section, add examples:

```bash
# Transcribe an existing WAV file
sweet-nothings --file recording.wav

# Transcribe with a specific backend
sweet-nothings --file recording.wav --backend parakeet --model tdt-0.6b

# Transcribe and auto-paste
sweet-nothings --file recording.wav --paste
```

In the "From Source" section, add:

```bash
# Build with ffmpeg support (auto-converts mp3/mp4/flac/ogg to WAV)
cargo build --release --features ffmpeg

# Build with everything
cargo build --release --features "whisper parakeet ffmpeg"
```

**Step 2: Add `--file` to CLI Options section**

Update the CLI Options block to include:

```
  -f, --file <FILE>        Transcribe a file directly (skip recording TUI)
```

**Step 3: Add a "File Transcription" section**

After the "Basic Usage" section, add:

```markdown
### File Transcription

Transcribe existing audio files without the recording TUI:

\```bash
# WAV files work out of the box
sweet-nothings --file meeting.wav

# Other formats require the ffmpeg feature
sweet-nothings --file interview.mp3    # needs ffmpeg feature
sweet-nothings --file podcast.mp4      # needs ffmpeg feature
\```

The `ffmpeg` feature automatically converts non-WAV files to the required format. Without it, only WAV files are accepted.
```

**Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add --file usage and ffmpeg feature to README"
```
