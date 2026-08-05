//! Having configuration and parsing is a new surface attack and bloats binary with extra
//! dependencies.
//!
//! Therefore, there is an hardcoded config and a customizable config for the users liking.

use crate::ui::Component;

pub struct Config {
    pub bar_size_px: usize,
    pub components: Vec<Component>,
}

#[cfg(feature = "config")]
mod hardcoded {
    use {
        crate::{conf::Config, ui::Component},
        std::sync::LazyLock,
    };

    pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
        Config {
            bar_size_px: 72,
            components: vec![
                Component::Logo('󱄅'),
                Component::Workspaces,
                Component::Spacer,
                Component::Battery,
                Component::Wifi,
                Component::Brightness,
                Component::Volume,
                Component::Clock,
            ],
        }
    });
}

#[cfg(feature = "config")]
pub use hardcoded::*;
