# Architecture

A minimalist GTK4 layer-shell status bar for the `niri` Wayland compositor. The
whole bar is a single window that is a GTK `ApplicationWindow` registered as a
layer-shell surface.

## Crate layout

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
│       └── workspaces.rs  niri IPC workspace strip
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

### 2. Event-driven pattern (`workspaces.rs`)

`workspaces` opens a long-lived niri IPC `Socket`, sends an `EventStream`
request, and reads events (`WorkspacesChanged`, `WorkspaceActivated`,
`WorkspaceActiveWindowChanged`) on a dedicated thread, feeding a channel. The
GTK side rebuilds the workspace buttons on each update. Click handlers spawn a
thread per `focus_workspace` call so the main loop never blocks on IPC.

Both patterns guarantee the GTK main loop only ever does cheap widget updates.

## Config flow

```
main  →  CONFIG (LazyLock<Config>, conf.rs)  →  widgets
```

Widgets read the global `CONFIG` directly, e.g.
`CONFIG.position.orientation()` selects vertical/horizontal layout,
`CONFIG.position.anchors()` sets the layer-shell edges, and `CONFIG.components`
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
subscribes to niri events, model it on `workspaces.rs`.