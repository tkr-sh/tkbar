# Architecture

A minimalist GTK4 layer-shell status bar for the `niri`, `Hyprland`, and `sway`
Wayland compositors. The whole bar is a single window that is a GTK `Window`
registered as a layer-shell surface.

## Crate layout

The repository is a Cargo workspace with two crates: `tkbar` at the root (a
bin + lib package) and the `tkbar-ci` pipeline in [`ci/`](../ci). The clippy
lints live once in the root manifest as `[workspace.lints.clippy]`. The flake
([`flake.nix`](../flake.nix)) provides the package, the devshell, and a NixOS
module (`programs.tkbar`) that installs the bar and an optional backlight udev
rule.

The `tkbar` crate:

```
src/
├── main.rs        entry point; parses the CLI, inits GTK, loads CSS, runs the bar
├── lib.rs         CSS/themes (THEME_CSS), init_gtk, re-exports build_window
├── conf.rs        global CONFIG: LazyLock<Config>
├── log.rs         eprintln!-only logging
├── ui/
│   ├── mod.rs     window + layer-shell setup, BarPosition
│   └── components/
│       ├── mod.rs         Component enum, add_to_bar, spawn_poller, spacer, clock
│       ├── battery.rs     battery percent/charging from sysfs
│       ├── brightness.rs  backlight percent read/written directly via sysfs
│       ├── volume.rs      PipeWire client for volume/mute
│       ├── wifi.rs        SSID/signal via nl80211 (neli-wifi)
│       └── workspaces/    per-compositor workspace strip (feature-gated backend)
│           ├── hypr.rs    Hyprland IPC (hyprland feature)
│           ├── niri.rs    niri IPC (niri feature)
│           └── sway.rs    sway/i3 IPC via swayipc (sway feature)
```

- **`main.rs`** — parses the CLI via `clap` (feature `cli`), disables the
  gdk-pixbuf image loaders, calls `init_gtk()`, `load_css()`, and
  `build_window()`, then runs a `glib::MainLoop`.
- **`lib.rs`** — declares the modules, provides `init_gtk()` (GTK
  initialization with an actionable error if there is no display), defines
  `THEME_CSS` (selected by the active color feature), loads the optional user
  CSS, and re-exports `build_window`.
- **`conf.rs`** — the global `CONFIG: std::sync::LazyLock<Config>` (feature
  `config`), the `Config` struct and the hardcoded defaults.
- **`log.rs`** — `warn(component, message)` and `error(component, message)`,
  both `eprintln!`. No logging framework.
- **`ui/mod.rs`** — `build_window` builds the `Window`, calls
  `init_layer_shell()`, sets the layer, anchors, and sole exclusive zone, then
  assembles the `bar` `GtkBox` from `CONFIG.behaviour.components`. `BarPosition`
  maps to an `Orientation` and a set of `Edge` anchors.
- **`ui/components/*`** — individual widgets; most are a `GtkBox` holding an
  icon and a value label, paired with the code that feeds it.

## The three background patterns

Heavy work — reading sysfs, talking to a daemon, or holding a long-lived IPC
socket — is kept off the GTK main loop. All patterns ship state over an
`async_channel` and update widgets on the main loop via `glib::spawn_future_local`.

### 1. Poller pattern (`spawn_poller`)

`spawn_poller` in [`src/ui/components/mod.rs`](../src/ui/components/mod.rs)
spawns a worker thread that:

1. polls a closure (`poll()`) on an interval,
2. diffs the returned state against the previous one,
3. sends **only on change** over the returned `async_channel`.

The GTK side uses `glib::spawn_future_local` with `glib::clone!` and
`#[weak]`/`#[upgrade_or]` (or `#[upgrade_or_default]`) to update widgets while
the container is alive. Used by `battery`, `brightness`, and `wifi`.

Poll intervals: `battery` 10 s for percent and 500 ms for the charging flag,
`brightness` 500 ms, `wifi` 5 s.

### 2. Event-driven pattern (`workspaces/`)

The `workspaces` backend opens a long-lived IPC socket to the compositor and
subscribes to workspace/window events, feeding a channel on a dedicated thread. The
GTK side rebuilds the workspace buttons on each update. Click handlers spawn a
thread per `focus_workspace` call so the main loop never blocks on IPC.

At most one backend is compiled in at build time (`niri`, `hyprland`, or
`sway`; the default build enables `niri`). Each exposes the same pair of
functions — `event_loop(tx)` and
`focus_workspace(id)` — over the shared `Ws` model in `workspaces/mod.rs`:

- `niri.rs` sends an `EventStream` request and reads `WorkspacesChanged`/
  `WorkspaceActivated`/`WorkspaceActiveWindowChanged` events.
- `hypr.rs` uses the `hyprland` event listener and reports the workspace window
  count directly from the IPC data.
- `sway.rs` uses `swayipc`: subscribing consumes the connection, so each
  snapshot opens a fresh one; since `get_workspaces` exposes no window count, a
  `get_tree()` walk counts leaf containers per workspace to derive `is_active`.

### 3. Native client pattern (`volume`)

The `volume` component runs a dedicated thread with its own PipeWire main loop.
It binds the default sink node and the `default` metadata object, receives
volume/mute parameter updates as they change, and pushes the resulting state over
an `async_channel`. Scroll and click handlers send `Cmd` values back over a
`pipewire::channel`; the loop applies them by writing SPA pods to the sink node
(or to its route on the owning device when the sink is part of a card) and
reports the new state. No external process is spawned.

All patterns guarantee the GTK main loop only ever does cheap widget updates.

## Config flow

```
main (CLI --config)  →  CLI_CONFIG_PATH (OnceLock<Box<Path>>)  →  CONFIG (LazyLock<Config>, conf.rs)  →  widgets
```

Widgets read the global `CONFIG` directly, e.g.
`CONFIG.style.position.orientation()` selects vertical/horizontal layout,
`CONFIG.style.position.anchors()` sets the layer-shell edges, and
`CONFIG.behaviour.components` drives widget construction in `build_inner_window`.

## Theme system

```
Cargo feature (exactly one)
        │  #[cfg(feature = "...")]
        ▼
THEME_CSS: &str  (lib.rs)  ──include_str!──▶  src/ui/styles/<color>.css
        │
        ▼
load_css()  →  CssProvider → style_context_add_provider_for_display
```

Themes are authored in SCSS (`src/ui/styles/main.scss`), compiled to CSS by
dart-sass via `scripts/css.nu` (`just css`), and the resulting CSS is committed
and embedded with `include_str!`. Only the selected color's CSS is linked. With
the `config` feature, a user `~/.config/tkbar/style.css` is additionally loaded
at `STYLE_PROVIDER_PRIORITY_USER`.

## Adding a new component

1. **Add a variant** to `Component` in
   [`src/ui/components/mod.rs`](../src/ui/components/mod.rs) and a matching
   `add_to_bar` arm.
2. **Add a module** (e.g. `src/ui/components/<name>.rs`) and declare it
   (`mod <name>;`) and re-export its constructor in `components/mod.rs`.
3. **Wire it into the UI.** If it is config-driven, make sure the new variant
   deserializes (the enum derives `serde::Deserialize` behind `config`).
4. **Register it in the default** — add it to `default_components()` in
   `src/conf.rs` if it should be on by default.
5. **Write parser tests** in an inline `#[cfg(test)]` module, including
   adversarial inputs (see `wifi.rs`).
6. **Feature-gate if needed** — new behavior should be opt-in behind a
   `[features]` entry in `Cargo.toml` rather than a new default dependency.
7. Run `just check` and confirm CI (`just dagger`) passes.

If the component polls or shells out, prefer the `spawn_poller` helper; if it
subscribes to compositor IPC, model it on `workspaces/`.