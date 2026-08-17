use {
    crate::conf::CONFIG,
    gtk::{Application, ApplicationWindow, Box as GtkBox, Orientation, prelude::*},
    gtk4 as gtk,
    gtk4_layer_shell::{Edge, Layer, LayerShell},
};
mod components;

pub(crate) use components::Component;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "doc", derive(serde::Serialize))]
#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
#[cfg_attr(
    not(feature = "config"),
    allow(
        dead_code,
        reason = "non-default positions are only reachable through the config feature"
    )
)]
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
    let bar_size = i32::try_from(CONFIG.style.bar_size_px)
        .unwrap_or_else(|_| i32::try_from(crate::conf::default_bar_size_px()).unwrap_or_default());

    let mut builder = ApplicationWindow::builder().application(app);
    builder = match CONFIG.style.position {
        BarPosition::Left | BarPosition::Right => builder.default_width(bar_size),
        BarPosition::Top | BarPosition::Bottom => builder.default_height(bar_size),
    };
    let window = builder.build();

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace("tkbar");

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

pub fn build_inner_window(bar: &GtkBox) {
    for component in &crate::conf::CONFIG.behaviour.components {
        component.add_to_bar(bar);
    }
}
