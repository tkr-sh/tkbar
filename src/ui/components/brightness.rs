use {
    super::{Component, spawn_poller},
    std::{path::PathBuf, thread, time::Duration},
};
#[cfg(feature = "renderer-gtk")]
use {
    super::{GtkView, find_widget},
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
};

const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
enum BrightnessState {
    NoDevice,
    Present(u8),
}

pub(crate) struct Brightness {
    device: Option<PathBuf>,
    poll_tx: async_channel::Sender<BrightnessState>,
    poll_rx: async_channel::Receiver<BrightnessState>,
    cmd_tx: std::sync::mpsc::Sender<Cmd>,
    cmd_rx: Option<std::sync::mpsc::Receiver<Cmd>>,
}

impl Component for Brightness {
    fn init() -> Self {
        let device = find_device();
        if device.is_none() {
            crate::log::warn("brightness", "no device under /sys/class/backlight");
        }

        let (poll_tx, poll_rx) = async_channel::unbounded();
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();

        Self {
            device,
            poll_tx,
            poll_rx,
            cmd_tx,
            cmd_rx: Some(cmd_rx),
        }
    }

    fn threads(&mut self) {
        let poll_device = self.device.clone();
        let mut warned = false;
        spawn_poller(self.poll_tx.clone(), POLL_INTERVAL, move || {
            Some(match poll_device.as_deref() {
                Some(device) => {
                    match Backlight::from_path(device) {
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

        let Some(device) = self.device.clone() else {
            return;
        };
        let Some(cmd_rx) = self.cmd_rx.take() else {
            return;
        };
        let cmd_device = device.clone();
        let tx = self.poll_tx.clone();
        thread::spawn(move || {
            for cmd in cmd_rx {
                let Some(mut brightness) = Backlight::from_path(&cmd_device) else {
                    crate::log::warn("brightness", "device vanished");
                    continue;
                };

                let new_brightness = brightness.apply(&cmd);

                match std::fs::write(device.join("brightness"), new_brightness.to_string()) {
                    Ok(()) => {
                        brightness.actual_brightness = new_brightness;
                        let _ = tx.send_blocking(BrightnessState::Present(brightness.percent()));
                    },
                    Err(err) => {
                        crate::log::warn("brightness", &format!("failed to set brightness: {err}"));
                    },
                }
            }
        });
    }
}

#[cfg(feature = "renderer-gtk")]
impl GtkView for Brightness {
    fn gtk_box() -> GtkBox {
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

        container
    }

    fn gtk_threads(&self, container: &GtkBox) {
        let Some(icon) = find_widget::<Label>(container, "icon") else {
            return;
        };
        let Some(value) = find_widget::<Label>(container, "value") else {
            return;
        };
        container.set_visible(self.device.is_some());
        let container = container.clone();
        let rx = self.poll_rx.clone();
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
        let scroll_tx = self.cmd_tx.clone();
        scroll.connect_scroll(move |_, _dx, dy| {
            let step_as_i8 =
                i8::try_from(CONFIG.behaviour.on_scroll_brightness_step.min(127)).unwrap_or(0);

            let _ = scroll_tx.send(Cmd::SetRelative(
                if dy < 0.0 {
                    step_as_i8
                } else {
                    step_as_i8.saturating_neg()
                },
            ));

            glib::Propagation::Stop
        });
        container.add_controller(scroll);

        let click = GestureClick::new();
        let click_tx = self.cmd_tx.clone();
        click.connect_released(move |_, _, _, _| {
            let _ = click_tx.send(Cmd::SetAbsolute(1));
        });
        container.add_controller(click);
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
struct Backlight {
    actual_brightness: u64,
    max_brightness: u64,
}

impl Backlight {
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
        super::percent(self.actual_brightness, self.max_brightness)
    }

    fn apply(self, cmd: &Cmd) -> u64 {
        match cmd {
            Cmd::SetAbsolute(absolute) => (*absolute).clamp(0, self.max_brightness),
            Cmd::SetRelative(relative) => {
                if *relative > 0 {
                    self.max_brightness.min(
                        self.actual_brightness.saturating_add(
                            u64::try_from(*relative)
                                .unwrap_or(0)
                                .saturating_mul(self.max_brightness)
                                .saturating_div(100)
                                .max(1),
                        ),
                    )
                } else {
                    self.actual_brightness.saturating_sub(
                        u64::try_from(relative.saturating_neg())
                            .unwrap_or(0)
                            .saturating_mul(self.max_brightness)
                            .saturating_div(100)
                            .max(1),
                    )
                }
            },
        }
    }
}



enum Cmd {
    /// Set the value to in actual_brightness
    SetAbsolute(u64),
    /// Increase / Decrease by, in percentage
    SetRelative(i8),
}

const fn icon_for(percent: u8) -> &'static str {
    match percent {
        0..=1 => "",
        2..=33 => "󰃞",
        34..=66 => "󰃟",
        _ => "󰃠",
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    const fn b(actual: u64, max: u64) -> Backlight {
        Backlight {
            actual_brightness: actual,
            max_brightness: max,
        }
    }

    #[test]
    fn relative_clamps_to_max_and_zero() {
        assert_eq!(b(99, 100).apply(&Cmd::SetRelative(3)), 100);
        assert_eq!(b(1, 100).apply(&Cmd::SetRelative(-3)), 0);
    }

    #[test]
    fn relative_moves_at_least_one_unit_on_coarse_backlights() {
        assert_eq!(b(3, 7).apply(&Cmd::SetRelative(3)), 4);
        assert_eq!(b(3, 7).apply(&Cmd::SetRelative(-3)), 2);
    }

    #[test]
    fn relative_step_is_percent_of_max() {
        assert_eq!(b(500, 1000).apply(&Cmd::SetRelative(10)), 600);
        assert_eq!(b(500, 1000).apply(&Cmd::SetRelative(-10)), 400);
    }
}
