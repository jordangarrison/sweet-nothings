# Sweet Nothings

*Whisper sweet nothings to your Linux computer.*

A terminal-based dictation tool that records speech, transcribes it locally, and copies the result to your clipboard. Finally, a machine that listens.

## Features

- Record audio from your microphone
- Transcribe locally (no cloud required)
- Pluggable transcription backends: [whisper.cpp](https://github.com/ggerganov/whisper.cpp) (default) and [Parakeet](https://github.com/nvidia/parakeet) (NVIDIA ASR)
- Copy transcription to clipboard
- Optional auto-paste after transcription
- TUI with visual feedback (recording timer, audio level, spinner)
- Automatic model downloading
- XDG-compliant configuration

## Installation

### Nix (Flakes)

```bash
# Run directly (whisper backend)
nix run github:jordangarrison/sweet-nothings

# Run with all backends
nix run github:jordangarrison/sweet-nothings#full

# Install to profile
nix profile install github:jordangarrison/sweet-nothings
```

### Nix Module (Home Manager)

The recommended way to use Sweet Nothings with Nix. The module builds the package with your selected backends and generates the config file declaratively.

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    sweet-nothings.url = "github:jordangarrison/sweet-nothings";
  };

  outputs = { self, nixpkgs, sweet-nothings, ... }: {
    homeConfigurations.yourusername = home-manager.lib.homeManagerConfiguration {
      # ...
      modules = [
        sweet-nothings.homeManagerModules.default
        {
          programs.sweet-nothings = {
            enable = true;
            backends = [ "whisper" ];       # backends to compile in: "whisper", "parakeet"
            defaultBackend = "whisper";      # backend used by default
            model = "base.en";              # default model name
            settings = {                    # additional config.toml settings
              auto_paste = false;
              exit_delay = "2s";
            };
          };
        }
      ];
    };
  };
}
```

With Parakeet:

```nix
programs.sweet-nothings = {
  enable = true;
  backends = [ "whisper" "parakeet" ];
  defaultBackend = "parakeet";
  model = "tdt-0.6b";
};
```

### Nix Module (NixOS)

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    sweet-nothings.url = "github:jordangarrison/sweet-nothings";
  };

  outputs = { self, nixpkgs, sweet-nothings, ... }: {
    nixosConfigurations.yourhostname = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        sweet-nothings.nixosModules.default
        {
          programs.sweet-nothings = {
            enable = true;
            backends = [ "whisper" ];
          };
        }
      ];
    };
  };
}
```

#### Module Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable` | bool | `false` | Enable sweet-nothings |
| `backends` | list of enum | `[ "whisper" ]` | Backends to compile in (`"whisper"`, `"parakeet"`) |
| `defaultBackend` | string | `"whisper"` | Backend used when not specified via CLI |
| `model` | string | `"base.en"` | Default model name for the selected backend |
| `package` | package | auto-built | Override the sweet-nothings package |
| `settings` | attrs | `{}` | Additional `config.toml` settings (e.g., `auto_paste`, `exit_delay`) |

### Nix Packages (without module)

If you prefer manual package installation without the module:

```nix
# In environment.systemPackages or home.packages:
sweet-nothings.packages.${system}.default       # whisper only
sweet-nothings.packages.${system}.full           # whisper + parakeet
sweet-nothings.packages.${system}.parakeet-only  # parakeet only
```

### Devbox

```bash
# Add to your devbox.json
devbox add github:jordangarrison/sweet-nothings
```

Or manually add to your `devbox.json`:

```json
{
  "packages": [
    "github:jordangarrison/sweet-nothings"
  ]
}
```

### From Source

```bash
# Clone the repository
git clone https://github.com/jordangarrison/sweet-nothings
cd sweet-nothings

# With Nix
nix develop
cargo build --release

# Without Nix (requires ALSA dev libraries)
cargo build --release

# Build with Parakeet support (requires ONNX Runtime)
cargo build --release --features parakeet

# Build with ffmpeg support (auto-converts mp3/mp4/flac/ogg to WAV)
cargo build --release --features ffmpeg

# Build with everything
cargo build --release --features "whisper parakeet ffmpeg"
```

## Usage

### Basic Usage

```bash
# Start dictation (TUI mode)
sweet-nothings

# With auto-paste
sweet-nothings --paste

# Use a different model
sweet-nothings --model small.en

# Use a specific backend
sweet-nothings --backend parakeet
sweet-nothings --backend parakeet --model tdt-0.6b
```

### File Transcription

Transcribe existing audio files without the recording TUI:

```bash
# WAV files work out of the box
sweet-nothings --file meeting.wav

# With a specific backend
sweet-nothings --file recording.wav --backend parakeet --model tdt-0.6b

# Transcribe and auto-paste
sweet-nothings --file recording.wav --paste

# Other formats require the ffmpeg feature
sweet-nothings --file interview.mp3    # needs ffmpeg feature
sweet-nothings --file podcast.mp4      # needs ffmpeg feature
```

The `ffmpeg` feature automatically converts non-WAV files to the required format. Without it, only WAV files are accepted.

### Model Management

```bash
# List installed models
sweet-nothings models list

# Show available models for the default backend
sweet-nothings models available

# Show available models for a specific backend
sweet-nothings models available --backend parakeet

# Download a model
sweet-nothings models download base.en
sweet-nothings models download tdt-0.6b --backend parakeet
```

### Configuration

```bash
# Show current config
sweet-nothings config show

# Show config file path
sweet-nothings config path

# Set a config value
sweet-nothings config set backend parakeet
sweet-nothings config set model small.en
sweet-nothings config set auto_paste true
sweet-nothings config set exit_delay 3s
```

## Configuration File

Configuration is stored at `$XDG_CONFIG_HOME/sweet-nothings/config.toml`:

```toml
backend = "whisper"
model = "base.en"
auto_paste = false
exit_delay = "2s"
```

## Window Manager Integration

### Niri

```kdl
binds {
    Mod+D {
        spawn "alacritty" "--class" "sweet-nothings" "-e" "sweet-nothings" "--paste";
    }
}

window-rules {
    match app-id="sweet-nothings" {
        open-floating true
        default-column-width { proportion 0.4; }
    }
}
```

### Hyprland

```conf
bind = $mainMod, D, exec, alacritty --class sweet-nothings -e sweet-nothings --paste
windowrulev2 = float,class:(sweet-nothings)
windowrulev2 = size 40% 30%,class:(sweet-nothings)
windowrulev2 = center,class:(sweet-nothings)
```

### Sway / i3

```conf
bindsym $mod+d exec alacritty --class sweet-nothings -e sweet-nothings --paste
for_window [app_id="sweet-nothings"] floating enable, resize set 600 400
```

## CLI Options

```
sweet-nothings [OPTIONS] [COMMAND]

Options:
  -b, --backend <BACKEND>    Transcription backend [default: from config]
  -f, --file <FILE>          Transcribe a file directly (skip recording TUI)
  -m, --model <MODEL>        Model to use [default: base.en]
  -p, --paste                Auto-paste transcription after completion
      --exit-delay <DELAY>   Delay before exiting after result
      --whisper-path <PATH>  Path to whisper-cli binary
      --models-dir <DIR>     Path to models directory
  -h, --help                 Print help
  -V, --version              Print version

Commands:
  models  Manage transcription models
  config  Show or modify configuration
```

## Available Models

### Whisper (default)

| Model       | Size     | Description                           |
|-------------|----------|---------------------------------------|
| tiny.en     | ~75 MB   | Fastest, lowest quality (English)    |
| tiny        | ~75 MB   | Fastest, multilingual                 |
| base.en     | ~142 MB  | Good balance, recommended (English)  |
| base        | ~142 MB  | Good balance, multilingual            |
| small.en    | ~466 MB  | Better quality (English)              |
| small       | ~466 MB  | Better quality, multilingual          |
| medium.en   | ~1.5 GB  | High quality (English)                |
| medium      | ~1.5 GB  | High quality, multilingual            |
| large-v3    | ~3.1 GB  | Highest quality, multilingual         |

### Parakeet (requires `parakeet` feature)

| Model       | Size     | Description                                    |
|-------------|----------|------------------------------------------------|
| tdt-0.6b    | ~2.5 GB  | TDT 0.6B — recommended, multilingual          |
| tdt-1.1b    | ~4.3 GB  | TDT 1.1B — highest accuracy, multilingual     |
| ctc-0.6b    | ~2.4 GB  | CTC 0.6B — English only, fast                 |

## Requirements

- ALSA (for audio capture on Linux)
- wtype (Wayland) or xdotool (X11) for paste simulation
- **Whisper backend:** whisper-cpp (whisper-cli binary)
- **Parakeet backend:** ONNX Runtime
- **ffmpeg feature:** ffmpeg (for non-WAV file conversion)

## License

MIT
