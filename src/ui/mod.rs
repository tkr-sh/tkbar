use {
    gtk::{Application, ApplicationWindow, Box as GtkBox, Orientation, prelude::*},
    gtk4 as gtk,
    gtk4_layer_shell::{Edge, Layer, LayerShell},
};
mod components;

const BAR_WIDTH: i32 = 72;

pub fn build_window(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(BAR_WIDTH)
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("tkbar");

    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);

    window.auto_exclusive_zone_enable();


    let bar = GtkBox::new(Orientation::Vertical, 8);
    bar.add_css_class("bar");

    build_inner_window(&bar);

    window.set_child(Some(&bar));
    window.present();
}

pub fn build_inner_window(bar: &GtkBox) {
    bar.append(&components::nix_logo());
    bar.append(&components::workspaces());
    bar.append(&components::spacer());
    bar.append(&components::battery());
    bar.append(&components::wifi());
    bar.append(&components::brightness());
    bar.append(&components::volume());
    bar.append(&components::clock());
}
