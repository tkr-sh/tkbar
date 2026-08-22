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
    std::{path::PathBuf, thread, time::Duration},
};

const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
enum BrightnessState {
    NoDevice,
    Present(u8),
}

pub fn brightness() -> GtkBox {
    let container = GtkBox::new(CONFIG.style.position.orientation(), 2);
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
    let mut warned = false;
    let (tx, rx) = super::spawn_poller(POLL_INTERVAL, move || {
        Some(match poll_device.as_deref() {
            Some(device) => {
                match Brightness::from_path(device) {
                    Some(brightness) => BrightnessState::Present(brightness.percent()),
                    None => {
                        if !warned {
                            crate::log::warn(
                                "brightness",
                                &format!("could not read brightness for {}", device.display()),
                            );
                            warned = true;
                        }
                        BrightnessState::NoDevice
                    },
                }
            },
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
            set_brightness(
                scroll_device.clone(),
                SetValue::Relative(
                    i8::try_from(CONFIG.behaviour.on_scroll_brightness_step).unwrap_or(0),
                ),
                scroll_tx.clone(),
            );
        } else {
            set_brightness(
                scroll_device.clone(),
                SetValue::Relative(
                    i8::try_from(
                        CONFIG
                            .behaviour
                            .on_scroll_brightness_step
                            .min(u8::try_from(i8::MAX).unwrap_or(0)),
                    )
                    .unwrap_or(0)
                    .saturating_neg(),
                ),
                scroll_tx.clone(),
            );
        }
        glib::Propagation::Stop
    });
    container.add_controller(scroll);

    let click = GestureClick::new();
    let click_tx = tx.clone();
    let click_device = device.clone();
    click.connect_released(move |_, _, _, _| {
        set_brightness(
            click_device.clone(),
            SetValue::Absolute(1),
            click_tx.clone(),
        );
    });
    container.add_controller(click);

    container
}

const fn icon_for(percent: u8) -> &'static str {
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

#[derive(Debug, Copy, Clone)]
struct Brightness {
    actual_brightness: u64,
    max_brightness: u64,
}

impl Brightness {
    fn from_path(device: &std::path::Path) -> Option<Self> {
        let read_u64 = |name: &str| -> Option<u64> {
            std::fs::read_to_string(device.join(name))
                .ok()?
                .trim()
                .parse()
                .ok()
        };
        let actual_brightness = read_u64("brightness")?;
        let max_brightness = read_u64("max_brightness")?;
        if max_brightness == 0 {
            return None;
        }

        Some(Self {
            actual_brightness,
            max_brightness,
        })
    }

    fn percent(&self) -> u8 {
        let percent = self.actual_brightness.saturating_mul(100) /
            if self.max_brightness == 0 {
                1
            } else {
                self.max_brightness
            };

        u8::try_from(percent.min(100)).unwrap_or(100)
    }
}



enum SetValue {
    /// Set the value to in actual_brightness
    Absolute(u64),
    /// Increase / Decrease by, in percentage
    Relative(i8),
}

fn set_brightness(
    device: Option<PathBuf>,
    set_value: SetValue,
    tx: async_channel::Sender<BrightnessState>,
) {
    if let Some(device) = device &&
        let Some(mut brightness) = Brightness::from_path(&device)
    {
        thread::spawn(move || {
            let new_brightness = match set_value {
                SetValue::Absolute(absolute) => absolute,
                SetValue::Relative(relative) => {
                    if relative > 0 {
                        brightness.max_brightness.min(
                            brightness.actual_brightness +
                                (u64::try_from(relative)
                                    .unwrap_or(0)
                                    .saturating_mul(brightness.max_brightness)) /
                                    100,
                        )
                    } else {
                        brightness.actual_brightness.saturating_sub(
                            (u64::try_from(relative.saturating_neg())
                                .unwrap_or(0)
                                .saturating_mul(brightness.max_brightness)) /
                                100,
                        )
                    }
                },
            };

            if let Err(err) = std::fs::write(&device, new_brightness.to_string()) {
                crate::log::warn("brightness", &format!("failed to set brightness: {err}"));
            }

            brightness.actual_brightness = new_brightness;

            let _ = tx.send_blocking(BrightnessState::Present(brightness.percent()));
        });
    } else {
        crate::log::warn("brightness", "cannot adjust brightness: device unavailable");
    }
}
