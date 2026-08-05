use {
    gtk::{Application, ApplicationWindow, Box as GtkBox, Orientation, prelude::*},
    gtk4 as gtk,
    gtk4_layer_shell::{Edge, Layer, LayerShell},
};
mod components;

pub(crate) use components::Component;

const BAR_WIDTH: i32 = 72;

pub enum BarPosition {
    Left,
    Right,
    Top,
    Bottom,
}

pub fn build_window(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(i32::try_from(crate::conf::CONFIG.bar_size_px).unwrap_or(BAR_WIDTH))
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
    for component in &crate::conf::CONFIG.components {
        component.add_to_bar(bar);
    }
}
