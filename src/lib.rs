use {gtk::CssProvider, gtk4 as gtk};

pub use crate::ui::build_window;

mod conf;
mod log;
mod ui;

pub const APP_ID: &str = "dev.tk.tkbar";

pub fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string(theme_css());

    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
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
