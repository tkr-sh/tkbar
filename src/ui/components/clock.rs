use {
    super::{ComponentWidget, find_widget},
    crate::conf::CONFIG,
    gtk::{Box as GtkBox, Label, Orientation, glib, prelude::*},
    gtk4 as gtk,
    std::fmt::Write as _,
};

pub(crate) struct Clock;

impl ComponentWidget for Clock {
    fn init() -> Self {
        Self
    }

    fn gtk_box() -> GtkBox {
        let container = GtkBox::new(CONFIG.style.position.orientation(), 0);

        let label = Label::new(None);
        label.add_css_class("clock");
        container.append(&label);

        container
    }

    fn gtk_threads(&self, container: &GtkBox) {
        let Some(label) = find_widget::<Label>(container, "clock") else {
            return;
        };

        let mut buf = String::with_capacity(8);
        update_clock(&label, &mut buf);

        glib::timeout_add_seconds_local(
            1,
            glib::clone!(
                #[weak]
                label,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    update_clock(&label, &mut buf);
                    glib::ControlFlow::Continue
                }
            ),
        );
    }

    fn threads(&mut self) {}
}


fn update_clock(label: &Label, buf: &mut String) {
    let (h, m, s) = get_hms_now();
    buf.clear();
    let c = if CONFIG.style.position.orientation() == Orientation::Horizontal {
        ' '
    } else {
        '\n'
    };
    let _ = write!(buf, "{h:02}{c}{m:02}{c}{s:02}",);
    label.set_text(buf);
}


fn get_hms_now() -> (u8, u8, u8) {
    let Ok(now) = glib::DateTime::now_local() else {
        return (0, 0, 0);
    };

    (
        u8::try_from(now.hour()).unwrap_or_default(),
        u8::try_from(now.minute()).unwrap_or_default(),
        u8::try_from(now.second()).unwrap_or_default(),
    )
}
