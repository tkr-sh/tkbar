set shell := ["nu", "-c"]

# Enter in a nix-shell to dev
[group: 'dev']
env:
    nix develop -c nu

# Watch
[group: 'dev']
w:
    bacon run-long

[group: 'dev']
nix-w:
    nix develop -c "bacon"

# Run
[group: 'dev']
run: css
    cargo run

[group: 'dev']
nix-run:
    nix develop -c "cargo" "run"

# Build stylesheets from SCSS
[group: 'check']
css:
    nu ./scripts/css.nu

# --- CI checks (non-destructive, --locked) ---

[group: 'check']
fmt-check:
    cargo fmt --check
    taplo fmt --check

[group: 'check']
cargo-check:
    cargo check --locked

[group: 'check']
clippy:
    cargo clippy --locked

[group: 'check']
test:
    cargo nextest run

# Run all CI checks
[group: 'check']
ci: css fmt-check cargo-check clippy test

# Check + auto-format (local convenience)
[group: 'check']
check:
    cargo check
    cargo fmt
    try { cargo nextest run }
    cargo clippy
    taplo fmt

[group: 'check']
fix:
    cargo clippy --fix --allow-staged
    cargo fmt
    taplo fmt

# Run the Dagger CI pipeline locally (needs docker or podman)
[group: 'check']
dagger:
    cargo build --release --manifest-path ci/Cargo.toml
    dagger run ci/target/release/tkbar-ci
