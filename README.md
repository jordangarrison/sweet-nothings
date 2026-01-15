# Sweet Nothings

*Whisper sweet nothings to your Linux computer.*

A terminal-based dictation tool that records speech, transcribes it locally with [whisper.cpp](https://github.com/ggerganov/whisper.cpp), and copies the result to your clipboard. The name is a playful nod to the phrase "whispering sweet nothings" — except here, you're whispering to your machine and it actually listens.

## Features

- Record audio from your microphone
- Transcribe locally using whisper.cpp (no cloud required)
- Copy transcription to clipboard
- Optional auto-paste after transcription
- TUI with visual feedback (recording timer, audio level, spinner)
- Automatic model downloading
- XDG-compliant configuration

## Installation

### Nix (Flakes)

```bash
# Run directly
nix run github:jordangarrison/sweet-nothings

# Install to profile
nix profile install github:jordangarrison/sweet-nothings
```

### NixOS Configuration

Add as a flake input in your `flake.nix`:

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
        ({ pkgs, ... }: {
          environment.systemPackages = [
            sweet-nothings.packages.${pkgs.system}.default
          ];
        })
      ];
    };
  };
}
```

### Home Manager

```nix
{ inputs, pkgs, ... }: {
  home.packages = [
    inputs.sweet-nothings.packages.${pkgs.system}.default
  ];
}
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
```

### Model Management

```bash
# List installed models
sweet-nothings models list

# Show available models
sweet-nothings models available

# Download a model
sweet-nothings models download base.en
```

### Configuration

```bash
# Show current config
sweet-nothings config show

# Show config file path
sweet-nothings config path

# Set a config value
sweet-nothings config set model small.en
sweet-nothings config set auto_paste true
sweet-nothings config set exit_delay 3s
```

## Configuration File

Configuration is stored at `$XDG_CONFIG_HOME/sweet-nothings/config.toml`:

```toml
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
  -m, --model <MODEL>        Whisper model to use [default: base.en]
  -p, --paste                Auto-paste transcription after completion
      --exit-delay <DELAY>   Delay before exiting after result
      --whisper-path <PATH>  Path to whisper-cli binary
      --models-dir <DIR>     Path to models directory
  -h, --help                 Print help
  -V, --version              Print version

Commands:
  models  Manage whisper models
  config  Show or modify configuration
```

## Available Models

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

## Requirements

- whisper-cpp (whisper-cli binary)
- ALSA (for audio capture on Linux)
- wtype (Wayland) or xdotool (X11) for paste simulation

## License

MIT
