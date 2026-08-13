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

# --- CI checks (non-destructive, --locked) ---

# Run the Dagger CI pipeline locally (needs docker or podman)
[group: 'ci']
dagger:
    cargo build --release -p tkbar-ci
    dagger run target/release/tkbar-ci

[group: 'ci']
release version:
    nu ./scripts/release.nu {{version}}

[group: 'ci']
ci-fmt:
    cargo fmt --check
    taplo fmt --check

[group: 'ci']
ci-check:
    cargo check --locked --workspace

[group: 'ci']
ci-check-features:
    cargo check --locked --target tkbar --no-default-features --features config,hyprland
    cargo check --locked --target tkbar --no-default-features
    cargo check --locked --target tkbar --no-default-features --features red

[group: 'ci']
ci-clippy:
    cargo clippy --locked --workspace -- -Dwarnings

[group: 'ci']
ci-test:
    cargo nextest run --workspace

