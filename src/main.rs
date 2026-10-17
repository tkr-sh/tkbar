#[cfg(feature = "cli")]
#[derive(clap::Parser)]
#[command(name = "tkbar", version, about = "A GTK4 status bar for Wayland")]
struct Cli {
    /// Path to configuration file
    #[cfg(feature = "config")]
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
}

fn main() {
    #[cfg(feature = "cli")]
    {
        #[cfg(not(feature = "config"))]
        {
            let _cli = <Cli as clap::Parser>::parse();
        }

        #[cfg(feature = "config")]
        {
            let cli = <Cli as clap::Parser>::parse();
            if let Some(config_path) = cli.config {
                use tkbar::CLI_CONFIG_PATH;

                let _ = CLI_CONFIG_PATH.set(config_path.into_boxed_path());
            }
        }
    }

    tkbar::run();
}
