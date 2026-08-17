use {
    crate::conf::CONFIG,
    gtk::{Align, Box as GtkBox, Button, glib, prelude::*},
    gtk4 as gtk,
    std::thread,
};

#[cfg(feature = "hyprland")]
mod hypr;
#[cfg(feature = "niri")]
mod niri;
#[cfg(feature = "sway")]
mod sway;

#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "hyprland",
    expect(dead_code, reason = "`idx` is not used in hyprland")
)]
struct Ws {
    id: u64,
    /// Maximum of 255 workspaces
    idx: u8,
    label: String,
    is_active: bool,
    is_focused: bool,
}

pub fn workspaces() -> GtkBox {
    let container = GtkBox::new(CONFIG.position.orientation(), 4);
    container.add_css_class("workspaces");

    let (tx, rx) = async_channel::unbounded::<Vec<Ws>>();

    thread::spawn(move || {
        #[cfg(feature = "niri")]
        if let Err(e) = niri::event_loop(&tx) {
            crate::log::warn("workspaces", &format!("niri IPC error: {e}"));
        }

        #[cfg(feature = "hyprland")]
        if let Err(e) = hypr::event_loop(&tx) {
            crate::log::warn("workspaces", &format!("hyprland IPC error: {e}"));
        }

        #[cfg(feature = "sway")]
        if let Err(e) = sway::event_loop(&tx) {
            crate::log::warn("workspaces", &format!("sway IPC error: {e}"));
        }
    });

    glib::spawn_future_local(glib::clone!(
        #[weak]
        container,
        #[upgrade_or_default]
        async move {
            while let Ok(list) = rx.recv().await {
                while let Some(child) = container.first_child() {
                    container.remove(&child);
                }

                for ws in &list {
                    if !CONFIG.should_show_empty_workspace && !ws.is_active && ws.is_focused {
                        continue;
                    }

                    let btn = Button::with_label(&ws.label);
                    btn.add_css_class("workspace");
                    if ws.is_focused {
                        btn.add_css_class("current");
                    }
                    if !ws.is_active {
                        btn.add_css_class("inactive");
                    }
                    btn.set_valign(Align::Center);
                    btn.set_halign(Align::Center);

                    let id = ws.id;
                    btn.connect_clicked(move |_| focus_workspace(id));
                    container.append(&btn);
                }
            }
        }
    ));

    container
}

fn focus_workspace(id: u64) {
    thread::spawn(move || {
        #[cfg(feature = "niri")]
        niri::focus_workspace(id);

        #[cfg(feature = "hyprland")]
        hypr::focus_workspace(id);

        #[cfg(feature = "sway")]
        sway::focus_workspace(id);
    });
}
