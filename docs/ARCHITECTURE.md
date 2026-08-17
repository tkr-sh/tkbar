# Architecture

A minimalist GTK4 layer-shell status bar for the `niri`, `Hyprland`, and `sway`
Wayland compositors. The whole bar is a single window that is a GTK
`ApplicationWindow` registered as a layer-shell surface.

## Crate layout

The repository is a Cargo workspace with two crates: `tkbar` at the root (a
bin + lib package) and the `tkbar-ci` pipeline in [`ci/`](../ci). The clippy
lints live once in the root manifest as `[workspace.lints.clippy]`.

The `tkbar` crate:

```
src/
├── main.rs        entry point; builds the GTK Application and wires callbacks
├── lib.rs         CSS/themes (THEME_CSS), re-exports build_window
├── conf.rs        global CONFIG: LazyLock<Config>
├── log.rs         eprintln!-only logging
├── ui/
│   ├── mod.rs     window + layer-shell setup, BarPosition
│   └── components/
│       ├── mod.rs         Component enum, add_to_bar, spawn_poller, spacer, clock
│       ├── battery.rs     battery percent/charging from sysfs
│       ├── brightness.rs  backlight percent from sysfs + brightnessctl
│       ├── volume.rs      wireplumber volume/mute via wpctl
│       ├── wifi.rs        SSID/signal via iwctl
│       └── workspaces/    per-compositor workspace strip (feature-gated backend)
│           ├── hypr.rs    Hyprland IPC (hyprland feature)
│           ├── niri.rs    niri IPC (niri feature)
│           └── sway.rs    sway/i3 IPC via swayipc (sway feature)
```

- **`main.rs`** — `Application::builder().application_id(APP_ID)`; on startup
  calls `load_css()`, on activate calls `build_window(app)`.
- **`lib.rs`** — declares the modules, defines `THEME_CSS` (selected by the
  active color feature), loads the optional user CSS, and re-exports
  `build_window`. `APP_ID = "dev.tk.tkbar"`.
- **`conf.rs`** — the global `CONFIG: std::sync::LazyLock<Config>` (feature
  `config`), the `Config` struct and the hardcoded defaults.
- **`log.rs`** — `warn(component, message)` and `error(component, message)`,
  both `eprintln!`. No logging framework.
- **`ui/mod.rs`** — `build_window` builds the `ApplicationWindow`, calls
  `init_layer_shell()`, sets the layer, anchors, and sole exclusive zone, then
  assembles the `bar` `GtkBox` from `CONFIG`. `BarPosition` maps to an
  `Orientation` and a set of `Edge` anchors.
- **`ui/components/*`** — individual widgets; each is a pair of a `GtkBox`
  (icon + value labels) and the code that feeds it.

## The two background patterns

Heavy work — reading sysfs, spawning processes, or holding a long-lived IPC
socket — is kept off the GTK main loop. Both patterns ship state over an
`async_channel` and update widgets on the main loop via `glib::spawn_future_local`.

### 1. Poller pattern (`spawn_poller`)

`spawn_poller` in [`src/ui/components/mod.rs`](../src/ui/components/mod.rs)
spawns a worker thread that:

1. polls a closure (`poll()`) on an interval,
2. diffs the returned state against the previous one,
3. sends **only on change** over the returned `async_channel`.

The GTK side uses `glib::spawn_future_local` with `glib::clone!` and
`#[weak]`/`#[upgrade_or]` (or `#[upgrade_or_default]`) to update widgets while
the container is alive. Used by `battery`, `brightness`, `volume`, and `wifi`.

Poll intervals: `battery`/`brightness` 500 ms, `volume` 500 ms, `wifi` 5 s.

### 2. Event-driven pattern (`workspaces/`)

The `workspaces` backend opens a long-lived IPC socket to the compositor and
subscribes to workspace/window events, feeding a channel on a dedicated thread. The
GTK side rebuilds the workspace buttons on each update. Click handlers spawn a
thread per `focus_workspace` call so the main loop never blocks on IPC.

Exactly one backend is compiled in at build time (`niri`, `hyprland`, or
`sway`). Each exposes the same pair of functions — `event_loop(tx)` and
`focus_workspace(id)` — over the shared `Ws` model in `workspaces/mod.rs`:

- `niri.rs` sends an `EventStream` request and reads `WorkspacesChanged`/
  `WorkspaceActivated`/`WorkspaceActiveWindowChanged` events.
- `hypr.rs` uses the `hyprland` event listener and reports the workspace window
  count directly from the IPC data.
- `sway.rs` uses `swayipc`: subscribing consumes the connection, so each
  snapshot opens a fresh one; since `get_workspaces` exposes no window count, a
  `get_tree()` walk counts leaf containers per workspace to derive `is_active`.

Both patterns guarantee the GTK main loop only ever does cheap widget updates.

## Config flow

```
main  →  CONFIG (LazyLock<Config>, conf.rs)  →  widgets
```

Widgets read the global `CONFIG` directly, e.g.
`CONFIG.style.position.orientation()` selects vertical/horizontal layout,
`CONFIG.style.position.anchors()` sets the layer-shell edges, and `CONFIG.components`
drives widget construction in `build_inner_window`.

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
   adversarial inputs (see `wifi.rs`/`volume.rs`).
6. **Feature-gate if needed** — new behavior should be opt-in behind a
   `[features]` entry in `Cargo.toml` rather than a new default dependency.
7. Run `just check` and confirm CI (`just dagger`) passes.

If the component polls or shells out, prefer the `spawn_poller` helper; if it
subscribes to compositor IPC, model it on `workspaces/`.