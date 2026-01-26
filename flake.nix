{
  description = "Sweet Nothings - Terminal-based whisper dictation";

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

        inherit (pkgs) lib stdenv;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Platform-specific build inputs
        buildInputs = with pkgs; [
          # For model downloads (reqwest with native-tls)
          openssl
        ] ++ lib.optionals stdenv.isLinux [
          # Audio on Linux
          alsa-lib
          # Clipboard on Linux (X11)
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
        ] ++ lib.optionals stdenv.isDarwin [
          # Apple SDK includes all frameworks (CoreAudio, AudioUnit, AudioToolbox, etc.)
          pkgs.apple-sdk
          pkgs.libiconv
        ];

        # Native build dependencies
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
          makeWrapper  # For wrapping the binary
        ];

        # Runtime dependencies for paste simulation and transcription
        runtimeDeps = with pkgs; [
          whisper-cpp  # Transcription (both platforms)
        ] ++ lib.optionals stdenv.isLinux [
          wtype        # Wayland paste simulation
          xdotool      # X11 paste simulation
          wl-clipboard # Wayland clipboard
          xclip        # X11 clipboard fallback
        ];
        # macOS uses osascript (system-provided) - no additional runtime deps needed

      in
      {
        devShells.default = pkgs.mkShell ({
          inherit buildInputs nativeBuildInputs;

          packages = with pkgs; [
            whisper-cpp  # For transcription
          ] ++ runtimeDeps;

          shellHook = ''
            echo "Sweet Nothings dev shell"
            echo "========================"
            echo "whisper-cli: $(which whisper-cli)"
            echo ""
          '';

          # For openssl-sys
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        } // lib.optionalAttrs stdenv.isLinux {
          # For finding ALSA (Linux only)
          PKG_CONFIG_PATH = "${pkgs.alsa-lib.dev}/lib/pkgconfig";
        });

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "sweet-nothings";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit buildInputs nativeBuildInputs;

          # Wrap with runtime dependencies
          postInstall = ''
            wrapProgram $out/bin/sweet-nothings \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
          '';

          meta = with pkgs.lib; {
            description = "Terminal-based whisper dictation tool";
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      }
    );
}
