{
    description = "tkbar - GTK4 layer-shell status bar";

    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
        systems.url = "github:nix-systems/default";
        rust-overlay.url = "github:oxalica/rust-overlay";
        flake-utils.url  = "github:numtide/flake-utils";
        dagger.url = "github:dagger/nix";
        dagger.inputs.nixpkgs.follows = "nixpkgs";
    };

    outputs = { self, nixpkgs, flake-utils, rust-overlay, dagger, ... }:
        flake-utils.lib.eachDefaultSystem (system:
        let
            overlays = [ (import rust-overlay) ];
            pkgs = import nixpkgs {
                inherit system overlays;
            };
            # Nightly is only needed for the dev shell (advanced rustfmt/clippy);
            # release artifacts are built with the stable toolchain.
            rustPlatform = pkgs.makeRustPlatform {
                cargo = pkgs.rust-bin.stable.latest.default;
                rustc = pkgs.rust-bin.stable.latest.default;
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
                    dart-sass
                    dagger.packages.${system}.dagger

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

            packages.dagger = dagger.packages.${system}.dagger;

            # tkbar.packages.${system}.default.override {
            #   color = "purple";               # one of: black blue cyan green orange pink purple red white yellow
            #   workspace = "hyprland";         # one of: niri hyprland sway
            #   withConfig = false;             # set false to drop the optional TOML/CSS config
            # }
            packages.default = pkgs.lib.makeOverridable
                ({ color ? "black", workspace ? "niri", withConfig ? true }:
                rustPlatform.buildRustPackage {
                    pname = "tkbar";
                    version = "0.1.0";
                    src = ./.;
                    cargoLock.lockFile = ./Cargo.lock;

                    buildNoDefaultFeatures = true;
                    buildFeatures = [ color workspace ] ++ pkgs.lib.optional withConfig "config";

                    # The bar renders icons from font glyphs and never loads
                    # images. Point gdk-pixbuf at an empty loader cache so
                    # wrapGAppsHook4 does not wire in image loader modules
                    # (librsvg's SVG/XML parser otherwise ends up in the
                    # runtime closure and dlopened by GTK).
                    postInstall = ''
                        mkdir -p $out/lib/gdk-pixbuf-2.0/2.10.0
                        printf '%s\n' \
                            '# GdkPixbuf Image Loader Modules file' \
                            '# tkbar loads no images: keep every loader module unregistered.' \
                            > $out/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache
                        export GDK_PIXBUF_MODULE_FILE=$out/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache
                    '';

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
                })
                {};


            apps.default = {
                type = "app";
                program = "${self.packages.${system}.default}/bin/tkbar";
            };
        }
    );
}
