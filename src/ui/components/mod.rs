mod battery;
mod brightness;
mod clock;
#[cfg(feature = "volume")]
mod volume;
#[cfg(feature = "wifi")]
mod wifi;
#[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
mod workspaces;

#[cfg(feature = "volume")]
pub use volume::volume;
#[cfg(feature = "wifi")]
pub use wifi::wifi;
#[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
pub use workspaces::workspaces;
use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, prelude::*},
    gtk4 as gtk,
    std::{thread, time::Duration},
};
pub use {battery::battery, brightness::brightness, clock::clock};

#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "doc", derive(serde::Serialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
pub enum Component {
    Logo(char),
    #[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
    Workspaces,
    Spacer,
    Battery,
    #[cfg(feature = "wifi")]
    Wifi,
    Brightness,
    #[cfg(feature = "volume")]
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
            #[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
            Component::Workspaces => bar.append(&workspaces()),
            Component::Spacer => bar.append(&spacer()),
            Component::Battery => bar.append(&battery()),
            #[cfg(feature = "wifi")]
            Component::Wifi => bar.append(&wifi()),
            Component::Brightness => bar.append(&brightness()),
            #[cfg(feature = "volume")]
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
    let spacer = GtkBox::new(CONFIG.style.position.orientation(), 0);
    spacer.set_vexpand(true);
    spacer.set_hexpand(true);
    spacer
}

fn percent(current: u64, max: u64) -> u8 {
    let percent = current.saturating_mul(100) / if max == 0 { 1 } else { max };

    u8::try_from(percent.min(100)).unwrap_or(100)
}



#[cfg(test)]
mod tests {
    use super::percent;

    #[test]
    fn percent_truncates_and_clamps_to_a_hundred() {
        assert_eq!(percent(0, 100), 0);
        assert_eq!(percent(50, 100), 50);
        assert_eq!(percent(1, 3), 33);
        assert_eq!(percent(150, 100), 100);
    }

    #[test]
    fn percent_never_divides_by_zero() {
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(42, 0), 100);
    }
}
