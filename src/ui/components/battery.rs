use {
    super::{Component, spawn_poller},
    std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, AtomicU8, Ordering},
        time::Duration,
    },
};
#[cfg(feature = "renderer-gtk")]
use {
    super::{GtkView, find_widget},
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, glib, prelude::*},
    gtk4::{self as gtk},
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
    fn percent(self) -> u8 {
        super::percent(self.energy_now, self.energy_full)
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
static IS_CHARGING: AtomicBool = AtomicBool::new(false);
static PERCENT: AtomicU8 = AtomicU8::new(100);
const CRITICAL_PERCENT: u8 = 15;

pub(crate) struct Battery {
    battery_tx: async_channel::Sender<BatteryState>,
    battery_rx: async_channel::Receiver<BatteryState>,
    ac_tx: async_channel::Sender<bool>,
    ac_rx: async_channel::Receiver<bool>,
}

impl Component for Battery {
    fn init() -> Self {
        let (battery_tx, battery_rx) = async_channel::unbounded();
        let (ac_tx, ac_rx) = async_channel::unbounded();

        Self {
            battery_tx,
            battery_rx,
            ac_tx,
            ac_rx,
        }
    }

    fn threads(&mut self) {
        let (batteries, acs) = find_power_supplies();

        if batteries.is_empty() {
            crate::log::warn("battery", "no battery found under /sys/class/power_supply");
        }

        if acs.is_empty() {
            crate::log::warn("ac", "no ac found under /sys/class/power_supply");
        }

        let mut warned = false;
        spawn_poller(self.battery_tx.clone(), POLL_INTERVAL_PERCENT, move || {
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

        spawn_poller(self.ac_tx.clone(), POLL_INTERVAL_CHARGING, move || {
            Some(acs.iter().any(|ac| read_online(ac) == Some(true)))
        });
    }
}

#[cfg(feature = "renderer-gtk")]
impl GtkView for Battery {
    fn gtk_box() -> GtkBox {
        let container = GtkBox::new(CONFIG.style.position.orientation(), 2);
        container.add_css_class("battery");

        let icon = Label::new(Some("󰁹"));
        icon.add_css_class("icon");
        icon.set_halign(gtk::Align::Center);

        let value = Label::new(Some("--"));
        value.add_css_class("value");
        value.set_halign(gtk::Align::Center);

        container.append(&icon);
        container.append(&value);

        container
    }

    fn gtk_threads(&self, container: &GtkBox) {
        let Some(icon) = find_widget::<Label>(container, "icon") else {
            return;
        };
        let Some(value) = find_widget::<Label>(container, "value") else {
            return;
        };

        let container = container.clone();
        let rx_battery = self.battery_rx.clone();
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

        let container = container.clone();
        let rx_ac = self.ac_rx.clone();
        glib::spawn_future_local(glib::clone!(
            #[weak]
            container,
            #[weak]
            icon,
            #[upgrade_or_default]
            async move {
                while let Ok(is_charging) = rx_ac.recv().await {
                    let percent = PERCENT.load(Ordering::Relaxed);

                    if is_charging {
                        container.add_css_class("charging");
                    } else {
                        container.remove_css_class("charging");
                    }

                    if !is_charging && percent <= CRITICAL_PERCENT {
                        container.add_css_class("critical");
                    } else {
                        container.remove_css_class("critical");
                    }

                    icon.set_text(icon_for(percent, is_charging));
                    IS_CHARGING.store(is_charging, Ordering::Relaxed);
                }
            }
        ));
    }
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


/// Aggregates battery data by summing raw values (e.g., µWh or µAh).
/// This results in a mathematically correct weighted average for multi-battery systems.
///
/// NOTE: This is structurally problematic if batteries expose mismatched units (e.g.,
/// one battery exposes `energy_*` in µWh, while another only exposes `capacity` in %).
/// Because `%` is a relative unit, it cannot be directly added to absolute units like `µWh`
/// without knowing the physical maximum capacity of the battery reporting the percentage.
///
/// In these mixed-unit scenarios, there is no way to calculate a perfectly correct weighted
/// percentage from userspace. Fortunately, in real-world Linux systems, kernel drivers
/// almost universally standardize on a single unit type across all batteries in a machine.
fn read_battery_state(device: &Path) -> Option<BatteryData> {
    // 1. Energy (µWh)
    if let (Ok(now), Ok(full)) = (
        fs::read_to_string(device.join("energy_now")),
        fs::read_to_string(device.join("energy_full")),
    ) && let (Ok(n), Ok(f)) = (now.trim().parse(), full.trim().parse())
    {
        return Some(BatteryData {
            energy_now: n,
            energy_full: f,
        });
    }

    // 2. Fallback charge (µAh)
    if let (Ok(now), Ok(full)) = (
        fs::read_to_string(device.join("charge_now")),
        fs::read_to_string(device.join("charge_full")),
    ) && let (Ok(n), Ok(f)) = (now.trim().parse(), full.trim().parse())
    {
        return Some(BatteryData {
            energy_now: n,
            energy_full: f,
        });
    }

    // 3. Ultimate fallback: Raw Capacity (%)
    if let Ok(cap) = fs::read_to_string(device.join("capacity")) &&
        let Ok(c) = cap.trim().parse::<u64>()
    {
        return Some(BatteryData {
            energy_now: c,
            energy_full: 100,
        });
    }

    None
}

fn read_online(device: &Path) -> Option<bool> {
    let is_online_as_u8: u8 = fs::read_to_string(device.join("online"))
        .ok()?
        .trim()
        .parse()
        .ok()?;

    Some(bool::try_from(is_online_as_u8).unwrap_or(true))
}



#[cfg(test)]
mod tests {
    use super::*;

    const fn battery(now: u64, full: u64) -> BatteryData {
        BatteryData {
            energy_now: now,
            energy_full: full,
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tkbar-battery-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn merging_batteries_yields_a_weighted_average() {
        assert_eq!(battery(25, 100).merge(battery(75, 100)).percent(), 50);

        // A nearly empty huge battery dominates a full tiny one.
        assert_eq!(battery(10, 10).merge(battery(0, 90)).percent(), 10);
    }

    #[test]
    fn read_battery_state_prefers_energy_over_charge_over_capacity() {
        let dir = tmpdir("fallback");

        fs::write(dir.join("capacity"), "42\n").unwrap();
        assert_eq!(read_battery_state(&dir), Some(battery(42, 100)));

        fs::write(dir.join("charge_now"), "30").unwrap();
        fs::write(dir.join("charge_full"), "60").unwrap();
        assert_eq!(read_battery_state(&dir), Some(battery(30, 60)));

        fs::write(dir.join("energy_now"), "5").unwrap();
        fs::write(dir.join("energy_full"), "80").unwrap();
        assert_eq!(read_battery_state(&dir), Some(battery(5, 80)));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_battery_state_is_none_without_any_source() {
        let dir = tmpdir("empty");
        assert_eq!(read_battery_state(&dir), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_online_parses_the_flag() {
        let dir = tmpdir("online");
        fs::write(dir.join("online"), "0\n").unwrap();
        assert_eq!(read_online(&dir), Some(false));
        fs::write(dir.join("online"), "1").unwrap();
        assert_eq!(read_online(&dir), Some(true));
        fs::write(dir.join("online"), "garbage").unwrap();
        assert_eq!(read_online(&dir), None);
        fs::remove_dir_all(dir).unwrap();
    }
}
