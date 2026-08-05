use {
    gtk::{Application, glib, prelude::*},
    gtk4 as gtk,
    tkbar::{APP_ID, build_window, load_css},
};

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| load_css());
    app.connect_activate(build_window);

    app.run()
}
