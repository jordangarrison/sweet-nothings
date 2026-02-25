{
  description = "Sweet Nothings - Terminal-based dictation tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    {
      # Modules (not system-specific)
      homeManagerModules.default = import ./nix/module.nix self;
      nixosModules.default = import ./nix/module.nix self;
    } //
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # Build dependencies (always needed)
        baseBuildInputs = with pkgs; [
          alsa-lib
          openssl
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
          makeWrapper
        ];

        # Runtime deps per backend
        whisperRuntimeDeps = with pkgs; [ whisper-cpp ];
        parakeetRuntimeDeps = with pkgs; [ onnxruntime ];

        commonRuntimeDeps = with pkgs; [
          wtype
          xdotool
          wl-clipboard
          xclip
        ];

        # ALSA config with PipeWire support for dev shell
        alsaConfigWithPipewire = pkgs.runCommand "alsa-config-pipewire" {} ''
          mkdir -p $out/conf.d
          cp ${pkgs.alsa-lib}/share/alsa/alsa.conf $out/
          if [ -d "${pkgs.alsa-lib}/share/alsa/conf.d" ]; then
            cp -r ${pkgs.alsa-lib}/share/alsa/conf.d/* $out/conf.d/ 2>/dev/null || true
          fi
          cp ${pkgs.pipewire}/share/alsa/alsa.conf.d/* $out/conf.d/ 2>/dev/null || true
        '';

        # Parameterized build function
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
          pkgs.rustPlatform.buildRustPackage {
            pname = "sweet-nothings";
            version = "0.1.0";
            src = ./.;
            cargoLock = { lockFile = ./Cargo.lock; };

            buildInputs = baseBuildInputs ++ extraBuildInputs;
            inherit nativeBuildInputs;

            buildNoDefaultFeatures = true;
            buildFeatures = features;

            postInstall = ''
              wrapProgram $out/bin/sweet-nothings \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
            '';

            meta = with pkgs.lib; {
              description = "Terminal-based dictation tool";
              license = licenses.mit;
            };
          };

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = baseBuildInputs;
          inherit nativeBuildInputs;

          packages = with pkgs; [
            whisper-cpp
            alsa-plugins
            pipewire
            ffmpeg
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
          ALSA_PLUGIN_DIR = "${pkgs.pipewire}/lib/alsa-lib";
          ALSA_CONFIG_DIR = "${alsaConfigWithPipewire}";
        };

        packages = {
          default = buildSweetNothings { features = [ "whisper" ]; };
          full = buildSweetNothings { features = [ "whisper" "parakeet" "ffmpeg" ]; };
          whisper-only = buildSweetNothings { features = [ "whisper" ]; };
          parakeet-only = buildSweetNothings { features = [ "parakeet" ]; };
        };

        # Expose build function for the nix module
        lib = { inherit buildSweetNothings; };
      }
    );
}
