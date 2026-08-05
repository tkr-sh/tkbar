use {
    gtk::{Label, glib, prelude::*},
    gtk4 as gtk,
    std::{fs, process::Command, thread, time::Duration},
};

#[derive(Clone, Debug, PartialEq)]
enum WifiState {
    Connected { ssid: String, signal: Option<u32> },
    Disconnected,
}


pub fn wifi() -> Label {
    let icon = Label::new(Some("\u{f092d}"));
    icon.add_css_class("wifi");
    icon.set_halign(gtk::Align::Center);
    icon.set_justify(gtk::Justification::Center);

    let (tx, rx) = async_channel::unbounded::<WifiState>();

    thread::spawn(move || {
        let Some(device) = find_wireless_device() else {
            eprintln!("wifi: no wireless interface under /sys/class/net");
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

    // UI side.
    glib::spawn_future_local(glib::clone!(
        #[weak]
        icon,
        #[upgrade_or_default]
        async move {
            while let Ok(state) = rx.recv().await {
                icon.set_text(icon_for(&state));
                match &state {
                    WifiState::Connected { ssid, signal: _ } => {
                        icon.set_tooltip_text(Some(ssid));
                        icon.remove_css_class("down");
                    },
                    WifiState::Disconnected => {
                        icon.set_tooltip_text(Some("disconnected"));
                        icon.add_css_class("down");
                    },
                }
            }
        }
    ));

    icon
}

fn find_wireless_device() -> Option<String> {
    fs::read_dir("/sys/class/net")
        .ok()?
        .filter_map(Result::ok)
        .find(|e| e.path().join("wireless").exists())
        .and_then(|e| e.file_name().into_string().ok())
}

/// Parse `iwctl station <dev> show`. Typical (color-stripped) lines:
///     State                 connected
///     Connected network     MySSID
///     AverageRSSI           -52 dBm      (recent iwd only)
fn query(device: &str) -> Option<WifiState> {
    let out = Command::new("iwctl")
        .args(["station", device, "show"])
        .output()
        .ok()?;
    let text = strip_ansi(&String::from_utf8_lossy(&out.stdout));

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
    u32::try_from((2 * (dbm + 100)).clamp(0, 100)).unwrap_or_default()
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
