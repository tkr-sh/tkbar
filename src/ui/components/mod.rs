mod battery;
mod brightness;
mod volume;
mod wifi;
#[cfg(any(feature = "niri", feature = "hyprland"))]
mod workspaces;

#[cfg(any(feature = "niri", feature = "hyprland"))]
pub use workspaces::workspaces;
use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, Orientation, glib, prelude::*},
    gtk4 as gtk,
    std::{
        fmt::Write as _,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
};
pub use {battery::battery, brightness::brightness, volume::volume, wifi::wifi};

#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
pub enum Component {
    Logo(char),
    #[cfg(any(feature = "niri", feature = "hyprland"))]
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
            #[cfg(any(feature = "niri", feature = "hyprland"))]
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

pub(crate) fn spawn_poller<S, P>(
    interval: Duration,
    mut poll: P,
) -> (async_channel::Sender<S>, async_channel::Receiver<S>)
where
    S: Clone + PartialEq + Send + 'static,
    P: FnMut() -> Option<S> + Send + 'static,
{
    let (tx, rx) = async_channel::unbounded();
    let poll_tx = tx.clone();
    thread::spawn(move || {
        let mut last: Option<S> = None;
        loop {
            if let Some(state) = poll() &&
                last.as_ref() != Some(&state)
            {
                last = Some(state.clone());
                if poll_tx.send_blocking(state).is_err() {
                    return;
                }
            }
            thread::sleep(interval);
        }
    });
    (tx, rx)
}

pub fn spacer() -> GtkBox {
    let spacer = GtkBox::new(CONFIG.position.orientation(), 0);
    spacer.set_vexpand(true);
    spacer.set_hexpand(true);
    spacer
}

pub fn clock() -> Label {
    let clock = Label::new(None);
    clock.add_css_class("clock");
    let mut buf = String::with_capacity(8);
    update_clock(&clock, &mut buf);

    glib::timeout_add_seconds_local(
        1,
        glib::clone!(
            #[weak]
            clock,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                update_clock(&clock, &mut buf);
                glib::ControlFlow::Continue
            }
        ),
    );

    clock
}


fn update_clock(label: &Label, buf: &mut String) {
    let (h, m, s) = get_hms_now();
    buf.clear();
    let c = if CONFIG.position.orientation() == Orientation::Horizontal {
        ' '
    } else {
        '\n'
    };
    let _ = write!(buf, "{h:02}{c}{m:02}{c}{s:02}",);
    label.set_text(buf);
}


fn get_hms_now() -> (u8, u8, u8) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let day_secs = secs % 86_400;

    let h = u8::try_from(day_secs / 3_600).unwrap_or_default();
    let m = u8::try_from((day_secs % 3_600) / 60).unwrap_or_default();
    let s = u8::try_from(day_secs % 60).unwrap_or_default();

    (h, m, s)
}
