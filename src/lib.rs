use {gtk::CssProvider, gtk4 as gtk};

pub use crate::ui::build_window;

mod conf;
mod ui;

pub const APP_ID: &str = "dev.tk.tkbar";

pub fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string(include_str!("style.css"));

    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
