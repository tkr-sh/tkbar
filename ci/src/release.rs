use {
    crate::nix_develop,
    dagger_sdk::{DaggerConn, HostDirectoryOpts, Query, connect},
};

pub async fn run(dag: Query) -> eyre::Result<()> {
    let tag = std::env::var("RELEASE_TAG")?;
    let version = tag.trim_start_matches('v');

    let src = crate::src(&dag);
    let mut env = crate::env(&dag, src);

    env = env.with_exec(nix_develop(&[])).sync().await?;

    let cargo_token = dag.set_secret(
        "cargo-registry-token",
        std::env::var("CARGO_REGISTRY_TOKEN")?,
    );

    env.clone()
        .with_secret_variable("CARGO_REGISTRY_TOKEN", cargo_token)
        .with_exec(vec!["cargo", "publish", "-p", "tkbar"])
        .sync()
        .await?;

    // TODO: GitHub release, Nix, AUR

    Ok(())
}
