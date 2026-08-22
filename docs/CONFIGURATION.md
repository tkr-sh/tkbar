# Configuration

Configuration is optional and deliberately limited. It lives behind the
`config` Cargo feature; without it, the bar always uses the hardcoded default
and no TOML parser is linked at all.

## The TOML config file

With the `config` feature, the bar reads
`$XDG_CONFIG_HOME/tkbar/config.toml` (usually
`~/.config/tkbar/config.toml`). Parsing is handled in
[`src/conf.rs`](../src/conf.rs).

### Fields

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `position` | `left` \| `right` \| `top` \| `bottom` | `left` | `left`/`right` give a vertical bar anchored to the top and bottom edges; `top`/`bottom` give a horizontal bar anchored to the left and right edges. |
| `bar_size_px` | `usize` | `72` | Bar thickness — width for a vertical bar, height for a horizontal one. |
| `components` | list | *(see below)* | Ordered list of widgets to render. |

### `components`

Each entry is either an untagged string for a built-in component, or an inline
table of the form `{ logo = "..." }` for the logo glyph.

Built-in string names:

- `workspaces`
- `spacer`
- `battery`
- `wifi`
- `brightness`
- `volume`
- `clock`

Inline table:

- `{ logo = "\u{e0003}" }` — the logo glyph (a single character).

The `Component` enum is defined in
[`src/ui/components/mod.rs`](../src/ui/components/mod.rs).

### Full example

```toml
position = "left"
bar_size_px = 72
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
```

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
`workspace`, `withConfig`, and `components` parameters (see
[`flake.nix`](../flake.nix)). A consumer flake selects the compiled-in theme,
the compositor backend, whether the optional TOML/CSS config is built in, and
which feature-gated components ship — no forking required:

```nix
tkbar.packages.${system}.default.override {
  color = "purple";        # one of: black blue cyan green orange pink purple red white yellow
  workspace = "hyprland";  # one of: niri hyprland sway
  withConfig = false;      # set false to drop the optional TOML/CSS config
  components = [ "wifi" ]; # feature-gated components to build in; set [ ] to drop wifi
}
```

- `color` selects the compiled-in base theme (exactly one of the ten).
- `workspace` selects the compositor backend (`"niri"`, `"hyprland"`, or
  `"sway"`); exactly one is compiled in.
- `withConfig` toggles the optional `config` feature.
- `components` is the list of feature-gated components to build in. For now
  only `"wifi"` is supported (a `"brightness"` and an `"audio"` component will
  follow); set it to `[ ]` to ship a bar with no wifi widget. Each name maps
  1:1 to a Cargo feature, and unsupported names fail the build with a message.

The plain cargo equivalent is:

```sh
cargo build --no-default-features --features purple,hyprland,wifi
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

## Adding a theme

Ten themes are generated from a single SCSS source,
[`src/ui/styles/main.scss`](../src/ui/styles/main.scss), using
[`scripts/css.nu`](../scripts/css.nu) (which shells out to dart-sass). Run
`just css` to regenerate the compiled `src/ui/styles/<color>.css` files. To add
a color theme, add a palette entry to `scripts/css.nu`, regenerate the CSS, and
register the new `#[cfg]` feature in `src/lib.rs` (both the theme block and
`color_feature_count()`).