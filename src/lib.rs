use {gtk::CssProvider, gtk4 as gtk};

pub use crate::ui::build_window;

mod conf;
mod log;
mod ui;

pub const APP_ID: &str = "dev.tk.tkbar";

pub fn load_css() {
    let display = gtk::gdk::Display::default().expect("Could not get default display");

    let provider = CssProvider::new();
    provider.load_from_string(theme_css());
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    #[cfg(feature = "config")]
    load_custom_css(&display);
}

#[cfg(feature = "config")]
fn load_custom_css(display: &gtk::gdk::Display) {
    let Some(base) = directories::BaseDirs::new() else {
        return;
    };
    let path = base.config_dir().join("tkbar").join("style.css");
    let css = match std::fs::read_to_string(&path) {
        Ok(css) => css,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            crate::log::warn("config", &format!("cannot read {}: {err}", path.display()));
            return;
        },
    };

    let provider = CssProvider::new();
    provider.load_from_string(&css);
    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

const fn theme_css() -> &'static str {
    if cfg!(feature = "black") {
        include_str!("ui/styles/black.css")
    } else if cfg!(feature = "blue") {
        include_str!("ui/styles/blue.css")
    } else if cfg!(feature = "cyan") {
        include_str!("ui/styles/cyan.css")
    } else if cfg!(feature = "green") {
        include_str!("ui/styles/green.css")
    } else if cfg!(feature = "orange") {
        include_str!("ui/styles/orange.css")
    } else if cfg!(feature = "pink") {
        include_str!("ui/styles/pink.css")
    } else if cfg!(feature = "purple") {
        include_str!("ui/styles/purple.css")
    } else if cfg!(feature = "red") {
        include_str!("ui/styles/red.css")
    } else if cfg!(feature = "white") {
        include_str!("ui/styles/white.css")
    } else if cfg!(feature = "yellow") {
        include_str!("ui/styles/yellow.css")
    } else {
        include_str!("ui/styles/red.css")
    }
}

const fn color_feature_count() -> usize {
    let mut n = 0;
    if cfg!(feature = "black") {
        n += 1;
    }
    if cfg!(feature = "blue") {
        n += 1;
    }
    if cfg!(feature = "cyan") {
        n += 1;
    }
    if cfg!(feature = "green") {
        n += 1;
    }
    if cfg!(feature = "orange") {
        n += 1;
    }
    if cfg!(feature = "pink") {
        n += 1;
    }
    if cfg!(feature = "purple") {
        n += 1;
    }
    if cfg!(feature = "red") {
        n += 1;
    }
    if cfg!(feature = "white") {
        n += 1;
    }
    if cfg!(feature = "yellow") {
        n += 1;
    }
    n
}

const _: () = assert!(
    color_feature_count() == 1,
    "exactly one color feature must be enabled: black, blue, cyan, green, orange, pink, purple, red, white, yellow"
);
