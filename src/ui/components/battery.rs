use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, glib, prelude::*},
    gtk4::{self as gtk},
    std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            LazyLock,
            atomic::{AtomicBool, AtomicU8, Ordering},
        },
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

    const fn merge(self, r: Self) -> Self {
        Self {
            energy_now: self.energy_now + r.energy_now,
            energy_full: self.energy_full + r.energy_full,
        }
    }
}


const POLL_INTERVAL_PERCENT: Duration = Duration::from_secs(10);
const POLL_INTERVAL_CHARGING: Duration = Duration::from_millis(500);
static IS_CHARGING: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
static PERCENT: LazyLock<AtomicU8> = LazyLock::new(|| AtomicU8::new(100));
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

    let (batteries, acs) = find_power_supplies();

    if batteries.is_empty() {
        crate::log::warn("battery", "no battery found under /sys/class/power_supply");
    }

    if acs.is_empty() {
        crate::log::warn("ac", "no ac found under /sys/class/power_supply");
    }


    let mut warned = false;
    let (_, rx_battery) = super::spawn_poller(POLL_INTERVAL_PERCENT, move || {
        Some(
            batteries
                .iter()
                .filter_map(|battery| {
                    match read_battery_state(battery) {
                        Some(state) => Some(state),
                        None => {
                            if !warned {
                                crate::log::warn(
                                    "battery",
                                    &format!("could not read state for {}", battery.display()),
                                );
                                warned = true;
                            }
                            None
                        },
                    }
                })
                .reduce(BatteryData::merge)
                .map_or(BatteryState::NoBattery, BatteryState::Present),
        )
    });

    let (_, rx_ac) = super::spawn_poller(POLL_INTERVAL_CHARGING, move || {
        Some(acs.iter().any(|ac| read_online(ac) == Some(true)))
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
            while let Ok(state) = rx_battery.recv().await {
                match state {
                    BatteryState::NoBattery => {
                        container.set_visible(false);
                    },
                    BatteryState::Present(battery) => {
                        container.set_visible(true);
                        let is_charging = IS_CHARGING.load(Ordering::Relaxed);
                        icon.set_text(icon_for(battery.percent(), is_charging));
                        value.set_text(&battery.percent().to_string());

                        if !is_charging && battery.percent() <= CRITICAL_PERCENT {
                            container.add_css_class("critical");
                        } else {
                            container.remove_css_class("critical");
                        }

                        PERCENT.store(battery.percent(), Ordering::Relaxed);
                    },
                }
            }
        }
    ));

    glib::spawn_future_local(glib::clone!(
        #[weak]
        container,
        #[weak]
        icon,
        #[upgrade_or_default]
        async move {
            while let Ok(is_charging) = rx_ac.recv().await {
                if is_charging {
                    container.add_css_class("charging");
                } else {
                    container.remove_css_class("charging");
                }
                icon.set_text(icon_for(PERCENT.load(Ordering::Relaxed), is_charging));
                IS_CHARGING.store(is_charging, Ordering::Relaxed);
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


fn find_power_supplies() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let Ok(sys_power_supply_dir) = fs::read_dir("/sys/class/power_supply") else {
        return (Vec::default(), Vec::default());
    };

    sys_power_supply_dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .fold(
            (Vec::new(), Vec::new()),
            |(mut batteries, mut acs), path| {
                if let Ok(type_str) = fs::read_to_string(path.join("type")) {
                    match type_str.trim() {
                        "Battery" => batteries.push(path),
                        "Mains" => acs.push(path),
                        _ => (),
                    }
                }

                (batteries, acs)
            },
        )
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

    Some(BatteryData {
        energy_now,
        energy_full,
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
