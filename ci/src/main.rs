use {
    dagger_sdk::{HostDirectoryOpts, connect},
    std::process::ExitCode,
};

const NIX_IMAGE: &str = "nixos/nix:latest";

const NIX_CONF: &str = "experimental-features = nix-command flakes\nsandbox = false";

// Each entry becomes its own Dagger step, so a failure is attributed to the
// exact check in the trace and CI logs. Kept in sync with the `ci` recipe in
// the justfile.
const STEPS: &[&str] = &[
    "css",
    "fmt-check",
    "cargo-check",
    "clippy",
    "test",
];

fn nix_develop(cmd: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = vec!["nix".into(), "develop".into(), "--command".into()];
    args.extend(cmd.iter().map(|s| (*s).to_string()));
    args
}

async fn run() -> eyre::Result<()> {
    connect(|dag| {
        async move {
            let src = dag.host().directory_opts(
                ".",
                HostDirectoryOpts {
                    exclude: Some(vec![
                        "target/",
                        "result",
                        ".git/",
                        ".sass-cache/",
                        "ci/",
                    ]),
                    gitignore: Some(true),
                    include: None,
                    no_cache: None,
                },
            );

            let mut env = dag
                .container()
                .from(NIX_IMAGE)
                .with_env_variable("NIX_CONFIG", NIX_CONF)
                .with_mounted_cache("/root/.cache/nix", dag.cache_volume("tkbar-nix-cache"))
                .with_mounted_cache("/root/.cargo", dag.cache_volume("tkbar-cargo-home"))
                .with_directory("/src", src)
                .with_workdir("/src")
                .with_mounted_cache("/src/target", dag.cache_volume("tkbar-cargo-target"));

            for recipe in STEPS {
                env = env.with_exec(nix_develop(&["just", recipe])).sync().await?;
            }

            Ok(())
        }
    })
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ci failed: {e:#}");
            ExitCode::FAILURE
        },
    }
}
