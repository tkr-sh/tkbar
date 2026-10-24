# Architecture

A minimalist GTK4 layer-shell status bar for the `niri`, `Hyprland`, and `sway`
Wayland compositors. The whole bar is a single window that is a GTK `Window`
registered as a layer-shell surface.

## Crate layout

The repository is a Cargo workspace with two crates: `tkbar` at the root (a
bin + lib package) and the `tkbar-ci` pipeline in [`ci/`](../ci). The clippy
lints live once in the root manifest as `[workspace.lints.clippy]`. The flake
([`flake.nix`](../flake.nix)) provides the package, the devshell, a NixOS
module (`programs.tkbar`) that installs the bar and an optional backlight udev
rule, and a Home Manager module that manages the per-user TOML/CSS
configuration and an optional systemd user service.

The `tkbar` crate:

```
src/
├── main.rs        entry point; parses the CLI, calls the selected renderer
├── lib.rs         renderer dispatch (run) + compile-time renderer checks
├── conf.rs        global CONFIG: LazyLock<Config>
├── log.rs         eprintln!-only logging
├── ui/
│   ├── mod.rs     BarPosition (renderer-agnostic)
│   ├── gtk/       GTK4 renderer: run(), layer-shell window, CSS, GLib loop
│   ├── wl/        Wayland renderer (stub until it lands)
│   └── components/
│       ├── mod.rs         Component (model) + GtkView (GTK) traits, spawn_poller
│       ├── battery.rs     battery percent/charging from sysfs
│       ├── brightness.rs  backlight percent read/written directly via sysfs
│       ├── volume.rs      PipeWire client for volume/mute
│       ├── wifi.rs        SSID/signal via nl80211 (neli-wifi)
│       └── workspaces/    per-compositor workspace strip (feature-gated backend)
│           ├── hypr.rs    Hyprland IPC (hyprland feature)
│           ├── niri.rs    niri IPC (niri feature)
│           └── sway.rs    sway/i3 IPC via swayipc (sway feature)
```

## Renderers

The drawing backend is chosen at build time by two mutually exclusive Cargo
features: `renderer-gtk` (default) links `gtk4`/`gtk4-layer-shell` and lives in
[`src/ui/gtk/`](../src/ui/gtk), while `renderer-wayland` is a stub in
[`src/ui/wl/`](../src/ui/wl) that exits with a diagnostic until the native
layer-shell backend lands. [`lib.rs`](../src/lib.rs) `run()` dispatches to
whichever is compiled in, and a `compile_error!` rejects builds with both or
neither. Everything the renderers share is GTK-free: each component splits into
a `Component` model (`init`/`threads`, owning its state and worker threads) and a
renderer-specific view trait — today only `GtkView` (`gtk_box`/`gtk_threads`)
behind `renderer-gtk` — so the model half compiles for either backend. Styling
stays renderer-owned: the GTK renderer keeps its feature-selected CSS
(`THEME_CSS` plus the optional `~/.config/tkbar/style.css`), and the Wayland
renderer will bring its own `theme` when it lands.

- **`main.rs`** — parses the CLI via `clap` (feature `cli`) and calls
  `tkbar::run()`.
- **`lib.rs`** — defines `run()`, which dispatches to the compiled-in renderer,
  and emits a `compile_error!` if both (or neither) renderer feature is set.
- **`conf.rs`** — the global `CONFIG: std::sync::LazyLock<Config>` (feature
  `config`), the `Config` struct and the hardcoded defaults.
- **`log.rs`** — `warn(component, message)` and `error(component, message)`,
  both `eprintln!`. No logging framework.
- **`ui/components/*`** — the renderer-agnostic model, plus its GTK view; most
  are a `GtkBox` holding an icon and a value label, paired with the code that
  feeds it.

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

At most one compositor backend is compiled in at build time (`niri`, `hyprland`, or
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
`CONFIG.style.outputs` selects which monitors get a bar: an empty list builds one
window per monitor, while each name builds on its matching connector (or
model/description fallback).

## Theme system

```
Cargo feature (exactly one)
        │  #[cfg(feature = "...")]
        ▼
THEME_CSS: &str  (ui/gtk)  ──include_str!──▶  src/ui/styles/<color>.css
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

1. **Add a variant** to `ComponentKind` in
   [`src/ui/components/mod.rs`](../src/ui/components/mod.rs) and a matching
   `add_to_bar` arm.
2. **Add a module** (e.g. `src/ui/components/<name>.rs`) with its `Component`
   model (`init`/`threads`) and, behind `renderer-gtk`, its `GtkView`
   (`gtk_box`/`gtk_threads`); declare it (`mod <name>;`) and re-export it in
   `components/mod.rs`.
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