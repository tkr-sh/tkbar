use {
    dagger_sdk::{HostDirectoryOpts, connect},
    std::process::ExitCode,
    tkbar_ci::{push, release},
};



#[tokio::main]
async fn main() -> ExitCode {
    let cmd = std::env::args().nth(1);

    let result = connect(|dag| {
        async move {
            match cmd.as_deref() {
                Some("release") => release::run(dag).await,
                _ => push::run(dag).await,
            }
        }
    })
    .await;

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fail: {e:#}");
            ExitCode::FAILURE
        },
    }
}
