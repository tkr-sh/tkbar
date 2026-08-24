use {
    gtk::glib,
    gtk4 as gtk,
    tkbar::{build_window, load_css},
};

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
        let cli = <Cli as clap::Parser>::parse();

        if let Some(config_path) = cli.config {
            use tkbar::CLI_CONFIG_PATH;

            let _ = CLI_CONFIG_PATH.set(config_path.into_boxed_path());
        }
    }

    disable_image_loaders();

    let _ = gtk::init();

    load_css();
    build_window();

    let main_loop = glib::MainLoop::new(None, false);

    main_loop.run();
}

fn disable_image_loaders() {
    let _ = glib::setenv("GDK_PIXBUF_MODULE_FILE", "/dev/null", true);
}
