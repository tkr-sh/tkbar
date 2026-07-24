use {
    gtk::{
        Box as GtkBox,
        EventControllerScroll,
        EventControllerScrollFlags,
        GestureClick,
        Label,
        Orientation,
        glib,
        prelude::*,
    },
    gtk4 as gtk,
    std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        thread,
    },
};



#[derive(Clone, Debug)]
struct VolState {
    percent: u32,
    muted: bool,
}

pub fn volume() -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 2);
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

    thread::spawn(move || {
        if let Some(s) = query() {
            let _ = tx.send_blocking(s);
        }

        let child = Command::new("pactl")
            .arg("subscribe")
            .stdout(Stdio::piped())
            .spawn();

        let Ok(mut child) = child else {
            eprintln!("volume: failed to spawn `pactl subscribe`");
            return;
        };
        let stdout = child.stdout.take().expect("piped stdout");

        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if (line.contains(" sink ") || line.contains(" server ")) &&
                let Some(s) = query()
            {
                let _ = tx.send_blocking(s);
            }
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
    click.connect_released(|_, _, _, _| {
        pactl(&["set-sink-mute", "@DEFAULT_SINK@", "toggle"]);
    });
    container.add_controller(click);

    let scroll = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    scroll.connect_scroll(|_, _dx, dy| {
        if dy < 0.0 {
            pactl(&["set-sink-volume", "@DEFAULT_SINK@", "+3%"]);
        } else {
            pactl(&["set-sink-volume", "@DEFAULT_SINK@", "-3%"]);
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
    let vol = Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .ok()?;
    let mute = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .ok()?;

    // "Volume: front-left: 39321 /  60% / -13.31 dB, front-right: ..."
    let vol_text = String::from_utf8_lossy(&vol.stdout);
    let percent = vol_text.split('/').find_map(|part| {
        part.trim()
            .strip_suffix('%')
            .and_then(|n| n.trim().parse::<u32>().ok())
    })?;

    let muted = String::from_utf8_lossy(&mute.stdout).contains("yes");

    Some(VolState { percent, muted })
}

fn pactl(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    thread::spawn(move || {
        let _ = Command::new("pactl").args(&args).status();
    });
}
