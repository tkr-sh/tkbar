use {
    crate::conf::CONFIG,
    gtk::{
        Box as GtkBox,
        EventControllerScroll,
        EventControllerScrollFlags,
        GestureClick,
        Label,
        glib,
        prelude::*,
    },
    gtk4 as gtk,
    std::{process::Command, thread, time::Duration},
};

const SINK: &str = "@DEFAULT_AUDIO_SINK@";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
struct VolState {
    percent: u32,
    muted: bool,
}

pub fn volume() -> GtkBox {
    let container = GtkBox::new(CONFIG.style.position.orientation(), 2);
    container.add_css_class("volume");

    let icon = Label::new(Some("\u{f057f}"));
    icon.add_css_class("icon");
    icon.set_halign(gtk::Align::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("value");
    value.set_halign(gtk::Align::Center);

    container.append(&icon);
    container.append(&value);

    let mut warned = false;
    let (tx, rx) = super::spawn_poller(POLL_INTERVAL, move || {
        match query() {
            Some(state) => Some(state),
            None => {
                if !warned {
                    crate::log::warn("volume", "waiting for `wpctl get-volume` to succeed");
                    warned = true;
                }
                None
            },
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
            while let Ok(s) = rx.recv().await {
                icon.set_text(icon_for(&s));
                if s.muted {
                    value.set_text("mute");
                    container.add_css_class("muted");
                } else {
                    value.set_text(&s.percent.to_string());
                    container.remove_css_class("muted");
                }
            }
        }
    ));

    let click = GestureClick::new();
    let click_tx = tx.clone();
    click.connect_released(move |_, _, _, _| {
        wpctl(["set-mute", SINK, "toggle"].as_slice(), click_tx.clone());
    });
    container.add_controller(click);

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    let scroll_tx = tx.clone();
    scroll.connect_scroll(move |_, _dx, dy| {
        if dy < 0.0 {
            wpctl(
                [
                    "set-volume",
                    SINK,
                    &format!("{}%+", CONFIG.behaviour.on_scroll_volume_step),
                ]
                .as_slice(),
                scroll_tx.clone(),
            );
        } else {
            wpctl(
                [
                    "set-volume",
                    SINK,
                    &format!("{}%-", CONFIG.behaviour.on_scroll_volume_step),
                ]
                .as_slice(),
                scroll_tx.clone(),
            );
        }
        glib::Propagation::Stop
    });
    container.add_controller(scroll);

    container
}

const fn icon_for(state: &VolState) -> &'static str {
    if state.muted {
        "󰝟"
    } else {
        match state.percent {
            0..=20 => "󰕿",
            21..=50 => "󰖀",
            51..=100 => "󰕾",
            _ => "󱄡",
        }
    }
}

fn query() -> Option<VolState> {
    let out = Command::new("wpctl")
        .args(["get-volume", SINK])
        .output()
        .ok()?;

    parse_get_volume(&String::from_utf8_lossy(&out.stdout))
}

/// Parses `wpctl get-volume` output: `"Volume: X.XX"` or `"Volume: X.XX [MUTED]"`.
fn parse_get_volume(text: &str) -> Option<VolState> {
    let muted = text.contains("[MUTED]");
    let fraction: f64 = text.split_whitespace().nth(1)?.parse().ok()?;
    #[allow(
        clippy::as_conversions,
        reason = "f64->u32 `as` saturates, which is exactly what we want for a volume fraction"
    )]
    let percent: u32 = (fraction * 100.0).round() as u32;

    Some(VolState { percent, muted })
}

fn wpctl(args: &[&str], tx: async_channel::Sender<VolState>) {
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    thread::spawn(move || {
        if let Err(err) = Command::new("wpctl").args(&args).status() {
            crate::log::warn("volume", &format!("failed to run wpctl: {err}"));
        }
        if let Some(state) = query() {
            let _ = tx.send_blocking(state);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_volume() {
        assert_eq!(
            parse_get_volume("Volume: 0.84"),
            Some(VolState {
                percent: 84,
                muted: false
            })
        );
    }

    #[test]
    fn parses_muted() {
        assert_eq!(
            parse_get_volume("Volume: 0.84 [MUTED]"),
            Some(VolState {
                percent: 84,
                muted: true
            })
        );
    }

    #[test]
    fn parses_boosted_volume() {
        assert_eq!(parse_get_volume("Volume: 1.50").unwrap().percent, 150);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_get_volume(""), None);
        assert_eq!(parse_get_volume("Volume: abc"), None);
        assert_eq!(parse_get_volume("whatever"), None);
    }
}
