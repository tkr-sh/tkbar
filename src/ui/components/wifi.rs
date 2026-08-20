use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, glib, prelude::*},
    gtk4 as gtk,
    std::{fs, process::Command, time::Duration},
};

#[derive(Clone, Debug, PartialEq)]
enum WifiState {
    Connected { ssid: String, signal: Option<u32> },
    Disconnected,
}

const POLL_INTERVAL: Duration = Duration::from_secs(5);

pub fn wifi() -> GtkBox {
    let container = GtkBox::new(CONFIG.style.position.orientation(), 2);
    container.add_css_class("wifi");

    let icon = Label::new(Some("\u{f092d}"));
    icon.add_css_class("icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_justify(gtk::Justification::Center);

    container.append(&icon);

    let device = find_wireless_device();
    if device.is_none() {
        crate::log::warn("wifi", "no wireless interface under /sys/class/net");
    }

    let mut warned = false;
    let (_, rx) = super::spawn_poller(POLL_INTERVAL, move || {
        match query() {
            Some(state) => Some(state),
            None => {
                if !warned {
                    crate::log::warn("wifi", "nl80211 query failed");
                    warned = true;
                }
                Some(WifiState::Disconnected)
            },
        }
    });

    glib::spawn_future_local(glib::clone!(
        #[weak]
        container,
        #[weak]
        icon,
        #[upgrade_or_default]
        async move {
            while let Ok(state) = rx.recv().await {
                icon.set_text(icon_for(&state));
                match &state {
                    WifiState::Connected { ssid, signal: _ } => {
                        container.set_tooltip_text(Some(ssid));
                        container.remove_css_class("down");
                    },
                    WifiState::Disconnected => {
                        container.set_tooltip_text(Some("disconnected"));
                        container.add_css_class("down");
                    },
                }
            }
        }
    ));

    container
}

fn find_wireless_device() -> Option<String> {
    fs::read_dir("/sys/class/net")
        .ok()?
        .filter_map(Result::ok)
        .find(|e| e.path().join("wireless").exists())
        .and_then(|e| e.file_name().into_string().ok())
}

fn query() -> Option<WifiState> {
    let mut socket = neli_wifi::Socket::connect().ok()?;
    let interfaces = socket.get_interfaces_info().ok()?;

    for iface in interfaces {
        let (Some(index), Some(ssid)) = (iface.index, iface.ssid) else {
            continue;
        };
        if ssid.is_empty() {
            continue;
        }

        let signal = socket
            .get_station_info(index)
            .ok()
            .and_then(|stations| stations.into_iter().next())
            .and_then(|s| s.average_signal.or(s.signal))
            .map(|dbm| dbm_to_percent(i32::from(dbm)));

        return Some(WifiState::Connected {
            ssid: String::from_utf8_lossy(&ssid).into_owned(),
            signal,
        });
    }

    Some(WifiState::Disconnected)
}


fn dbm_to_percent(dbm: i32) -> u32 {
    let scaled = dbm.saturating_add(100).saturating_mul(2);
    u32::try_from(scaled.clamp(0, 100)).unwrap_or_default()
}

const fn icon_for(state: &WifiState) -> &'static str {
    match state {
        WifiState::Disconnected => "󰤮",
        WifiState::Connected { signal, .. } => {
            match signal {
                Some(s) => {
                    match s {
                        0..=24 => "󰤟",
                        25..=49 => "󰤢",
                        50..=74 => "󰤥",
                        _ => "󰤨",
                    }
                },
                None => "󰤯",
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connected() {
        let out = parse_iwctl_show(
            "    State                 connected\n    Connected network     MySSID\n",
            "wlan0",
        );
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: "MySSID".to_string(),
                signal: None,
            })
        );
    }

    #[test]
    fn parses_connected_with_rssi() {
        let out = parse_iwctl_show(
            "    State                 connected\n    Connected network     MySSID\n    AverageRSSI           -52 dBm\n",
            "wlan0",
        );
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: "MySSID".to_string(),
                signal: Some(dbm_to_percent(-52)),
            })
        );
    }

    #[test]
    fn parses_disconnected() {
        let out = parse_iwctl_show("    State                 disconnected\n", "wlan0");
        assert_eq!(out, Some(WifiState::Disconnected));
    }

    #[test]
    fn ssid_falls_back_to_device_name() {
        let out = parse_iwctl_show("    State                 connected\n", "wlan0");
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: "wlan0".to_string(),
                signal: None,
            })
        );
    }

    #[test]
    fn strips_ansi_before_parsing() {
        let out = parse_iwctl_show(
            "\u{1b}[1;32m    State\u{1b}[0m                 connected\n",
            "wlan0",
        );
        assert!(matches!(out, Some(WifiState::Connected { .. })));
    }

    #[test]
    fn dbm_to_percent_clamps() {
        assert_eq!(dbm_to_percent(-100), 0);
        assert_eq!(dbm_to_percent(-75), 50);
        assert_eq!(dbm_to_percent(-50), 100);
        assert_eq!(dbm_to_percent(0), 100);
    }

    #[test]
    fn malicious_ssid_ansi_injection_is_stripped() {
        let out = parse_iwctl_show(
            "    State                 connected\n    Connected network     \u{1b}[31mEVIL\u{1b}[0m\u{1b}[2J\n",
            "wlan0",
        );
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: "EVIL".to_string(),
                signal: None,
            })
        );
    }

    #[test]
    fn malicious_ssid_cannot_forge_other_fields() {
        let out = parse_iwctl_show(
            "    State                 connected\n    Connected network     State disconnected AverageRSSI 999\n",
            "wlan0",
        );
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: "State disconnected AverageRSSI 999".to_string(),
                signal: None,
            })
        );
    }

    #[test]
    fn malicious_rssi_overflow_is_clamped() {
        assert_eq!(dbm_to_percent(i32::MAX), 100);
        assert_eq!(dbm_to_percent(i32::MIN), 0);
    }

    #[test]
    fn malicious_rssi_out_of_range_is_ignored() {
        let out = parse_iwctl_show(
            "    State                 connected\n    Connected network     X\n    AverageRSSI 99999999999999999999 dBm\n",
            "wlan0",
        );
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: "X".to_string(),
                signal: None,
            })
        );
    }

    #[test]
    fn malicious_ssid_unicode_and_length_are_inert() {
        let long = "😀".repeat(10_000);
        let input =
            format!("    State                 connected\n    Connected network     {long}\n");
        let out = parse_iwctl_show(&input, "wlan0");
        assert_eq!(
            out,
            Some(WifiState::Connected {
                ssid: long,
                signal: None,
            })
        );
    }

    #[test]
    fn malicious_garbage_input_never_panics() {
        for input in [
            "",
            "\0\0\0",
            "State",
            "Connected network",
            "AverageRSSI -",
            "\u{1b}\u{1b}[",
            "󱄅󱄅󱄅",
            "State connected Connected network AverageRSSI",
        ] {
            let _ = parse_iwctl_show(input, "wlan0");
        }
    }
}
