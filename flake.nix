{
  description = "mimo_manager — keyboard-centric MIME type and default-application manager for Linux";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);

      pkgsFor = system: nixpkgs.legacyPackages.${system};

      mimo_manager =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "mimo_manager";
          version = "0.1.0";

          src = self;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            wrapGAppsHook4
          ];

          buildInputs = with pkgs; [
            glib
            gtk4
          ];

          postInstall = ''
            install -Dm644 ${./resources/dev.mimo_manager.MimoManager.desktop} \
              $out/share/applications/dev.mimo_manager.MimoManager.desktop
            install -Dm644 ${./resources/dev.mimo_manager.MimoManager.svg} \
              $out/share/icons/hicolor/scalable/apps/dev.mimo_manager.MimoManager.svg
          '';

          meta = with pkgs.lib; {
            description = "Keyboard-centric MIME type and default-application manager for Linux";
            license = with licenses; [
              mit
              asl20
            ];
            mainProgram = "mimo_manager";
            platforms = platforms.linux;
          };
        };
    in
    {
      packages = forAllSystems (system: {
        default = mimo_manager (pkgsFor system);
        mimo_manager = mimo_manager (pkgsFor system);
      });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.mimo_manager}/bin/mimo_manager";
          meta.description = "Run mimo_manager";
        };
      });

      devShells = forAllSystems (system: {
        default = (pkgsFor system).mkShell {
          inputsFrom = [ self.packages.${system}.mimo_manager ];
          packages = with pkgsFor system; [
            cargo
            clippy
            glib
            gtk4
            pkg-config
            rust-analyzer
            rustfmt
            shared-mime-info
          ];
          RUST_SRC_PATH = "${(pkgsFor system).rustPlatform.rustLibSrc}";
        };
      });

      checks = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          hm-module = import ./nix/hm-module-check.nix {
            inherit pkgs;
            lib = pkgs.lib;
          };
          mimo_manager = self.packages.${system}.mimo_manager;
        }
      );

      homeManagerModules.default = import ./nix/hm-module.nix self;
      homeManagerModules.mimo_manager = self.homeManagerModules.default;
    };
}
