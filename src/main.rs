use {
    gtk::glib,
    gtk4 as gtk,
    tkbar::{build_window, init_gtk, load_css},
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

    harden_gtk_env();

    init_gtk();

    load_css();
    build_window();

    let main_loop = glib::MainLoop::new(None, false);

    main_loop.run();
}

/// <https://docs.gtk.org/gtk4/running.html>
fn harden_gtk_env() {
    let set = |k, v| {
        let _ = glib::setenv(k, v, true);
    };

    // no image loaders
    set("GDK_PIXBUF_MODULE_FILE", "/dev/null");
    // no AT-SPI D-Bus export
    set("GTK_A11Y", "none");
    // no GL/Vulkan, no driver dlopen
    set("GSK_RENDERER", "cairo");
    set("GDK_DISABLE", "gl,vulkan,dmabuf");
    // never fall back to X11
    set("GDK_BACKEND", "wayland");
    // no xdg-desktop-portal round-trips
    set("GDK_DEBUG", "no-portals");
}
