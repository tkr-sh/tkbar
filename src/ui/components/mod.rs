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
    gtk::{glib, prelude::*, Box as GtkBox, Label, Orientation},
    gtk4 as gtk,
};

pub fn nix_logo() -> Label {
    let logo = Label::new(Some("󱄅"));
    logo.add_css_class("logo");
    logo.set_xalign(0.45);
    logo
}

// pub fn workspaces() -> GtkBox {
//     let container = GtkBox::new(Orientation::Vertical, 10);
//     container.add_css_class("workspaces");
//
//     for idx in 1..=10 {
//         let wsp = Label::new(Some(&idx.to_string()));
//         wsp.add_css_class("workspace");
//
//         if idx % 2 == 0 {
//             wsp.add_css_class("workspace-inactive");
//         }
//
//         if idx % 6 == 0 {
//             wsp.add_css_class("workspace-current");
//         }
//
//         container.append(&wsp);
//     }
//
//     container
// }

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
