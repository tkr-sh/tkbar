use {
    crate::conf::CONFIG,
    gtk::{Application, ApplicationWindow, Box as GtkBox, Orientation, prelude::*},
    gtk4 as gtk,
    gtk4_layer_shell::{Edge, Layer, LayerShell},
};
mod components;

pub(crate) use components::Component;

const BAR_SIZE: i32 = 72;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
pub enum BarPosition {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl BarPosition {
    const fn orientation(self) -> Orientation {
        match self {
            BarPosition::Left | BarPosition::Right => Orientation::Vertical,
            BarPosition::Top | BarPosition::Bottom => Orientation::Horizontal,
        }
    }

    const fn anchors(self) -> [Edge; 3] {
        match self {
            BarPosition::Left => [Edge::Left, Edge::Top, Edge::Bottom],
            BarPosition::Right => [Edge::Right, Edge::Top, Edge::Bottom],
            BarPosition::Top => [Edge::Top, Edge::Left, Edge::Right],
            BarPosition::Bottom => [Edge::Bottom, Edge::Left, Edge::Right],
        }
    }
}

pub fn build_window(app: &Application) {
    let bar_size = i32::try_from(CONFIG.bar_size_px).unwrap_or(BAR_SIZE);

    let mut builder = ApplicationWindow::builder().application(app);
    builder = match CONFIG.position {
        BarPosition::Left | BarPosition::Right => builder.default_width(bar_size),
        BarPosition::Top | BarPosition::Bottom => builder.default_height(bar_size),
    };
    let window = builder.build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("tkbar");

    for edge in CONFIG.position.anchors() {
        window.set_anchor(edge, true);
    }

    window.auto_exclusive_zone_enable();

    let bar = GtkBox::new(CONFIG.position.orientation(), 8);
    bar.add_css_class("bar");

    if CONFIG.position.orientation() == Orientation::Vertical {
        bar.add_css_class("vertical");
    } else {
        bar.add_css_class("horizontal");
    }

    build_inner_window(&bar);

    window.set_child(Some(&bar));
    window.present();
}

pub fn build_inner_window(bar: &GtkBox) {
    for component in &crate::conf::CONFIG.components {
        component.add_to_bar(bar);
    }
}
