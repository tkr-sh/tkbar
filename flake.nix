{
    description = "tkbar - GTK4 layer-shell status bar";

    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
        systems.url = "github:nix-systems/default";
        rust-overlay.url = "github:oxalica/rust-overlay";
        flake-utils.url  = "github:numtide/flake-utils";
    };

    outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
        flake-utils.lib.eachDefaultSystem (system:
        let
            overlays = [ (import rust-overlay) ];
            pkgs = import nixpkgs {
                inherit system overlays;
            };
            rustToolchain = pkgs.rust-bin.nightly.latest.default;
            rustPlatform = pkgs.makeRustPlatform {
                cargo = rustToolchain;
                rustc = rustToolchain;
            };
        in
        {
            devShells.default = with pkgs; mkShell {
                buildInputs = [
                    rust-bin.nightly.latest.default
                    just
                    nushell
                    taplo
                    watchexec
                    cargo-nextest
                    cargo-machete

                    # Nice utilities
                    fd
                    ripgrep

                    # GTK stack
                    gtk4
                    gtk4-layer-shell
                    glib
                    gsettings-desktop-schemas

                    # Runtime tools the bar shells out to
                    brightnessctl
                    wireplumber
                    iwd
                ];
                nativeBuildInputs = with pkgs; [
                    pkg-config
                    wrapGAppsHook4
                ];

                # GTK looks up gsettings schemas and icons via XDG_DATA_DIRS;
                # without this the binary can abort at startup in a pure shell.
                shellHook = ''
                    export XDG_DATA_DIRS=${gsettings-desktop-schemas}/share/gsettings-schemas/${gsettings-desktop-schemas.name}:${gtk4}/share/gsettings-schemas/${gtk4.name}:$XDG_DATA_DIRS
                '';
            };

            packages.default = rustPlatform.buildRustPackage {
                pname = "tkbar";
                version = "0.1.0";
                src = ./.;
                cargoLock.lockFile = ./Cargo.lock;


                nativeBuildInputs = with pkgs; [ pkg-config wrapGAppsHook4 ];
                buildInputs = with pkgs; [
                    gtk4
                    gtk4-layer-shell
                    glib
                    gsettings-desktop-schemas
                ];

                # Make the tools the bar shells out to resolvable at runtime.
                postFixup = ''
                    wrapProgram $out/bin/tkbar \
                      --prefix PATH : ${
                          pkgs.lib.makeBinPath [
                              pkgs.brightnessctl
                              pkgs.wireplumber
                              pkgs.iwd
                          ]
                      }
                '';
            };

            apps.default = {
                type = "app";
                program = "${self.packages.${system}.default}/bin/tkbar";
            };
        }
    );
}
