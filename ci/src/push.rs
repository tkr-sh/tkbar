use {crate::nix_develop, dagger_sdk::DaggerConn};

const STEPS: &[&str] = &[
    "css",
    "ci-fmt",
    "ci-check",
    "ci-clippy",
    "ci-test",
];

pub async fn run(dag: DaggerConn) -> eyre::Result<()> {
    let src = crate::src(&dag);
    let mut env = crate::env(&dag, src);

    env = env.with_exec(nix_develop(&[])).sync().await?;

    for recipe in STEPS {
        env = env.with_exec(nix_develop(&["just", recipe])).sync().await?;
    }

    Ok(())
}
