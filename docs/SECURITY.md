# Security

tkbar runs unsandboxed with your full user privileges and no permission
boundary: it can read files, spawn processes, and never stops running. Anything
it mishandles executes as *you*. This document summarizes the threat model and
the properties the code enforces.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue.
Preferred channels:

- A GitHub **private security advisory** on the repository (Security →
  "Report a vulnerability").
- A private email to the maintainer. If you cannot find a dedicated security
  address, use the repository's contact details or a private maintainer email.

Include as much as you safely can: a description, affected versions/commit,
reproduction steps, and your assessment of impact. Coordinated disclosure is
appreciated — allow a reasonable window before public discussion.

## Threat model

**Trusted:** the Linux kernel, your user account, the Wayland compositor
(niri, Hyprland, or sway), and everything installed on the system (libraries,
fonts, the tools in `PATH`). If any of those is compromised, no userspace
status bar can defend you. A malicious compositor can read your screen and
input for *any* client.

**Untrusted data that reaches the bar:**

1. **Wi-Fi SSIDs** — any nearby access point can broadcast an arbitrary SSID;
   it reaches the bar as raw, attacker-controlled bytes from the nl80211
   netlink socket. This is the only remotely-influenced input. Workspace names
   from the compositor are also treated as untrusted and are hidden unless
   `should_allow_workspace_label` is enabled.
2. **Daemon responses** — PipeWire node parameters (volume/mute); local, but
   produced by a daemon that talks to every local client.
3. **The configuration file** — writable by anything with access to your home
   directory.

The bar opens no *network* sockets, speaks no network protocol, and never opens
files based on data it received. The only sockets it holds are the Unix IPC
socket to the compositor (niri/Hyprland/sway) — the same trusted path the
wayland connection itself uses — a Unix socket to the local PipeWire daemon for
volume/mute, and a `NETLINK_GENERIC` socket to the kernel for Wi-Fi state, none
of which can reach a remote peer.

## Enforced security properties

These are structural, compile-time or design constraints, not runtime checks:

- **Safe Rust, one audited `unsafe` call.** The library is
  `#![forbid(unsafe_code)]`. The only `unsafe` in the project is a single
  `glib::setenv` call in `harden_gtk_env` (`src/main.rs`), which runs before GTK
  initializes and before any thread is spawned. `unsafe_code` is `deny` at the
  workspace level and the call is explicitly allowed with
  `#[expect(unsafe_code, reason = ...)]`; any additional `unsafe` needs a strong
  justification.
- **No panics on externally-influenced data.** The Wi-Fi SSID arrives as raw,
  attacker-controlled bytes from a nearby access point via nl80211; it is
  decoded lossily and sanitised — control characters and default-ignorable code
  points are replaced with the Unicode replacement character — then only ever
  displayed as a tooltip. PipeWire node parameters and sysfs inputs are parsed
  with fallible `parse().ok()?` chains that fail closed — malformed input makes
  a widget keep its previous state, never crash the bar. `panic = "abort"` is
  set in the release profile.
- **No network code.** No HTTP, no DNS, no listening sockets.
- **No dynamic loading.** No plugins, no `dlopen`, no scripting engine, no
  webview. GTK's image-loading machinery is neutered: at startup the bar points
  `GDK_PIXBUF_MODULE_FILE` at `/dev/null`, so gdk-pixbuf loads no image parser
  modules (SVG, TIFF, ...), and the Nix wrapper sets the same value as defense
  in depth. The base CSS is compiled into the binary
  (`include_str!`); the only runtime-loaded content is the optional
  `~/.config/tkbar/style.css` (behind the `config` feature), which is plain
  CSS data and cannot execute code.
- **SSID sanitisation.** SSIDs are arbitrary attacker-controlled bytes; before
  being *displayed* as a tooltip they are decoded lossily and every control
  character or default-ignorable code point (ANSI escape sequences, bidi
  overrides, zero-width characters, ...) is replaced with the Unicode
  replacement character. They are never interpreted.
- **Strict typed config.** TOML has no code execution, aliases, or external
  includes; parsing is strict and typed (`deny_unknown_fields`). An attacker
  who can write your config can change the layout, and nothing more.
- **Pinned, reproducible supply chain.** `Cargo.lock` and `flake.lock` pin
  every dependency; the Nix build is reproducible. `cargo tree` shows the whole
  picture. CI audits the full feature matrix with `cargo deny check advisories
  bans licenses sources` (no vulnerable, unsound, yanked, unlicensed or
  non-crates.io crate) and `cargo machete` (no unused dependency).
- **Stable compiler for artifacts.** Release builds use the stable Rust
  toolchain; nightly is confined to the devshell for `rustfmt`/`clippy`, so
  shipped binaries are produced by the well-tested stable compiler, not a
  moving nightly.
- **No privileges.** Runs as your user, no capabilities, no secrets, no setuid
  helpers. Brightness is written directly to sysfs, gated by kernel/udev
  permission checks; the flake's NixOS module installs a udev rule granting
  write access to a dedicated group (`tkbar-backlight` by default, or `video`).

## Measured footprint

"Hardened" is a property you can check, not a slogan. With the bar running:

```sh
pid=$(pidof tkbar)
ls -l /proc/$pid/fd | grep -c socket                 # open sockets
grep -c '\.so' /proc/$pid/maps                        # shared-object mappings
grep -E 'dri|vulkan|mesa|atspi' /proc/$pid/maps       # GPU / a11y code in the process
```

On a default-feature build (Intel GPU, niri, GTK 4.x):

- **4** open sockets — Wayland, compositor IPC, PipeWire, netlink.
- **0** D-Bus connections.
- **581** shared-object mappings.
- no GPU driver loaded (`*_dri.so`, `libEGL`, `libgbm`), and no
  `/dev/dri/renderD*` opened.
- nothing written under `$HOME` at runtime.

Every socket that remains maps to something visible on screen. The two
libraries you will still see are `libvulkan.so.1` and `libgio-2.0.so`: both are
link-time dependencies of `libgtk-4` itself, mapped by `ld.so` and never
initialized (no Vulkan ICD is loaded, no D-Bus connection is made).

Two things GTK still reads from your home directory, because GTK does so
unconditionally and they are in the same trust class as the bar's own config:
`~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-4.0/settings.ini`. Both are
plain data.

## Known attack surface

In short:

- The **GTK stack** (GTK4, Pango, FreeType, Cairo) is by far the largest attack
  surface. Mitigation: fonts come from already-trusted system fontconfig, and
  the bar loads no images or remote content — gdk-pixbuf's image loader modules
  are disabled at startup (`GDK_PIXBUF_MODULE_FILE` → `/dev/null`), so no image
  parser code runs. Keep your system GTK updated.
- The bar **spawns no external processes**, so there is no `PATH` hijack
  surface. Volume uses an in-process PipeWire client, Wi-Fi talks to the kernel
  via netlink, and brightness writes sysfs directly.
- A **malicious Wi-Fi SSID** traveling through a memory-safe parser has a worst
  realistic outcome of a misleading tooltip — UI confusion, not corruption.
- A **compromised compositor** can overlay, spoof, or intercept anything; this
  is true for every Wayland client and unfixable at this layer.

See the README's [Security](../README.md#security) section for a short summary.
