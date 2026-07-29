# Sweet Nothings - Shared NixOS / Home Manager module
#
# Works in both contexts. Uses home.packages and xdg.configFile
# for Home Manager; for NixOS use environment.systemPackages manually
# with the package from this module.
#
# Usage (Home Manager):
#   imports = [ sweet-nothings.homeManagerModules.default ];
#   programs.sweet-nothings = {
#     enable = true;
#     backends = [ "whisper" "parakeet" ];
#     defaultBackend = "parakeet";
#     model = "tdt-0.6b";
#     settings.preferred_words = [ "Mikayla" "Isla" ];
#   };
#
# Usage (NixOS):
#   imports = [ sweet-nothings.nixosModules.default ];
#   programs.sweet-nothings = {
#     enable = true;
#     backends = [ "whisper" ];
#   };

flake:
{ config, lib, pkgs, ... }:

let
  cfg = config.programs.sweet-nothings;
  system = pkgs.stdenv.hostPlatform.system;
  tomlFormat = pkgs.formats.toml {};

  # Build the package with selected backends
  package =
    if (flake.lib ? ${system})
    then flake.lib.${system}.buildSweetNothings { features = cfg.backends; }
    else flake.packages.${system}.default;

  # Generate config.toml content
  configSettings = {
    backend = cfg.defaultBackend;
    model = cfg.model;
    auto_paste = cfg.settings.auto_paste or false;
    exit_delay = cfg.settings.exit_delay or "2s";
  } // (builtins.removeAttrs (cfg.settings) [ "auto_paste" "exit_delay" ]);

  configFile = tomlFormat.generate "sweet-nothings-config.toml" configSettings;

  needsConfig = cfg.defaultBackend != "whisper" || cfg.model != "base.en" || cfg.settings != {};

in
{
  options.programs.sweet-nothings = {
    enable = lib.mkEnableOption "sweet-nothings dictation tool";

    package = lib.mkOption {
      type = lib.types.package;
      default = package;
      defaultText = lib.literalExpression "sweet-nothings built with selected backends";
      description = "The sweet-nothings package to use.";
    };

    backends = lib.mkOption {
      type = lib.types.listOf (lib.types.enum [ "whisper" "parakeet" "ffmpeg" ]);
      default = [ "whisper" ];
      description = "Transcription backends to compile in.";
    };

    defaultBackend = lib.mkOption {
      type = lib.types.str;
      default = "whisper";
      description = "Default backend when not specified via CLI.";
    };

    model = lib.mkOption {
      type = lib.types.str;
      default = "base.en";
      description = "Default model name for the selected backend.";
    };

    settings = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = {};
      description = "Additional config.toml settings (e.g., auto_paste, exit_delay).";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."sweet-nothings/config.toml" = lib.mkIf needsConfig {
      source = configFile;
    };
  };
}
