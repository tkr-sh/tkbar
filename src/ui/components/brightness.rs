use {
    gtk::{
        glib,
        prelude::*,
        Box as GtkBox,
        EventControllerScroll,
        EventControllerScrollFlags,
        GestureClick,
        Label,
        Orientation,
    },
    gtk4 as gtk,
    std::{process::Command, thread, time::Duration},
};


pub fn brightness() -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 2);
    container.add_css_class("brightness");

    let icon = Label::new(Some("\u{f00df}"));
    icon.add_css_class("brightness-icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_justify(gtk::Justification::Center);

    let value = Label::new(Some("--"));
    value.add_css_class("brightness-value");
    value.set_halign(gtk::Align::Center);
    value.set_justify(gtk::Justification::Center);

    container.append(&icon);
    container.append(&value);

    let (tx, rx) = async_channel::unbounded::<u32>();

    thread::spawn(move || {
        let Some(device) = find_device() else {
            eprintln!("brightness: no device under /sys/class/backlight");
            return;
        };

        let mut last: Option<u32> = None;
        loop {
            if let Some(percent) = read_percent(&device) {
                if last != Some(percent) {
                    last = Some(percent);
                    if tx.send_blocking(percent).is_err() {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });

    // UI side.
    glib::spawn_future_local(glib::clone!(
        #[weak]
        icon,
        #[weak]
        value,
        #[upgrade_or_default]
        async move {
            while let Ok(percent) = rx.recv().await {
                icon.set_text(icon_for(percent));
                value.set_text(&percent.to_string());
            }
        }
    ));

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    scroll.connect_scroll(|_, _dx, dy| {
        if dy < 0.0 {
            brightnessctl("5%+");
        } else {
            brightnessctl("5%-");
        }
        glib::Propagation::Stop
    });
    container.add_controller(scroll);

    let click = GestureClick::new();
    click.connect_released(|_, _, _, _| {
        brightnessctl("1");
    });
    container.add_controller(click);

    container
}

const fn icon_for(percent: u32) -> &'static str {
    match percent {
        0..=1 => "",
        2..=33 => "\u{f00de}",
        34..=66 => "\u{f00df}",
        _ => "\u{f00e0}",
    }
}


fn find_device() -> Option<std::path::PathBuf> {
    std::fs::read_dir("/sys/class/backlight")
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .next()
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

fn brightnessctl(step: &str) {
    let step = step.to_string();
    thread::spawn(move || {
        let _ = Command::new("brightnessctl").args(["set", &step]).status();
    });
}
