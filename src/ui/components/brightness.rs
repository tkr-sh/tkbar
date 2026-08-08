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
    std::{path::PathBuf, process::Command, thread, time::Duration},
};

const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
enum BrightnessState {
    NoDevice,
    Present(u32),
}

pub fn brightness() -> GtkBox {
    let container = GtkBox::new(CONFIG.position.orientation(), 2);
    container.add_css_class("brightness");

    let icon = Label::new(Some("\u{f00df}"));
    icon.add_css_class("icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_justify(gtk::Justification::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("value");
    value.set_halign(gtk::Align::Center);
    value.set_justify(gtk::Justification::Center);

    container.append(&icon);
    container.append(&value);

    let device = find_device();
    if device.is_none() {
        crate::log::warn("brightness", "no device under /sys/class/backlight");
    }
    container.set_visible(device.is_some());

    let poll_device = device.clone();
    let (tx, rx) = super::spawn_poller(POLL_INTERVAL, move || {
        Some(match poll_device.as_deref().and_then(read_percent) {
            Some(percent) => BrightnessState::Present(percent),
            None => BrightnessState::NoDevice,
        })
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
                    BrightnessState::NoDevice => {
                        container.set_visible(false);
                    },
                    BrightnessState::Present(percent) => {
                        container.set_visible(true);
                        icon.set_text(icon_for(percent));
                        value.set_text(&percent.to_string());
                    },
                }
            }
        }
    ));

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    let scroll_tx = tx.clone();
    let scroll_device = device.clone();
    scroll.connect_scroll(move |_, _dx, dy| {
        if dy < 0.0 {
            brightnessctl("5%+", scroll_device.clone(), scroll_tx.clone());
        } else {
            brightnessctl("5%-", scroll_device.clone(), scroll_tx.clone());
        }
        glib::Propagation::Stop
    });
    container.add_controller(scroll);

    let click = GestureClick::new();
    let click_tx = tx.clone();
    let click_device = device.clone();
    click.connect_released(move |_, _, _, _| {
        brightnessctl("1", click_device.clone(), click_tx.clone());
    });
    container.add_controller(click);

    container
}

const fn icon_for(percent: u32) -> &'static str {
    match percent {
        0..=1 => "",
        2..=33 => "󰃞",
        34..=66 => "󰃟",
        _ => "󰃠",
    }
}


fn find_device() -> Option<PathBuf> {
    let mut devices: Vec<(u32, PathBuf)> = std::fs::read_dir("/sys/class/backlight")
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|p| {
            let max = std::fs::read_to_string(p.join("max_brightness"))
                .ok()?
                .trim()
                .parse::<u32>()
                .ok()?;
            Some((max, p))
        })
        .collect();

    devices.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.as_path().cmp(b.1.as_path())));
    devices.into_iter().next().map(|(_, p)| p)
}

fn read_percent(device: &std::path::Path) -> Option<u32> {
    let read_u32 = |name: &str| -> Option<u32> {
        std::fs::read_to_string(device.join(name))
            .ok()?
            .trim()
            .parse()
            .ok()
    };
    let current = read_u32("brightness")?;
    let max = read_u32("max_brightness")?;
    if max == 0 {
        return None;
    }
    Some((current * 100 + max / 2) / max)
}

fn brightnessctl(step: &str, device: Option<PathBuf>, tx: async_channel::Sender<BrightnessState>) {
    let step = step.to_string();
    thread::spawn(move || {
        let _ = Command::new("brightnessctl").args(["set", &step]).status();
        if let Some(device) = &device &&
            let Some(percent) = read_percent(device)
        {
            let _ = tx.send_blocking(BrightnessState::Present(percent));
        }
    });
}
