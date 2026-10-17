#[cfg(feature = "battery")]
mod battery;
#[cfg(feature = "brightness")]
mod brightness;
#[cfg(feature = "clock")]
mod clock;
#[cfg(feature = "volume")]
mod volume;
#[cfg(feature = "wifi")]
mod wifi;
#[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
mod workspaces;

#[cfg(any(feature = "battery", feature = "brightness", feature = "wifi"))]
use std::{thread, time::Duration};

#[cfg(feature = "battery")]
pub(crate) use battery::Battery;
#[cfg(feature = "brightness")]
pub(crate) use brightness::Brightness;
#[cfg(feature = "clock")]
pub(crate) use clock::Clock;
#[cfg(feature = "volume")]
pub(crate) use volume::Volume;
#[cfg(feature = "wifi")]
pub(crate) use wifi::Wifi;
#[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
pub(crate) use workspaces::Workspaces;
use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, prelude::*},
    gtk4 as gtk,
};

#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "doc", derive(serde::Serialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
pub enum Component {
    Logo(char),
    #[cfg(any(feature = "niri", feature = "hyprland", feature = "sway"))]
    Workspaces,
    Spacer,
    #[cfg(feature = "battery")]
    Battery,
    #[cfg(feature = "wifi")]
    Wifi,
    #[cfg(feature = "brightness")]
    Brightness,
    #[cfg(feature = "volume")]
    Volume,
    #[cfg(feature = "clock")]
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
            Component::Workspaces => bar.append(&Workspaces::create_all()),
            Component::Spacer => bar.append(&spacer()),
            #[cfg(feature = "battery")]
            Component::Battery => bar.append(&Battery::create_all()),
            #[cfg(feature = "wifi")]
            Component::Wifi => bar.append(&Wifi::create_all()),
            #[cfg(feature = "brightness")]
            Component::Brightness => bar.append(&Brightness::create_all()),
            #[cfg(feature = "volume")]
            Component::Volume => bar.append(&Volume::create_all()),
            #[cfg(feature = "clock")]
            Component::Clock => bar.append(&Clock::create_all()),
        }
    }
}

#[cfg(any(
    feature = "battery",
    feature = "brightness",
    feature = "clock",
    feature = "volume",
    feature = "wifi",
    feature = "niri",
    feature = "hyprland",
    feature = "sway",
))]
trait ComponentWidget: Sized {
    fn init() -> Self;
    fn gtk_box() -> GtkBox;
    fn gtk_threads(&self, container: &GtkBox);
    fn threads(&mut self);

    fn create_all() -> GtkBox {
        let container = Self::gtk_box();
        let mut component = Self::init();
        component.gtk_threads(&container);
        component.threads();
        container
    }
}

#[cfg(any(
    feature = "battery",
    feature = "brightness",
    feature = "clock",
    feature = "volume",
    feature = "wifi",
))]
fn find_widget<T>(container: &GtkBox, css_class: &str) -> Option<T>
where
    T: IsA<gtk::Widget>,
{
    let mut child = container.first_child();
    while let Some(current) = child {
        if current.has_css_class(css_class) {
            return current.downcast::<T>().ok();
        }
        child = current.next_sibling();
    }
    None
}


#[cfg(any(feature = "battery", feature = "brightness", feature = "wifi"))]
pub(crate) fn spawn_poller<S, P>(tx: async_channel::Sender<S>, interval: Duration, mut poll: P)
where
    S: Clone + PartialEq + Send + 'static,
    P: FnMut() -> Option<S> + Send + 'static,
{
    thread::spawn(move || {
        let mut last: Option<S> = None;
        loop {
            if let Some(state) = poll() &&
                last.as_ref() != Some(&state)
            {
                last = Some(state.clone());
                if tx.send_blocking(state).is_err() {
                    return;
                }
            }
            thread::sleep(interval);
        }
    });
}

pub(crate) fn spacer() -> GtkBox {
    let spacer = GtkBox::new(CONFIG.style.position.orientation(), 0);
    spacer.set_vexpand(true);
    spacer.set_hexpand(true);
    spacer
}

#[cfg(any(feature = "battery", feature = "brightness"))]
fn percent(current: u64, max: u64) -> u8 {
    let percent = current.saturating_mul(100) / if max == 0 { 1 } else { max };

    u8::try_from(percent.min(100)).unwrap_or(100)
}



#[cfg(all(test, any(feature = "battery", feature = "brightness")))]
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
