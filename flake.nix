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
        let
            nixosModule = { config, lib, pkgs, ... }:
                let
                    cfg = config.programs.tkbar;
                    bl = cfg.backlight;

                    backlightUdevRules = pkgs.writeTextDir
                        "etc/udev/rules.d/99-tkbar-backlight.rules"
                        ''
                        ACTION=="add", SUBSYSTEM=="backlight", KERNEL=="*", RUN+="${pkgs.coreutils}/bin/chgrp ${bl.group} /sys/class/backlight/%k/brightness"
                        ACTION=="add", SUBSYSTEM=="backlight", KERNEL=="*", RUN+="${pkgs.coreutils}/bin/chmod g+rw /sys/class/backlight/%k/brightness"
                        '';
                in
                    {
                        options.programs.tkbar = {
                            enable = lib.mkEnableOption "tkbar";

                            package = lib.mkOption {
                                type = lib.types.package;
                                default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
                                description = "The tkbar package to install.";
                            };

                            backlight = {
                                enable = lib.mkEnableOption ''
                                Install a udev rule giving a dedicated group write access to
                                /sys/class/backlight/*/brightness.

                                This avoids setuid binaries.
                                '';

                                group = lib.mkOption {
                                    type = lib.types.str;
                                    default = "tkbar-backlight";
                                    description = ''
                                    Group that will get write access to backlight brightness files.

                                    You can also set this to "video" if you prefer using the
                                    conventional Linux video group.
                                    '';
                                };

                                users = lib.mkOption {
                                    type = lib.types.listOf lib.types.str;
                                    default = [ ];
                                    description = ''
                                    Users to add to the backlight group.

                                    Example:

                                    programs.tkbar.backlight.users = [ "alice" ];
                                    '';
                                    };
                                };
                            };

                            config = lib.mkIf cfg.enable (lib.mkMerge [
                                {
                                    environment.systemPackages = [ cfg.package ];
                                }

                                (lib.mkIf bl.enable {
                                    users.groups.${bl.group} = { };

                                    services.udev.packages = [ backlightUdevRules ];
                                })

                                (lib.mkIf (bl.enable && bl.users != [ ]) {
                                    users.groups.${bl.group}.members = bl.users;
                                })
                            ]);
                    };
    in
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
                    (rust-bin.nightly.latest.default.override {
                        extensions = [
                            "rust-src"
                            "rust-analyzer"
                            "clippy"
                            "rustfmt"
                        ];
                    })
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

                    pipewire
                ];
                nativeBuildInputs = with pkgs; [
                    pkg-config
                    wrapGAppsHook4
                    rustPlatform.bindgenHook
                ];

                shellHook = ''
                    export GSETTINGS_SCHEMA_DIR=${gsettings-desktop-schemas}/share/gsettings-schemas/${gsettings-desktop-schemas.name}:${gtk4}/share/gsettings-schemas/${gtk4.name}
                '';
            };

            packages.dagger = dagger.packages.${system}.dagger;

            # tkbar.packages.${system}.default.override {
            #   color = "purple";               # one of: black blue cyan green orange pink purple red white yellow
            #   workspace = "hyprland";         # one of: niri hyprland sway
            #   withConfig = false;             # set false to drop the optional TOML/CSS config
            #   components = [ "wifi" "volume" ];        # feature-gated components to build in
            # }
            packages.default = pkgs.lib.makeOverridable
                ({ color ? "black", workspace ? "niri", withConfig ? true, components ? [ "wifi" "volume" ] }:
                let
                    # Component names mapping 1:1 to Cargo features. Both
                    # "wifi" and "volume" are feature-gated; "brightness" will
                    # follow once its module is gated.
                    allowedComponents = [ "volume" "wifi" ];
                    withVolume = builtins.elem "volume" components;
                    invalidComponents =
                        builtins.filter (c: !(builtins.elem c allowedComponents)) components;
                in
                if invalidComponents != []
                then throw (
                    "tkbar: unsupported component(s) in 'components': "
                    + builtins.concatStringsSep ", " invalidComponents
                    + ". Allowed: " + builtins.concatStringsSep ", " allowedComponents
                    + " (brightness will follow once feature-gated)"
                )
                else rustPlatform.buildRustPackage {
                    pname = "tkbar";
                    version = "0.1.0";
                    src = ./.;
                    cargoLock.lockFile = ./Cargo.lock;

                    buildNoDefaultFeatures = true;
                    buildFeatures = [ color workspace ]
                        ++ pkgs.lib.optional withConfig "config"
                        ++ components;

                    # Mirror `harden_gtk_env` in src/main.rs at the wrapper
                    # level as well (defense in depth): even if the in-binary
                    # calls were bypassed, the wrapper still disables image
                    # loaders, AT-SPI, GL/Vulkan and the X11 fallback.
                    # Pointing gdk-pixbuf at /dev/null keeps GTK from
                    # dlopening image loader modules (e.g. librsvg's SVG/XML
                    # parser) through gdk-pixbuf's default loader cache.
                    preFixup = ''
                        gappsWrapperArgs+=(
                            "--set" "GDK_PIXBUF_MODULE_FILE" "/dev/null"
                            "--set" "GTK_A11Y" "none"
                            "--set" "GSK_RENDERER" "cairo"
                            "--set" "GDK_DISABLE" "gl,vulkan,dmabuf"
                            "--set" "GDK_BACKEND" "wayland"
                            "--set" "GDK_DEBUG" "no-portals"
                        )
                    '';

                    nativeBuildInputs = with pkgs; [ pkg-config wrapGAppsHook4 ]         ++ pkgs.lib.optional withVolume rustPlatform.bindgenHook;
                    buildInputs = with pkgs; [
                        gtk4
                        gtk4-layer-shell
                        glib
                        gsettings-desktop-schemas
                    ]
                      ++ pkgs.lib.optional withVolume pipewire;
                })
                {};


            apps.default = {
                type = "app";
                program = "${self.packages.${system}.default}/bin/tkbar";
            };
        }) // {
            nixosModules.default = nixosModule;
            nixosModules.tkbar = nixosModule;
        };
}
