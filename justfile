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
fix: &&fmt
    cargo clippy --fix --allow-staged

[group: 'check']
fmt: &&doc
    cargo fmt
    taplo fmt ...(fd -e toml | lines | where { $in != 'docs/config.toml' })

[group: 'doc']
doc:
    cargo run --bin doc --features doc
    taplo fmt --config ./docs/taplo.toml ./docs/taplo.toml

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
    taplo fmt --check ...(fd -e toml | lines | where { $in != 'docs/config.toml' })
    taplo fmt --check --config ./docs/taplo.toml ./docs/config.toml

[group: 'ci']
ci-check:
    cargo check --locked --workspace

[group: 'ci']
ci-clippy-features:
    cargo clippy --locked --no-default-features --features config,hyprland,black -- -Dwarnings
    cargo clippy --locked --no-default-features --features red -- -Dwarnings
    cargo clippy --locked --no-default-features --features config,sway,black -- -Dwarnings
    cargo clippy --locked --no-default-features --features doc,purple --bin doc -- -Dwarnings

[group: 'ci']
ci-clippy:
    cargo clippy --locked --workspace -- -Dwarnings

[group: 'ci']
ci-test:
    cargo nextest run --workspace

