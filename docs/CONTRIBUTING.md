# Contributing

Thanks for considering a contribution. This project is small on purpose, and
changes are expected to respect that: minimal dependencies, a hardened
security posture, and strict feature gating. Everything here runs with your
full user privileges, so bloat is treated as a security risk, not a
convenience.

## Design philosophy

- **Minimal dependencies.** Every crate and library is code that runs with
  your full user privileges. There is no logging framework (just `eprintln!`
  in [`src/log.rs`](../src/log.rs)), and optional parsing dependencies are
  feature-gated so they can be dropped entirely. Do not add a runtime
  dependency without a strong justification.
- **Hardened and security-first.** The bar opens no sockets, speaks no network
  protocol, does no dynamic loading, and runs with no special privileges.
  Parsing of externally-influenced data must be fallible and must fail closed
  (keep the previous state), never panic.
- **Everything is feature-gated.** New behavior is opt-in via Cargo features,
  not a new default dependency.

See the README's [Security](../README.md#security) section and
[`docs/SECURITY.md`](./SECURITY.md) for the full threat model.

## Feature gating

There are several categories of features in [`Cargo.toml`](../Cargo.toml):

- Exactly one renderer: `renderer-gtk` (the default GTK4 backend, linking
  `gtk4`/`gtk4-layer-shell`) or `renderer-wayland` (a stub until the native
  backend lands). A `compile_error!` rejects builds with both or neither.
- `cli` — the optional clap-based CLI (`--help`, `--version`, `--config`).
- `config` — the optional TOML/CSS configuration. It pulls in `directories`,
  `serde`, and `toml` (all `optional = true`). Only `serde` is already present
  transitively through the default `niri` backend (`niri-ipc`); `directories`
  and `toml` are genuinely new.
- Exactly one of ten color features: `black`, `blue`, `cyan`, `green`,
  `orange`, `pink`, `purple`, `red`, `white`, `yellow`. This set is the
  compiled-in base stylesheet.
- The workspace backend: at most one of `niri`, `hyprland`, `sway`. The
  workspace strip code lives in `src/ui/components/workspaces/`, one module
  per backend exposing `event_loop`/`focus_workspace` over the shared `Ws`
  model.
- The feature-gated components: `battery`, `brightness`, `clock`, `volume`
  (`pipewire`), and `wifi` (`neli-wifi`). The `all_components` bundle enables
  all five at once. The `doc` feature is used only to regenerate the default
  configuration.

The default is `all_components` + `black` + `cli` + `config` + `niri` +
`renderer-gtk`
(`default = ["all_components", "black", "cli", "config", "niri", "renderer-gtk"]`).

**Exactly one color must be enabled** (for the GTK renderer). This is enforced
by a `const` assert in [`src/lib.rs`](../src/lib.rs):

```rust
const _: () = assert!(
    color_feature_count() == 1,
    "exactly one color feature must be enabled: ..."
);
```

Color themes are selected with `#[cfg(feature = "...")]` and embedded with
`include_str!` (see `THEME_CSS` in `src/ui/gtk/mod.rs`). Because this is
compile-time selection rather than runtime `cfg!`, only the selected stylesheet
is linked into the binary — the other nine are not. Styling is renderer-owned:
the Wayland renderer will get its own `theme` when it lands.

### Building with feature combinations

```sh
# the default: black + cli + config + niri + volume + wifi
cargo build

# GTK renderer + purple theme + config + cli
cargo build --no-default-features --features renderer-gtk,purple,config,cli

# GTK renderer + black theme, no config, no cli (no TOML parser linked at all)
cargo build --no-default-features --features renderer-gtk,black
```

Use `--no-default-features` whenever you pick a color, otherwise the default
`black` feature also applies and the const assert fails on two colors.

### Adding a new feature

1. Declare it under `[features]` in `Cargo.toml`, wiring any `optional = true`
   dependencies via `dep:`.
2. Gate the code with `#[cfg(feature = "...")]`.
3. Keep the color theme const assert intact — a new color must be added to both
   the `#[cfg]` theme block and `color_feature_count()` in `src/lib.rs`.

## Testing

- Unit tests live inline as `#[cfg(test)]` modules next to the code. See
  [`src/ui/components/wifi.rs`](../src/ui/components/wifi.rs) for the
  adversarial-parser test style.
- New parsing or parsing-adjacent code **must** be tested, including
  adversarial inputs: ANSI injection, overflow, and garbage that must never
  panic.
- `allow-unwrap-in-tests` is set in [`clippy.toml`](../clippy.toml), so
  `unwrap` in `#[cfg(test)]` modules is fine.
- Run them with `cargo nextest run` or `just ci-test`.

## Style and lint standards

- `cargo fmt` — nightly rustfmt options in [`rustfmt.toml`](../rustfmt.toml)
  (e.g. `imports_granularity = "One"`, `group_imports = "StdExternalCrate"`).
  Formatting is not pinned to stable; the flake devshell provides the nightly
  toolchain.
- `cargo clippy` — the strict `[lints.clippy]` table in
  [`Cargo.toml`](../Cargo.toml) denies `as_conversions` and `pub_without_shorthand`
  and warns on `unwrap_used` outside tests. Keep it clean.
- `taplo fmt` — TOML formatting, configured in [`taplo.toml`](../taplo.toml).
- `#[allow(...)]` must always carry a `reason`.

Local convenience recipes:

```sh
just check   # check + fmt + test + clippy + taplo
just fix     # clippy --fix + fmt + taplo
```

## Hard rules

- **No `unsafe` except one documented call.** Safe Rust only; the single allowed
  exception is the `glib::setenv` call in `harden_gtk_env` (`src/ui/gtk/mod.rs`),
  needed to seed the GTK-hardening env vars before GTK initializes. Any new
  `unsafe` requires a strong justification and an `#[expect(unsafe_code, reason = ...)]`.
- **No `unwrap`/`expect`/panic on externally-influenced data.** That means
  Wi-Fi SSIDs, PipeWire node parameters, and sysfs. The Wi-Fi SSID arrives
  as raw attacker-controlled bytes from nl80211 and is only ever decoded and
  displayed, never parsed; PipeWire and sysfs inputs stay on fallible
  `parse().ok()?` chains that fail closed (keep the last state); a malformed
  input must never crash the bar.
- **No new runtime dependency without justification.** Minimal-dependency ethos.
- **No network code.** No HTTP, no DNS, no listening sockets.

## CI

The CI pipeline is a Dagger pipeline in the [`ci/`](../ci) crate (see
[`ci/src/main.rs`](../ci/src/main.rs)). It runs each recipe inside a Nix
container via `nix develop`:

- `ci-css` — verify the committed stylesheets match the SCSS sources.
- `ci-fmt` — `cargo fmt --check` + `taplo fmt --check`.
- `ci-machete` — `cargo machete` (no unused dependencies).
- `ci-deny` — `cargo deny --exclude tkbar-ci check advisories bans licenses sources` (supply chain, shipped crate only).
- `ci-check` — `cargo check --locked`.
- `ci-clippy` — `cargo clippy --locked`.
- `ci-test` — `cargo nextest run`.
- `ci-clippy-features` — `cargo clippy` over several `--no-default-features`
  feature combinations.

These map one-to-one onto the `ci-*` recipes in the
[`justfile`](../justfile), and the `STEPS` list in `ci/src/push.rs` must stay in
sync with them. The GitHub workflow `.github/workflows/ci.yml` builds the
pipeline binary and runs it with Dagger. You can run the whole pipeline locally
with `just dagger` (needs docker or podman).

## PR checklist

- [ ] Feature-gated: optional behavior is behind a `[features]` entry, and the
      build works with `--no-default-features`.
- [ ] No new `unsafe` (the documented `glib::setenv` call is the only exception), no panicking on externally-influenced data.
- [ ] No new runtime dependency without justification.
- [ ] Parser changes are covered by adversarial tests (and pass `cargo nextest run`).
- [ ] `cargo fmt`, `cargo clippy`, and `taplo fmt` are clean; `#[allow(...)]` has a `reason`.
- [ ] CI (`just dagger` or the GitHub workflow) passes.