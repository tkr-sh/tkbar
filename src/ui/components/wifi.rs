use {
    super::{ComponentWidget, find_widget, spawn_poller},
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

pub(crate) struct Wifi {
    tx: async_channel::Sender<WifiState>,
    rx: async_channel::Receiver<WifiState>,
}

impl ComponentWidget for Wifi {
    fn init() -> Self {
        let (tx, rx) = async_channel::unbounded();

        Self { tx, rx }
    }

    fn gtk_box() -> GtkBox {
        let container = GtkBox::new(CONFIG.style.position.orientation(), 2);
        container.add_css_class("wifi");

        let icon = Label::new(Some("\u{f092d}"));
        icon.add_css_class("icon");
        icon.set_halign(gtk::Align::Center);
        icon.set_justify(gtk::Justification::Center);

        container.append(&icon);

        container
    }

    fn gtk_threads(&self, container: &GtkBox) {
        let Some(icon) = find_widget::<Label>(container, "icon") else {
            return;
        };
        let container = container.clone();
        let rx = self.rx.clone();
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
    }

    fn threads(&mut self) {
        let mut warned = false;
        spawn_poller(self.tx.clone(), POLL_INTERVAL, move || {
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
    }
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

const REPLACEMENT_CHAR: char = '�';

/// Also sanitizes the SSID into a "safe" [`String`] to display.
fn ssid_to_string(ssid: &[u8]) -> String {
    String::from_utf8_lossy(ssid)
        .chars()
        .map(|c| if is_malicious(c) { REPLACEMENT_CHAR } else { c })
        .collect()
}

/// Source: <https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt>: `Default_Ignorable_Code_Point`
const fn is_malicious(c: char) -> bool {
    c.is_control() ||
        matches!(
            c,
            // <https://www.unicode.org/Public/UCD/latest/ucd/DerivedCoreProperties.txt>: `Default_Ignorable_Code_Point`
            '\u{00AD}' |
            '\u{034F}' |
            '\u{061C}' |
            '\u{115F}'..='\u{1160}' |
            '\u{17B4}'..='\u{17B5}' |
            '\u{180B}'..='\u{180F}' |
            '\u{200B}'..='\u{200F}' |
            '\u{202A}'..='\u{202E}' |
            '\u{2060}'..='\u{206F}' |
            '\u{3164}' |
            '\u{FE00}'..='\u{FE0F}' |
            '\u{FEFF}' |
            '\u{FFA0}' |
            '\u{FFF0}'..='\u{FFF8}' |
            '\u{1BCA0}'..='\u{1BCA3}' |
            '\u{1D173}'..='\u{1D17A}' |
            '\u{E0000}'..='\u{E0FFF}'
        )
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
    fn malicious_ssid_control_chars_are_marked() {
        assert_eq!(
            ssid_to_string(b"\x1b[31mEVIL\x1b[0m"),
            "\u{FFFD}[31mEVIL\u{FFFD}[0m"
        );
        assert_eq!(
            ssid_to_string(b"\x01Home\x7f\x0aNet\x00"),
            "\u{FFFD}Home\u{FFFD}\u{FFFD}Net\u{FFFD}"
        );
    }

    #[test]
    fn malicious_ssid_format_chars_are_marked() {
        // RLO would otherwise render the tail reversed and hide it.
        assert_eq!(
            ssid_to_string("Free_WiFi\u{202E}moc.knab".as_bytes()),
            "Free_WiFi\u{FFFD}moc.knab"
        );
        assert_eq!(
            ssid_to_string("Ho\u{200B}me\u{FEFF}Net".as_bytes()),
            "Ho\u{FFFD}me\u{FFFD}Net"
        );
    }

    #[test]
    fn malicious_ssid_invalid_utf8_uses_the_same_marker() {
        assert_eq!(ssid_to_string(&[0xff, b'A', 0xfe]), "\u{FFFD}A\u{FFFD}");
    }

    #[test]
    fn legitimate_ssids_are_untouched() {
        for ssid in ["家のWiFi", "Café ☕", "Wi-Fi (5 GHz)"] {
            assert_eq!(ssid_to_string(ssid.as_bytes()), ssid);
        }
    }

    #[test]
    fn output_is_always_displayable() {
        let all: String = (0..=0x0010_FFFF_u32).filter_map(char::from_u32).collect();
        assert!(
            ssid_to_string(all.as_bytes())
                .chars()
                .all(|c| !is_malicious(c))
        );
    }
}
