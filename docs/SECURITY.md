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
   netlink socket. This is the only remotely-influenced input.
2. **Daemon responses** — `wpctl` output; local, but produced by daemons that
   talk to every local client.
3. **The configuration file** — writable by anything with access to your home
   directory.

The bar opens no *network* sockets, speaks no network protocol, and never opens
files based on data it received. The only sockets it holds are the Unix IPC
socket to the compositor (niri/Hyprland/sway) — the same trusted path the
wayland connection itself uses — and a `NETLINK_GENERIC` socket to the kernel
for Wi-Fi state, which cannot reach a remote peer.

## Enforced security properties

These are structural, compile-time or design constraints, not runtime checks:

- **Safe Rust, no `unsafe`.** The crate contains no `unsafe` code.
- **No panics on externally-influenced data.** The Wi-Fi SSID arrives as raw,
  attacker-controlled bytes from a nearby access point via nl80211; it is
  decoded lossily and stripped of control characters, then only ever displayed
  as a tooltip. `wpctl` and sysfs inputs are parsed with fallible
  `parse().ok()?` chains that fail closed — malformed input makes a widget keep
  its previous state, never crash the bar. `panic = "abort"` is set in the
  release profile.
- **No network code.** No HTTP, no DNS, no listening sockets.
- **No dynamic loading.** No plugins, no `dlopen`, no scripting engine, no
  webview. GTK's image-loading machinery is neutered: at startup the bar points
  `GDK_PIXBUF_MODULE_FILE` at an empty loader cache, so gdk-pixbuf loads no
  image parser modules (SVG, TIFF, ...), and the Nix package wires the same
  empty cache into the wrapper. The base CSS is compiled into the binary
  (`include_str!`); the only runtime-loaded content is the optional
  `~/.config/tkbar/style.css` (behind the `config` feature), which is plain
  CSS data and cannot execute code.
- **Control-character stripping of the SSID.** SSIDs are arbitrary
  attacker-controlled bytes; they are stripped of control characters (including
  ANSI escape sequences) before being *displayed* as a tooltip, never
  interpreted.
- **Strict typed config.** TOML has no code execution, aliases, or external
  includes; parsing is strict and typed (`deny_unknown_fields`). An attacker
  who can write your config can change the layout, and nothing more.
- **Pinned, reproducible supply chain.** `Cargo.lock` and `flake.lock` pin
  every dependency; the Nix build is reproducible. `cargo tree` shows the whole
  picture.
- **Stable compiler for artifacts.** Release builds use the stable Rust
  toolchain; nightly is confined to the devshell for `rustfmt`/`clippy`, so
  shipped binaries are produced by the well-tested stable compiler, not a
  moving nightly.
- **No privileges.** Runs as your user, no capabilities, no secrets, no setuid
  helpers. Brightness is written directly to sysfs, gated by kernel/udev
  permission checks; the flake's NixOS module installs a udev rule granting
  write access to a dedicated group (`tkbar-backlight` by default, or `video`).

## Known attack surface

The README details this; the short version:

- The **GTK stack** (GTK4, Pango, FreeType, Cairo) is by far the largest attack
  surface. Mitigation: fonts come from already-trusted system fontconfig, and
  the bar loads no images or remote content — gdk-pixbuf's image loader modules
  are disabled at startup (`GDK_PIXBUF_MODULE_FILE` → empty cache), so no image
  parser code runs. Keep your system GTK updated.
- **`PATH` hijacking** of the spawned `wpctl` would give code execution.
  Mitigated in the Nix package (pinned wrapper `PATH`); elsewhere, keep your
  `PATH` sane. The Wi-Fi and brightness paths don't spawn any process (Wi-Fi
  talks to the kernel via netlink, brightness writes sysfs directly), so they
  have no `PATH` hijack surface.
- A **malicious Wi-Fi SSID** traveling through a memory-safe parser has a worst
  realistic outcome of a misleading tooltip — UI confusion, not corruption.
- A **compromised compositor** can overlay, spoof, or intercept anything; this
  is true for every Wayland client and unfixable at this layer.

See the README's [Security](../README.md#security) section for the complete
detail.
