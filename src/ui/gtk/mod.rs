use {
    crate::{conf::CONFIG, ui::BarPosition},
    gtk::{Box as GtkBox, CssProvider, Orientation, glib, prelude::*},
    gtk4::{self as gtk, Window},
    gtk4_layer_shell::{Edge, Layer, LayerShell},
};

pub(crate) fn run() {
    harden_gtk_env();

    init_gtk();

    load_css();
    build_window();

    let main_loop = glib::MainLoop::new(None, false);

    main_loop.run();
}

#[cfg(feature = "black")]
const THEME_CSS: &str = include_str!("../styles/black.css");
#[cfg(feature = "blue")]
const THEME_CSS: &str = include_str!("../styles/blue.css");
#[cfg(feature = "cyan")]
const THEME_CSS: &str = include_str!("../styles/cyan.css");
#[cfg(feature = "green")]
const THEME_CSS: &str = include_str!("../styles/green.css");
#[cfg(feature = "orange")]
const THEME_CSS: &str = include_str!("../styles/orange.css");
#[cfg(feature = "pink")]
const THEME_CSS: &str = include_str!("../styles/pink.css");
#[cfg(feature = "purple")]
const THEME_CSS: &str = include_str!("../styles/purple.css");
#[cfg(feature = "red")]
const THEME_CSS: &str = include_str!("../styles/red.css");
#[cfg(feature = "white")]
const THEME_CSS: &str = include_str!("../styles/white.css");
#[cfg(feature = "yellow")]
const THEME_CSS: &str = include_str!("../styles/yellow.css");

fn init_gtk() {
    if let Err(err) = gtk::init() {
        crate::log::error("gtk", &format!("cannot initialize GTK: {err}"));
        std::process::exit(1);
    }
}

fn load_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        crate::log::error("gtk", "no default display");
        std::process::exit(1);
    };

    let provider = CssProvider::new();
    provider.load_from_string(THEME_CSS);
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

fn build_window() {
    let bar_size = i32::try_from(CONFIG.style.bar_size_px)
        .unwrap_or_else(|_| i32::try_from(crate::conf::default_bar_size_px()).unwrap_or_default());

    let mut builder = Window::builder();
    builder = match CONFIG.style.position {
        BarPosition::Left | BarPosition::Right => builder.default_width(bar_size),
        BarPosition::Top | BarPosition::Bottom => builder.default_height(bar_size),
    };
    let window = builder.build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace(Some("tkbar"));

    for edge in CONFIG.style.position.anchors() {
        window.set_anchor(edge, true);
    }

    window.auto_exclusive_zone_enable();

    let bar = GtkBox::new(CONFIG.style.position.orientation(), 8);
    bar.add_css_class("bar");

    if CONFIG.style.position.orientation() == Orientation::Vertical {
        bar.add_css_class("vertical");
    } else {
        bar.add_css_class("horizontal");
    }

    build_inner_window(&bar);

    window.set_child(Some(&bar));
    window.present();
}

fn build_inner_window(bar: &GtkBox) {
    for component in &CONFIG.behaviour.components {
        component.add_to_bar(bar);
    }
}

impl BarPosition {
    pub(crate) const fn orientation(self) -> Orientation {
        match self {
            BarPosition::Left | BarPosition::Right => Orientation::Vertical,
            BarPosition::Top | BarPosition::Bottom => Orientation::Horizontal,
        }
    }

    pub(crate) const fn anchors(self) -> [Edge; 3] {
        match self {
            BarPosition::Left => [Edge::Left, Edge::Top, Edge::Bottom],
            BarPosition::Right => [Edge::Right, Edge::Top, Edge::Bottom],
            BarPosition::Top => [Edge::Top, Edge::Left, Edge::Right],
            BarPosition::Bottom => [Edge::Bottom, Edge::Left, Edge::Right],
        }
    }
}

/// <https://docs.gtk.org/gtk4/running.html>
#[expect(
    unsafe_code,
    reason = "glib::setenv is unsafe: it mutates the global C environ. This runs at \
        the start of main, before GTK init and before any thread is spawned, so no \
        other thread can observe the environment concurrently."
)]
fn harden_gtk_env() {
    let set = |k, v| {
        // SAFETY: called before GTK init, while still single-threaded.
        let _ = unsafe { glib::setenv(k, v, true) };
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
