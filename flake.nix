{
    description = "Project flake";

    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
        systems.url = "github:nix-systems/default";
        rust-overlay.url = "github:oxalica/rust-overlay";
        flake-utils.url  = "github:numtide/flake-utils";
    };

    outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
        flake-utils.lib.eachDefaultSystem (system:
        let
            overlays = [ (import rust-overlay) ];
            pkgs = import nixpkgs {
                inherit system overlays;
            };
        in
        {
            devShells.default = with pkgs; mkShell {
                buildInputs = [
                    rust-bin.nightly.latest.default
                    just
                    nushell
                    taplo
                    bacon
                    openssl
                    cargo-nextest
                    cargo-machete

                    # Nice utilities
                    fd
                    ripgrep
                    nushell
                ];

                nativeBuildInputs = with pkgs; [ pkg-config ];
            };
        }
    );
}

