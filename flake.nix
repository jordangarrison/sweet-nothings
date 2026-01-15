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

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Build dependencies
        buildInputs = with pkgs; [
          # Audio
          alsa-lib

          # For model downloads (reqwest with native-tls)
          openssl

          # Clipboard on Linux
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
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
          whisper-cpp  # Transcription
          wtype        # Wayland paste simulation
          xdotool      # X11 paste simulation
          wl-clipboard # Wayland clipboard
          xclip        # X11 clipboard fallback
        ];

      in
      {
        devShells.default = pkgs.mkShell {
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

          # For finding ALSA
          PKG_CONFIG_PATH = "${pkgs.alsa-lib.dev}/lib/pkgconfig";
        };

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
