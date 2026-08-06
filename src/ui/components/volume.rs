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

#[derive(Clone, Debug, PartialEq)]
struct VolState {
    percent: u32,
    muted: bool,
}

pub fn volume() -> GtkBox {
    let container = GtkBox::new(CONFIG.position.orientation(), 2);
    container.add_css_class("volume");

    let icon = Label::new(Some("\u{f057f}"));
    icon.add_css_class("volume-icon");
    icon.set_halign(gtk::Align::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("volume-value");
    value.set_halign(gtk::Align::Center);

    container.append(&icon);
    container.append(&value);

    let (tx, rx) = async_channel::unbounded::<VolState>();

    let poll_tx = tx.clone();
    thread::spawn(move || {
        let mut last: Option<VolState> = None;
        let mut warned = false;
        loop {
            match query() {
                Some(state) if last.as_ref() != Some(&state) => {
                    last = Some(state.clone());
                    if poll_tx.send_blocking(state).is_err() {
                        return;
                    }
                },
                None if !warned => {
                    crate::log::warn("volume", "waiting for `wpctl get-volume` to succeed");
                    warned = true;
                },
                Some(_) | None => {},
            }
            thread::sleep(Duration::from_millis(500));
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
        wpctl(&["set-mute", SINK, "toggle"], click_tx.clone());
    });
    container.add_controller(click);

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    let scroll_tx = tx.clone();
    scroll.connect_scroll(move |_, _dx, dy| {
        if dy < 0.0 {
            wpctl(&["set-volume", SINK, "3%+"], scroll_tx.clone());
        } else {
            wpctl(&["set-volume", SINK, "3%-"], scroll_tx.clone());
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
        let _ = Command::new("wpctl").args(&args).status();
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
