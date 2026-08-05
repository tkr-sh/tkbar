mod battery;
mod brightness;
mod volume;
mod wifi;
mod workspaces;

pub use {
    battery::battery,
    brightness::brightness,
    volume::volume,
    wifi::wifi,
    workspaces::workspaces,
};
use {
    gtk::{Box as GtkBox, Label, Orientation, glib, prelude::*},
    gtk4 as gtk,
};

pub enum Component {
    Logo(char),
    Workspaces,
    Spacer,
    Battery,
    Wifi,
    Brightness,
    Volume,
    Clock,
}

impl Component {
    pub(crate) fn add_to_bar(&self, bar: &GtkBox) {
        match self {
            Component::Logo(c) => {
                let logo = Label::new(Some(&c.to_string()));
                logo.add_css_class("logo");
                logo.set_xalign(0.45);
                bar.append(&logo);
            },
            Component::Workspaces => bar.append(&workspaces()),
            Component::Spacer => bar.append(&spacer()),
            Component::Battery => bar.append(&battery()),
            Component::Wifi => bar.append(&wifi()),
            Component::Brightness => bar.append(&brightness()),
            Component::Volume => bar.append(&volume()),
            Component::Clock => bar.append(&clock()),
        }
    }
}

pub fn spacer() -> GtkBox {
    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    spacer
}

pub fn clock() -> Label {
    let clock = Label::new(None);
    clock.add_css_class("clock");
    update_clock(&clock);

    glib::timeout_add_seconds_local(
        1,
        glib::clone!(
            #[weak]
            clock,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                update_clock(&clock);
                glib::ControlFlow::Continue
            }
        ),
    );

    clock
}


fn update_clock(label: &Label) {
    let now = chrono::Local::now();
    label.set_text(&now.format("%H\n%M\n%S").to_string());
}
