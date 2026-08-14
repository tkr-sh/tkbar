pub mod push;
pub mod release;

use dagger_sdk::{
    Container,
    ContainerWithMountedCacheOpts,
    DaggerConn,
    Directory,
    HostDirectoryOpts,
};

const NIX_IMAGE: &str = "nixos/nix:latest";
const NIX_CONF: &str = "experimental-features = nix-command flakes\nsandbox = false";

fn nix_develop<'a>(cmd: &'a [&'a str]) -> Vec<&'a str> {
    let mut args: Vec<&str> = vec!["nix", "develop"];

    if !cmd.is_empty() {
        args.push("--command");
    }

    args.extend(cmd);
    args
}

pub(crate) fn src(dag: &DaggerConn) -> Directory {
    dag.host().directory_opts(
        ".",
        HostDirectoryOpts {
            exclude: Some(vec!["target/", "result", ".git/", ".sass-cache/"]),
            gitignore: Some(true),
            include: None,
            no_cache: None,
        },
    )
}

pub(crate) async fn env(dag: &DaggerConn, src: Directory) -> eyre::Result<Container> {
    // An empty cache mount over /nix would shadow nix and break it, so seed
    // the cache volume with the image's own /nix on first creation. After
    // that the devShell closure built by `nix develop` persists across runs.
    let nix_seed = dag.container().from(NIX_IMAGE).directory("/nix");

    Ok(dag
        .container()
        .from(NIX_IMAGE)
        .with_env_variable("NIX_CONFIG", NIX_CONF)
        .with_mounted_cache_opts(
            "/nix",
            dag.cache_volume("tkbar-nix"),
            ContainerWithMountedCacheOpts {
                source: Some(nix_seed.id().await?),
                sharing: None,
                owner: None,
                expand: None,
            },
        )
        .with_mounted_cache("/root/.cargo", dag.cache_volume("tkbar-cargo-home"))
        .with_directory("/src", src)
        .with_workdir("/src")
        .with_mounted_cache("/src/target", dag.cache_volume("tkbar-cargo-target")))
}
