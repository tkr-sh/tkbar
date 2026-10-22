# Configuration

Configuration is optional and deliberately limited. It lives behind the
`config` Cargo feature; without it, the bar always uses the hardcoded default
and no TOML parser is linked at all.

## The TOML config file

With the `config` feature, the bar reads
`$XDG_CONFIG_HOME/tkbar/config.toml` (usually
`~/.config/tkbar/config.toml`). Parsing is handled in
[`src/conf.rs`](../src/conf.rs).

The path can be overridden at runtime with the `--config <PATH>` CLI flag
(requires the `cli` and `config` features, both enabled by default). When
`--config` is provided, the bar reads from the specified path instead of the
default XDG location.

The file is organised in three optional tables — `[style]`, `[behaviour]`, and
`[security]`. Every table and every field has a default, so a partial (or even
empty) file is valid, and a missing file falls back to the hardcoded default.

### `[style]`

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `position` | `left` \| `right` \| `top` \| `bottom` | `left` | `left`/`right` give a vertical bar anchored to the top and bottom edges; `top`/`bottom` give a horizontal bar anchored to the left and right edges. |
| `bar_size_px` | `usize` | `72` | Bar thickness — width for a vertical bar, height for a horizontal one. |

### `[behaviour]`

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `components` | list | *(see below)* | Ordered list of widgets to render. |
| `should_show_empty_workspace` | `bool` | `true` | Render a button for every workspace, including empty ones. |
| `on_scroll_brightness_step` | `u8` | `3` | Percentage points added or removed per scroll notch on the brightness widget. |
| `on_scroll_volume_step` | `u8` | `3` | Percentage points added or removed per scroll notch on the volume widget. |
| `workspace_count` | `u8` | `10` | Number of workspace slots to synthesise when fewer real workspaces exist. |

### `[security]`

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `should_allow_workspace_label` | `bool` | `false` | Display custom workspace names instead of numeric IDs. Workspace names are untrusted input, so this is off by default. |

### `components`

Each entry is either a string naming a built-in component, or an inline table
of the form `{ logo = "..." }` for the logo glyph.

Built-in string names:

- `workspaces` (needs a WM backend feature)
- `spacer`
- `battery`
- `wifi` (needs the `wifi` feature)
- `brightness`
- `volume` (needs the `volume` feature)
- `clock`

Inline table:

- `{ logo = "󱄅" }` — the logo glyph (a single character).

The `Component` enum is defined in
[`src/ui/components/mod.rs`](../src/ui/components/mod.rs). A name whose
component is compiled out is rejected as an unknown variant.

### Full example

```toml
[style]
position = "left"
bar_size_px = 72

[behaviour]
components = [
    { logo = "󱄅" },
    "workspaces",
    "spacer",
    "battery",
    "wifi",
    "brightness",
    "volume",
    "clock",
]
should_show_empty_workspace = true
on_scroll_brightness_step = 3
on_scroll_volume_step = 3
workspace_count = 10

[security]
should_allow_workspace_label = false
```

This is exactly [`docs/config.toml`](./config.toml), generated from the
hardcoded defaults by `just doc`.

### Strictness

- **Unknown keys, unknown component names, and unknown positions are
  rejected** (`deny_unknown_fields` in `src/conf.rs`). A misspelling is a hard
  error, not a silent no-op.
- **A missing file** falls back to the hardcoded default.
- **An invalid file is a hard error**: the bar refuses to start and prints the
  parse error with line and column. Silently falling back would hide typos.

The config is stored in a global `CONFIG: LazyLock<Config>` (`src/conf.rs`) and
read by widgets through `CONFIG.style.position.orientation()` and friends.

## CSS overloading

Each color feature selects a compiled-in base stylesheet (`THEME_CSS` in
[`src/lib.rs`](../src/lib.rs), embedded with `include_str!`). With the `config`
feature, an additional user stylesheet at `~/.config/tkbar/style.css` is loaded
at `STYLE_PROVIDER_PRIORITY_USER` and overrides the base.

Precedence:

```
user stylesheet (STYLE_PROVIDER_PRIORITY_USER)  >  built-in base (STYLE_PROVIDER_PRIORITY_APPLICATION)
```

The user CSS is **plain, inert data.** It is only ever parsed as stylesheet
text and cannot execute code; see the README's security notes. A missing user
stylesheet is ignored; an unreadable one is logged as a warning and skipped.

## Feature opt-in/out via Nix

The flake's `packages.default` is `makeOverridable` with `color`,
`wm`, `withConfig`, and `components` parameters (see
[`flake.nix`](../flake.nix)). A consumer flake selects the compiled-in theme,
the WM backend, whether the optional TOML/CSS config is built in, and
which feature-gated components ship — no forking required:

```nix
tkbar.packages.${system}.default.override {
  color = "purple";        # one of: black blue cyan green orange pink purple red white yellow
  wm = "hyprland";         # one of: niri hyprland sway, or null for no WM backend
  withConfig = false;      # set false to drop the optional TOML/CSS config
  withCli = true;          # set true to enable the clap CLI (--config, --help, --version)
  components = [ "battery" "brightness" "clock" "wifi" "volume" ]; # feature-gated components to build in; set [ ] to drop them all
}
```

- `color` selects the compiled-in base theme (exactly one of the ten).
- `wm` selects the WM backend (`"niri"`, `"hyprland"`, or
  `"sway"`). At most one is compiled in; `null` builds no WM backend
  (the `workspaces` component is then unavailable).
- `withConfig` toggles the optional `config` feature.
- `withCli` toggles the optional clap-based CLI.
- `components` is the list of feature-gated components to build in. The
  supported names are `"battery"`, `"brightness"`, `"clock"`, `"volume"`, and
  `"wifi"`; set it to `[ ]` to ship a bar without any of those widgets. Each
  name maps 1:1 to a Cargo feature, and unsupported names fail the build with a
  message.

The plain cargo equivalent is:

```sh
cargo build --no-default-features --features purple,hyprland,battery,brightness,clock,wifi,volume
```

## NixOS module

The flake also exposes a NixOS module (`nixosModules.default`, aliased as
`nixosModules.tkbar`) under `programs.tkbar`:

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `programs.tkbar.enable` | bool | `false` | Installs the bar into `environment.systemPackages`. |
| `programs.tkbar.package` | package | the flake's default package | The tkbar package to install; use the `.override { ... }` form above to select theme/backend/features. |
| `programs.tkbar.backlight.enable` | bool | `false` | Installs a udev rule granting a dedicated group write access to `/sys/class/backlight/*/brightness`, so the bar writes sysfs directly — no setuid binary, no external CLI. |
| `programs.tkbar.backlight.group` | string | `"tkbar-backlight"` | Group that gets write access; set it to `"video"` to reuse the conventional Linux video group. |
| `programs.tkbar.backlight.users` | list of strings | `[ ]` | Users added to that group. |

Example:

```nix
programs.tkbar = {
  enable = true;
  backlight = {
    enable = true;
    users = [ "alice" ];
  };
};
```

The udev rule fires on device `add`; after the first `nixos-rebuild switch` on an
already-booted system, apply it without rebooting:

```sh
sudo udevadm trigger --subsystem-match=backlight --action=add
```

## Home Manager module

The flake exposes the module as `homeModules.default` / `homeModules.tkbar`. It
manages the per-user configuration and, optionally, a systemd user service:

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `programs.tkbar.enable` | bool | `false` | Installs the package and manages the configuration files. |
| `programs.tkbar.package` | package | the flake's default package | The tkbar package to install; use the `.override { ... }` form above to select theme/backend/features. |
| `programs.tkbar.settings` | attribute set | `{ }` | Typed subset of the TOML file described above, written to `$XDG_CONFIG_HOME/tkbar/config.toml`. |
| `programs.tkbar.style` | CSS string or path | `null` | Written to `$XDG_CONFIG_HOME/tkbar/style.css`, or linked if a path is given. |
| `programs.tkbar.systemd.enable` | bool | `false` | Installs a systemd user service for tkbar. |
| `programs.tkbar.systemd.targets` | list of strings | `[ config.wayland.systemd.target ]` | Targets that start and stop the service. |

Example:

```nix
programs.tkbar = {
  enable = true;

  settings = {
    style = {
      position = "top";
      bar_size_px = 32;
    };
    behaviour.components = [ "workspaces" "spacer" "battery" "clock" ];
  };

  style = ''
    .bar { background: #1e1e2e; }
  '';

  systemd.enable = true;
};
```

`settings` is a typed attribute set mirroring the TOML schema: unknown keys and
out-of-range values fail at evaluation time, and only the fields you set are
written. At runtime the remaining check is feature availability — a
`workspaces` entry or a component compiled out of the selected package makes
tkbar refuse to start. The systemd service is opt-in and defaults to
`config.wayland.systemd.target`; set `systemd.targets` when the compositor uses
a custom target (for example `[ "niri-session.target" ]`).

## Adding a theme

Ten themes are generated from a single SCSS source,
[`src/ui/styles/main.scss`](../src/ui/styles/main.scss), using
[`scripts/css.nu`](../scripts/css.nu) (which shells out to dart-sass). Run
`just css` to regenerate the compiled `src/ui/styles/<color>.css` files. To add
a color theme, add a palette entry to `scripts/css.nu`, regenerate the CSS, and
register the new `#[cfg]` feature in `src/lib.rs` (both the theme block and
`color_feature_count()`).