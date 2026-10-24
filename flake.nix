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
                                defaultText = lib.literalMD "the flake's default package";
                                description = ''
                                The tkbar package to install.

                                Use `tkbar.packages.${pkgs.stdenv.hostPlatform.system}.default.override { ... }`
                                to select the compiled-in theme, compositor backend, and components.
                                '';
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

            homeManagerModule = { config, lib, pkgs, ... }:
                let
                    cfg = config.programs.tkbar;
                    toml = pkgs.formats.toml { };

                    settingsType = lib.types.submodule {
                        options = {
                            style = lib.mkOption {
                                type = lib.types.nullOr (lib.types.submodule {
                                    options = {
                                        position = lib.mkOption {
                                            type = lib.types.nullOr (lib.types.enum [ "left" "right" "top" "bottom" ]);
                                            default = null;
                                            description = "Bar edge. `left`/`right` are vertical, `top`/`bottom` horizontal.";
                                        };

                                        bar_size_px = lib.mkOption {
                                            type = lib.types.nullOr lib.types.ints.unsigned;
                                            default = null;
                                            description = "Bar thickness in pixels (width for vertical bars, height for horizontal ones).";
                                        };

                                        outputs = lib.mkOption {
                                            type = lib.types.nullOr (lib.types.listOf lib.types.str);
                                            default = null;
                                            description = "Outputs to draw a bar on. Null or an empty list shows a bar on every output.";
                                        };
                                    };
                                });
                                default = null;
                                description = "Bar geometry.";
                            };

                            behaviour = lib.mkOption {
                                type = lib.types.nullOr (lib.types.submodule {
                                    options = {
                                        components = lib.mkOption {
                                            type = lib.types.nullOr (lib.types.listOf (lib.types.either
                                                (lib.types.enum [ "workspaces" "spacer" "battery" "wifi" "brightness" "volume" "clock" ])
                                                (lib.types.submodule {
                                                    options.logo = lib.mkOption {
                                                        type = lib.types.str;
                                                        description = "Single glyph to display.";
                                                    };
                                                })
                                            ));
                                            default = null;
                                            description = ''
                                            Ordered widgets to render.

                                            `workspaces` requires a compositor backend, and the
                                            feature-gated components require the matching
                                            compiled-in feature.
                                            '';
                                        };

                                        should_show_empty_workspace = lib.mkOption {
                                            type = lib.types.nullOr lib.types.bool;
                                            default = null;
                                            description = "Show a button for empty workspaces.";
                                        };

                                        on_scroll_brightness_step = lib.mkOption {
                                            type = lib.types.nullOr lib.types.ints.u8;
                                            default = null;
                                            description = "Percentage points added or removed per scroll notch on the brightness widget.";
                                        };

                                        on_scroll_volume_step = lib.mkOption {
                                            type = lib.types.nullOr lib.types.ints.u8;
                                            default = null;
                                            description = "Percentage points added or removed per scroll notch on the volume widget.";
                                        };

                                        workspace_count = lib.mkOption {
                                            type = lib.types.nullOr lib.types.ints.u8;
                                            default = null;
                                            description = "Number of workspace slots to synthesize when fewer real workspaces exist.";
                                        };
                                    };
                                });
                                default = null;
                                description = "Bar behaviour.";
                            };

                            security = lib.mkOption {
                                type = lib.types.nullOr (lib.types.submodule {
                                    options.should_allow_workspace_label = lib.mkOption {
                                        type = lib.types.nullOr lib.types.bool;
                                        default = null;
                                        description = "Display compositor-provided workspace names, which are untrusted input.";
                                    };
                                });
                                default = null;
                                description = "Security-related options.";
                            };
                        };
                    };

                    settings = lib.filterAttrsRecursive (_: value: value != null) cfg.settings;
                    hasSettings = settings != { };
                in
                    {
                        options.programs.tkbar = {
                            enable = lib.mkEnableOption "tkbar";

                            package = lib.mkOption {
                                type = lib.types.package;
                                default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
                                defaultText = lib.literalMD "the flake's default package";
                                description = ''
                                The tkbar package to install.

                                Use `tkbar.packages.${pkgs.stdenv.hostPlatform.system}.default.override { ... }`
                                to select the compiled-in theme, compositor backend, and components.
                                '';
                            };

                            settings = lib.mkOption {
                                type = settingsType;
                                default = { };
                                description = ''
                                Configuration written to
                                {file}`$XDG_CONFIG_HOME/tkbar/config.toml`.

                                Unset fields fall back to the built-in defaults.

                                See <https://github.com/tkr-sh/tkbar/blob/main/docs/CONFIGURATION.md>
                                for the available options.
                                '';
                                example = lib.literalExpression ''
                                {
                                    style.position = "top";
                                    behaviour.components = [ "workspaces" "spacer" "clock" ];
                                }
                                '';
                            };

                            style = lib.mkOption {
                                type = lib.types.nullOr (lib.types.either lib.types.path lib.types.lines);
                                default = null;
                                description = ''
                                User CSS written to
                                {file}`$XDG_CONFIG_HOME/tkbar/style.css`, or a path to
                                an existing CSS file.

                                See <https://github.com/tkr-sh/tkbar/blob/main/docs/CONFIGURATION.md#css-overloading>
                                for details.
                                '';
                                example = lib.literalExpression "./style.css";
                            };

                            systemd = {
                                enable = lib.mkOption {
                                    type = lib.types.bool;
                                    default = false;
                                    description = "Whether to install a systemd user service for tkbar.";
                                };

                                targets = lib.mkOption {
                                    type = lib.types.listOf lib.types.str;
                                    default = [ config.wayland.systemd.target ];
                                    defaultText = lib.literalExpression "[ config.wayland.systemd.target ]";
                                    example = [ "niri-session.target" ];
                                    description = ''
                                    systemd user targets that start tkbar.

                                    Make sure the compositor starts the selected target,
                                    otherwise the service never runs.
                                    '';
                                };
                            };
                        };

                        config = lib.mkIf cfg.enable (lib.mkMerge [
                            {
                                assertions = [
                                    (lib.hm.assertions.assertPlatform "programs.tkbar" pkgs lib.platforms.linux)
                                ];

                                home.packages = [ cfg.package ];

                                xdg.configFile."tkbar/config.toml" = lib.mkIf hasSettings {
                                    source = toml.generate "tkbar-config.toml" settings;
                                };

                                xdg.configFile."tkbar/style.css" = lib.mkIf (cfg.style != null) {
                                    source =
                                        if builtins.isPath cfg.style || lib.isStorePath cfg.style
                                        then cfg.style
                                        else pkgs.writeText "tkbar-style.css" cfg.style;
                                };
                            }

                            (lib.mkIf cfg.systemd.enable {
                                systemd.user.services.tkbar = {
                                    Unit = {
                                        Description = "tkbar status bar";
                                        Documentation = [ "https://github.com/tkr-sh/tkbar" ];
                                        PartOf = cfg.systemd.targets;
                                        After = cfg.systemd.targets;
                                        ConditionEnvironment = "WAYLAND_DISPLAY";
                                        X-Restart-Triggers =
                                            lib.optional hasSettings config.xdg.configFile."tkbar/config.toml".source
                                            ++ lib.optional (cfg.style != null) config.xdg.configFile."tkbar/style.css".source;
                                    };

                                    Service = {
                                        ExecStart = "${cfg.package}/bin/tkbar";
                                        Restart = "on-failure";
                                    };

                                    Install.WantedBy = cfg.systemd.targets;
                                };
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
                    cargo-deny
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
            #   compositor = "hyprland";        # one of: niri hyprland sway, or null for no compositor backend
            #   withConfig = false;             # set false to drop the optional TOML/CSS config
            #   withCli = true;                 # set true to enable the clap CLI (--config, --help, --version)
            #   components = [ "battery" "brightness" "clock" "wifi" "volume" ];  # feature-gated components to build in
            # }
            packages.default = pkgs.lib.makeOverridable
                ({ color ? "black", compositor ? "niri", withConfig ? true, withCli ? false, components ? [ "battery" "brightness" "clock" "wifi" "volume" ] }:
                let
                    # Component names mapping 1:1 to Cargo features.
                    allowedComponents = [ "battery" "brightness" "clock" "volume" "wifi" ];
                    allowedCompositors = [ "niri" "hyprland" "sway" ];
                    withVolume = builtins.elem "volume" components;
                    invalidComponents =
                        builtins.filter (c: !(builtins.elem c allowedComponents)) components;
                    invalidCompositor =
                        compositor != null && !(builtins.elem compositor allowedCompositors);
                in
                if invalidCompositor
                then throw (
                    "tkbar: unsupported compositor backend '${toString compositor}'. "
                    + "Allowed: " + builtins.concatStringsSep ", " allowedCompositors
                    + ", or null."
                )
                else if invalidComponents != []
                then throw (
                    "tkbar: unsupported component(s) in 'components': "
                    + builtins.concatStringsSep ", " invalidComponents
                    + ". Allowed: " + builtins.concatStringsSep ", " allowedComponents
                )
                else rustPlatform.buildRustPackage {
                    pname = "tkbar";
                    version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
                    src = ./.;
                    cargoLock.lockFile = ./Cargo.lock;

                    buildNoDefaultFeatures = true;
                    buildFeatures = [ color ]
                        ++ pkgs.lib.optional (compositor != null) compositor
                        ++ pkgs.lib.optional withConfig "config"
                        ++ pkgs.lib.optional withCli "cli"
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

            homeModules.default = homeManagerModule;
            homeModules.tkbar = homeManagerModule;
        };
}
