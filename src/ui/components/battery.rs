use {
    crate::{conf::CONFIG, ui::Component::Battery},
    gtk::{Box as GtkBox, Label, glib, prelude::*},
    gtk4::{self as gtk},
    std::{
        convert::Infallible,
        fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum BatteryState {
    NoBattery,
    Present(BatteryData),
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct BatteryData {
    energy_now: u64,
    energy_full: u64,
    charging: Charging,
}

impl BatteryData {
    #[expect(clippy::as_conversions, reason = "u64 to f64")]
    const fn percent(self) -> u8 {
        (self.energy_now as f64 /
            if self.energy_full == 0 {
                1
            } else {
                self.energy_full
            } as f64 *
            100.0)
            .round() as u8
    }

    fn merge(self, r: Self) -> Self {
        Self {
            energy_now: self.energy_now + r.energy_now,
            energy_full: self.energy_full + r.energy_full,
            charging: self.charging.merge(r.charging),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Charging {
    Full,
    Charging,
    Discharging,
    #[default]
    Unknown,
    NotCharging,
}

impl Charging {
    #[inline]
    fn merge(self, r: Self) -> Self {
        if self == Self::Full && r == Self::Full {
            Self::Full
        } else if self == Self::Charging || r == Self::Charging {
            Self::Charging
        } else if self == Self::Discharging || r == Self::Discharging {
            Self::Discharging
        } else if self == Self::NotCharging || r == Self::NotCharging {
            Self::NotCharging
        } else {
            Self::Unknown
        }
    }
}

impl Charging {
    fn is_charging(self) -> bool {
        Self::Charging == self
    }

    const fn to_css_class_name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::NotCharging => "not-charging",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for Charging {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[allow(
            clippy::wildcard_in_or_patterns,
            reason = "Makes it clearer that 'Unknown' and _ are different"
        )]
        Ok(match s {
            "Full" => Self::Full,
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Not charging" => Self::NotCharging,
            "Unknown" | _ => Self::Unknown,
        })
    }
}

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const CRITICAL_PERCENT: u8 = 15;

pub fn battery() -> GtkBox {
    let container = GtkBox::new(CONFIG.position.orientation(), 2);
    container.add_css_class("battery");

    let icon = Label::new(Some("󰁹"));
    icon.add_css_class("icon");
    icon.set_halign(gtk::Align::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("value");
    value.set_halign(gtk::Align::Center);

    container.append(&icon);
    container.append(&value);

    let devices = find_batteries();
    if devices.is_empty() {
        crate::log::warn("battery", "no battery found under /sys/class/power_supply");
    }

    let mut warned = false;
    let (_, rx) = super::spawn_poller(POLL_INTERVAL, move || {
        Some(
            devices
                .iter()
                .filter_map(|device| {
                    match read_battery_state(device) {
                        Some(state) => {
                            println!("{state:#?}");
                            Some(state)
                        },
                        None => {
                            if !warned {
                                crate::log::warn(
                                    "battery",
                                    &format!("could not read state for {}", device.display()),
                                );
                                warned = true;
                            }
                            None
                        },
                    }
                })
                .reduce(|acc, c| acc.merge(c))
                .map_or(BatteryState::NoBattery, BatteryState::Present),
        )
    });

    glib::spawn_future_local(glib::clone!(
        #[weak]
        container,
        #[weak]
        icon,
        #[weak]
        value,
        #[upgrade_or_default]
        async move {
            while let Ok(state) = rx.recv().await {
                match state {
                    BatteryState::NoBattery => {
                        container.set_visible(false);
                    },
                    BatteryState::Present(battery) => {
                        container.set_visible(true);
                        icon.set_text(icon_for(battery.percent(), battery.charging.is_charging()));
                        value.set_text(&battery.percent().to_string());

                        container
                            .set_css_classes(&["battery", battery.charging.to_css_class_name()]);

                        if !battery.charging.is_charging() && battery.percent() <= CRITICAL_PERCENT
                        {
                            container.add_css_class("critical");
                        } else {
                            container.remove_css_class("critical");
                        }
                    },
                }
            }
        }
    ));

    container
}

#[allow(
    clippy::wildcard_in_or_patterns,
    reason = "Make it clearer that in theory it should just be 100, but it's also the default case"
)]
const fn icon_for(percent: u8, charging: bool) -> &'static str {
    if charging {
        match percent {
            0..=9 => "󰢟",
            10..=19 => "󰢜",
            20..=29 => "󰂆",
            30..=39 => "󰂇",
            40..=49 => "󰂈",
            50..=59 => "󰢝",
            60..=69 => "󰂉",
            70..=79 => "󰢞",
            80..=89 => "󰂊",
            90..=99 => "󰂋",
            100 | _ => "󰂅",
        }
    } else {
        match percent {
            0..=9 => "󰂎",
            10..=19 => "󰁺",
            20..=29 => "󰁻",
            30..=39 => "󰁼",
            40..=49 => "󰁽",
            50..=59 => "󰁾",
            60..=69 => "󰁿",
            70..=79 => "󰂀",
            80..=89 => "󰂁",
            90..=99 => "󰂂",
            100 | _ => "󰁹",
        }
    }
}


fn find_batteries() -> Vec<PathBuf> {
    let Ok(sys_power_supply_dir) = fs::read_dir("/sys/class/power_supply") else {
        return Vec::default();
    };


    sys_power_supply_dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Battery"))
        .collect()
}

fn find_ac() -> Vec<PathBuf> {
    let Ok(sys_power_supply_dir) = fs::read_dir("/sys/class/power_supply") else {
        return Vec::default();
    };


    sys_power_supply_dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Mains"))
        .collect()
}

fn read_battery_state(device: &Path) -> Option<BatteryData> {
    let energy_now: u64 = fs::read_to_string(device.join("energy_now"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let energy_full: u64 = fs::read_to_string(device.join("energy_full"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let status = fs::read_to_string(device.join("status")).ok()?;
    let status = status.trim();

    Some(BatteryData {
        energy_now,
        energy_full,
        charging: Charging::from_str(status).unwrap_or_default(),
    })
}

fn read_online(device: &Path) -> Option<bool> {
    let is_online_as_u8: u8 = fs::read_to_string(device.join("online"))
        .ok()?
        .trim()
        .parse()
        .ok()?;

    Some(bool::try_from(is_online_as_u8).unwrap_or(true))
}
