use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, glib, prelude::*},
    gtk4 as gtk,
    std::{fs, process::Command, thread, time::Duration},
};

#[derive(Clone, Debug, PartialEq)]
enum WifiState {
    Connected { ssid: String, signal: Option<u32> },
    Disconnected,
}


pub fn wifi() -> GtkBox {
    let container = GtkBox::new(CONFIG.position.orientation(), 2);
    container.add_css_class("wifi");

    let icon = Label::new(Some("\u{f092d}"));
    icon.add_css_class("icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_justify(gtk::Justification::Center);

    container.append(&icon);

    let (tx, rx) = async_channel::unbounded::<WifiState>();

    thread::spawn(move || {
        let Some(device) = find_wireless_device() else {
            crate::log::warn("wifi", "no wireless interface under /sys/class/net");
            return;
        };

        let mut last: Option<WifiState> = None;
        loop {
            let state = query(&device).unwrap_or(WifiState::Disconnected);
            if last.as_ref() != Some(&state) {
                last = Some(state.clone());
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

fn query(device: &str) -> Option<WifiState> {
    let out = Command::new("iwctl")
        .args(["station", device, "show"])
        .output()
        .ok()?;

    parse_iwctl_show(&String::from_utf8_lossy(&out.stdout), device)
}

/// Parses `iwctl station <dev> show`. Typical (color-stripped) lines:
///     State                 connected
///     Connected network     MySSID
///     AverageRSSI           -52 dBm      (recent iwd only)
fn parse_iwctl_show(text: &str, device: &str) -> Option<WifiState> {
    let text = strip_ansi(text);

    let mut connected = false;
    let mut ssid: Option<String> = None;
    let mut rssi_dbm: Option<i32> = None;

    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("State") {
            connected = v.trim() == "connected";
        } else if let Some(v) = line.strip_prefix("Connected network") {
            let v = v.trim();
            if !v.is_empty() {
                ssid = Some(v.to_string());
            }
        } else if let Some(v) = line
            .strip_prefix("AverageRSSI")
            .or_else(|| line.strip_prefix("RSSI"))
        {
            rssi_dbm = v.split_whitespace().next().and_then(|n| n.parse().ok());
        }
    }

    if connected {
        Some(WifiState::Connected {
            ssid: ssid.unwrap_or_else(|| device.to_string()),
            signal: rssi_dbm.map(dbm_to_percent),
        })
    } else {
        Some(WifiState::Disconnected)
    }
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

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for t in chars.by_ref() {
                if t.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
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
