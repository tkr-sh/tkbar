# Installation

`tkbar` compiles in exactly one theme and at most one compositor backend at build time.
Neither can be changed at runtime, so pick them when you build or install the
package. The default build uses the `black` theme, the `niri` backend, every
component, the CLI, and the TOML/CSS configuration.

<details>
<summary><strong>Nix / NixOS</strong></summary>

```sh
nix build github:tkr-sh/tkbar
nix profile install github:tkr-sh/tkbar
```

The flake provides NixOS and Home Manager modules, as well as a package `.override` for selecting the theme, compositor backend, and components. See the [NixOS module](./CONFIGURATION.md#️-nixos-module) and [Home Manager module](./CONFIGURATION.md#-home-manager-module) sections in [CONFIGURATION.md](./CONFIGURATION.md).

To override the compiled-in options:

```sh
nix build --impure --expr '
  let
    flake = builtins.getFlake "github:tkr-sh/tkbar";
  in
  flake.packages.${builtins.currentSystem}.default.override {
    color = "purple";
    compositor = "hyprland";
    components = [ "battery" "brightness" "clock" "wifi" "volume" ];
  }
'
```

The same expression works with `nix profile install --impure --expr '...'`. See [Feature opt-in/out via Nix](./CONFIGURATION.md#feature-opt-inout-via-nix) for every option.

</details>

<details>
<summary><strong>Arch Linux</strong></summary>

A [`PKGBUILD`](../packages/PKGBUILD) is provided in [`packages/`](../packages):

```sh
git clone https://github.com/tkr-sh/tkbar.git
cd tkbar/packages
makepkg -si
```

The package builds the default feature set. To target another compositor,
theme, or component set, edit the variables near the top of the PKGBUILD
before running `makepkg`:

```sh
_compositor=hyprland
_theme=purple
_components=(battery brightness clock)
```

`_compositor` is one of `niri`, `hyprland`, or `sway`; `_theme` is one of `black`,
`blue`, `cyan`, `green`, `orange`, `pink`, `purple`, `red`, `white`, or
`yellow`; `_components` is a subset of `battery`, `brightness`, `clock`,
`volume`, and `wifi`. The package installs the binary, the license, and the
backlight udev rule described below.

</details>

<details>
<summary><strong>Other Linux distributions (from source)</strong></summary>

Build dependencies:

| Dependency | Notes |
| --- | --- |
| Rust 1.85 or newer | edition 2024 |
| GTK4 >= 4.12 | development headers |
| gtk4-layer-shell >= 1.0 | development headers |
| pkg-config | locates the C libraries |
| PipeWire headers | only when the `volume` component is enabled |
| clang / libclang | only when the `volume` component is enabled (`bindgen`) |

Install the development packages for GTK4, gtk4-layer-shell, PipeWire,
pkg-config, and clang with your distribution's package manager, then build:

```sh
git clone https://github.com/tkr-sh/tkbar.git
cd tkbar
cargo build --release --locked
sudo install -Dm755 target/release/tkbar /usr/local/bin/tkbar
```

The equivalent of the default build is:

```sh
cargo build --release --locked --no-default-features \
  --features black,niri,cli,config,battery,brightness,clock,volume,wifi
```

Exactly one color feature and at most one compositor backend (`niri`, `hyprland`, or
`sway`) must be enabled. See
[CONTRIBUTING.md](./CONTRIBUTING.md#feature-gating) for the full feature list.
The crate is also published on crates.io:

```sh
cargo install tkbar --locked
```

</details>

## Backlight permissions

The brightness widget writes to `/sys/class/backlight/*/brightness` directly, which requires write access to that file.

> **NixOS:** enabling [`programs.tkbar.backlight.enable`](./CONFIGURATION.md#️-nixos-module) installs the udev rule and adds `backlight.users` to the configured group automatically.

The Arch package installs [`99-tkbar-backlight.rules`](../packages/99-tkbar-backlight.rules), granting the `video` group that access without a setuid binary. Add yourself to the group and re-login:

```sh
sudo usermod -aG video "$USER"
```

If the rule was installed after boot, trigger it immediately:

```sh
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=backlight --action=add
```

For a manual source install, copy the rule yourself:

```sh
sudo install -Dm644 packages/99-tkbar-backlight.rules \
  /usr/lib/udev/rules.d/99-tkbar-backlight.rules
```

The rule grants every member of `video` write access to the backlight. Remove it if that is broader access than you want.

## Autostart

Start `tkbar` from your compositor's configuration:

- **niri** (`~/.config/niri/config.kdl`): `spawn-at-startup "tkbar"`
- **Hyprland** (`~/.config/hypr/hyprland.conf`): `exec-once = tkbar`
- **sway** (`~/.config/sway/config`): `exec tkbar`

The workspace widget only works with the compositor backend compiled into the binary
(`niri` in the default build).

## Configuration

Configuration is optional. When the `config` feature is enabled (the default), tkbar reads `$XDG_CONFIG_HOME/tkbar/config.toml` (usually `~/.config/tkbar/config.toml`) and an optional `$XDG_CONFIG_HOME/tkbar/style.css`. A missing file falls back to the built-in defaults; an invalid file is an error. The `--config <path>` flag overrides the TOML path for one invocation.

See [CONFIGURATION.md](./CONFIGURATION.md) for the format, defaults, and the NixOS and Home Manager modules.

## Verify

When built with the `cli` feature (the default):

```sh
tkbar --version
```
