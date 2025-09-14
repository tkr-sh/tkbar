set shell := ["nu", "-c"]

# Enter in a nix-shell to dev
[group: 'dev']
env:
    nix develop -c nu

# Watch
[group: 'dev']
w:
    bacon

[group: 'dev']
nix-w:
    nix develop -c "bacon"

# Run
[group: 'dev']
run:
    cargo run

[group: 'dev']
nix-run:
    nix develop -c "cargo" "run"

# Check
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
