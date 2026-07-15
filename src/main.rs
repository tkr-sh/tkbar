use {
    gtk::{glib, prelude::*, Application},
    gtk4 as gtk,
    project::{build_window, load_css, APP_ID},
};

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| load_css());
    app.connect_activate(build_window);

    app.run()
}
