use {
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, glib, prelude::*},
    gtk4 as gtk,
    std::time::Duration,
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
            ssid: ssid_to_string(&ssid),
            signal,
        });
    }

    Some(WifiState::Disconnected)
}

fn ssid_to_string(ssid: &[u8]) -> String {
    String::from_utf8_lossy(ssid)
        .chars()
        .filter(|c| !c.is_control())
        .collect()
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
                        0 => "󰤯",
                        1..=24 => "󰤟",
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
    fn dbm_to_percent_clamps() {
        assert_eq!(dbm_to_percent(-100), 0);
        assert_eq!(dbm_to_percent(-75), 50);
        assert_eq!(dbm_to_percent(-50), 100);
        assert_eq!(dbm_to_percent(0), 100);
    }

    #[test]
    fn malicious_ssid_ansi_injection_is_inert() {
        let out = ssid_to_string(b"\x1b[31mEVIL\x1b[0m");
        assert!(out.chars().all(|c| !c.is_control()));
        assert_eq!(out, "[31mEVIL[0m");
    }

    #[test]
    fn malicious_ssid_control_chars_are_stripped() {
        let out = ssid_to_string(b"\x01\x02Home\x7f\x0aNet\x00");
        assert!(out.chars().all(|c| !c.is_control()));
        assert_eq!(out, "HomeNet");
    }

    #[test]
    fn malicious_ssid_invalid_utf8_does_not_panic() {
        let out = ssid_to_string(&[0xff, 0xfe, 0xfa, b'A']);
        assert!(out.chars().all(|c| !c.is_control()));
        assert!(out.contains('A'));
    }
}
