use {
    crate::conf::CONFIG,
    gtk::{Label, Orientation, glib, prelude::*},
    gtk4 as gtk,
    std::fmt::Write as _,
};

pub(crate) fn clock() -> Label {
    let clock = Label::new(None);
    clock.add_css_class("clock");
    let mut buf = String::with_capacity(8);
    update_clock(&clock, &mut buf);

    glib::timeout_add_seconds_local(
        1,
        glib::clone!(
            #[weak]
            clock,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                update_clock(&clock, &mut buf);
                glib::ControlFlow::Continue
            }
        ),
    );

    clock
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
