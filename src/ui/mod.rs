use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Orientation, prelude::*},
    gtk4::{self as gtk, Window},
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

pub fn build_window() {
    let monitors = target_monitors();

    if monitors.is_empty() {
        build_window_for_monitor(None);
        return;
    }

    for monitor in &monitors {
        build_window_for_monitor(Some(monitor));
    }
}

fn target_monitors() -> Vec<gtk::gdk::Monitor> {
    let Some(display) = gtk::gdk::Display::default() else {
        crate::log::warn("ui", "no display, letting the compositor place the bar");
        return Vec::new();
    };

    let monitors = monitors(&display);
    let wanted = &CONFIG.style.outputs;

    if wanted.is_empty() {
        return monitors;
    }

    let mut selected = Vec::new();
    let mut missing = Vec::new();

    for name in wanted {
        let matches = select_monitors(&monitors, name);
        if matches.is_empty() {
            missing.push(name.as_str());
            continue;
        }

        for monitor in matches {
            if !selected.contains(&monitor) {
                selected.push(monitor);
            }
        }
    }

    if selected.is_empty() {
        let available = monitors.iter().map(monitor_label).collect::<Vec<_>>();
        let available = if available.is_empty() {
            "none".to_owned()
        } else {
            available.join(", ")
        };

        crate::log::warn(
            "ui",
            &format!("outputs {wanted:?} not found (available: {available}), using all outputs"),
        );

        return monitors;
    }

    if !missing.is_empty() {
        crate::log::warn(
            "ui",
            &format!("outputs {missing:?} not found, ignoring them"),
        );
    }

    selected
}

fn monitors(display: &gtk::gdk::Display) -> Vec<gtk::gdk::Monitor> {
    let list = display.monitors();

    (0..list.n_items())
        .filter_map(|index| list.item(index))
        .filter_map(|item| item.downcast::<gtk::gdk::Monitor>().ok())
        .collect()
}

fn select_monitors(monitors: &[gtk::gdk::Monitor], wanted: &str) -> Vec<gtk::gdk::Monitor> {
    let by_connector = monitors
        .iter()
        .filter(|monitor| output_name_matches(monitor.connector().as_deref(), wanted))
        .cloned()
        .collect::<Vec<_>>();

    if !by_connector.is_empty() {
        return by_connector;
    }

    let by_model = monitors
        .iter()
        .filter(|monitor| output_name_matches(monitor.model().as_deref(), wanted))
        .cloned()
        .collect::<Vec<_>>();

    if !by_model.is_empty() {
        return by_model;
    }

    monitors
        .iter()
        .filter(|monitor| output_name_matches(monitor.description().as_deref(), wanted))
        .cloned()
        .collect()
}

fn output_name_matches(candidate: Option<&str>, wanted: &str) -> bool {
    candidate.is_some_and(|candidate| candidate.eq_ignore_ascii_case(wanted))
}

fn monitor_label(monitor: &gtk::gdk::Monitor) -> String {
    monitor
        .connector()
        .or_else(|| monitor.model())
        .or_else(|| monitor.description())
        .map_or_else(|| String::from("unknown"), |name| name.to_string())
}

fn build_window_for_monitor(monitor: Option<&gtk::gdk::Monitor>) {
    let bar_size = i32::try_from(CONFIG.style.bar_size_px)
        .unwrap_or_else(|_| i32::try_from(crate::conf::default_bar_size_px()).unwrap_or_default());

    let mut builder = Window::builder();
    builder = match CONFIG.style.position {
        BarPosition::Left | BarPosition::Right => builder.default_width(bar_size),
        BarPosition::Top | BarPosition::Bottom => builder.default_height(bar_size),
    };
    let window = builder.build();

    window.init_layer_shell();
    window.set_monitor(monitor);
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

pub(crate) fn build_inner_window(bar: &GtkBox) {
    for component in &crate::conf::CONFIG.behaviour.components {
        component.add_to_bar(bar);
    }
}

#[cfg(test)]
mod tests {
    use super::output_name_matches;

    #[test]
    fn output_names_match_case_insensitively() {
        assert!(output_name_matches(Some("DP-1"), "dp-1"));
        assert!(output_name_matches(Some("hdmi-a-1"), "HDMI-A-1"));
        assert!(!output_name_matches(Some("DP-2"), "DP-1"));
        assert!(!output_name_matches(None, "DP-1"));
    }
}
