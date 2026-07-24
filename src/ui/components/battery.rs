use {
    gtk::{Box as GtkBox, Label, Orientation, glib, prelude::*},
    gtk4::{self as gtk},
    std::{
        fs,
        path::{Path, PathBuf},
        thread,
        time::Duration,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum BatteryState {
    NoBattery,
    Present { percent: u32, charging: bool },
}
pub fn battery() -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 2);
    container.add_css_class("battery");

    let icon = Label::new(Some("󰁹"));
    icon.add_css_class("battery-icon");
    icon.set_halign(gtk::Align::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("battery-value");
    value.set_halign(gtk::Align::Center);

    container.append(&icon);
    container.append(&value);

    let (tx, rx) = async_channel::unbounded::<BatteryState>();

    thread::spawn(move || {
        let Some(device) = find_battery() else {
            let _ = tx.send_blocking(BatteryState::NoBattery);
            return;
        };

        let mut last: Option<BatteryState> = None;
        loop {
            let state = read_state(&device).unwrap_or(BatteryState::NoBattery);
            if last.as_ref() != Some(&state) {
                last = Some(state);
                if tx.send_blocking(state).is_err() {
                    return;
                }
            }
            thread::sleep(Duration::from_secs(5));
        }
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
                    BatteryState::Present { percent, charging } => {
                        container.set_visible(true);
                        icon.set_text(icon_for(percent, charging));
                        value.set_text(&percent.to_string());

                        if charging {
                            container.add_css_class("charging");
                        } else {
                            container.remove_css_class("charging");
                        }
                        if !charging && percent <= 15 {
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
const fn icon_for(percent: u32, charging: bool) -> &'static str {
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


fn find_battery() -> Option<PathBuf> {
    let batteries: Vec<(BatteryState, PathBuf)> = fs::read_dir("/sys/class/power_supply")
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Battery"))
        .filter_map(|b| read_state(&b).map(|some| (some, b)))
        .collect();

    batteries
        .iter()
        .find(|(battery, _)| {
            if let BatteryState::Present { percent, charging } = battery {
                *percent != 100 || !*charging
            } else {
                false
            }
        })
        .or_else(|| batteries.first())
        .map(|(_, path)| path)
        .cloned()
}

fn read_state(device: &Path) -> Option<BatteryState> {
    let percent: u32 = fs::read_to_string(device.join("capacity"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let status = fs::read_to_string(device.join("status")).ok()?;
    let status = status.trim();

    let charging = status != "Discharging";

    Some(BatteryState::Present {
        percent: percent.min(100),
        charging,
    })
}
