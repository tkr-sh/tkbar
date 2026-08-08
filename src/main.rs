use {
    gtk::{Application, glib, prelude::*},
    gtk4 as gtk,
    tkbar::{APP_ID, build_window, load_css},
};

fn main() -> glib::ExitCode {
    disable_image_loaders();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| load_css());
    app.connect_activate(build_window);

    app.run()
}

fn disable_image_loaders() {
    let _ = glib::setenv("GDK_PIXBUF_MODULE_FILE", "/dev/null", true);
}
