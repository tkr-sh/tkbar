#[cfg(feature = "cli")]
pub use conf::CLI_CONFIG_PATH;
pub use conf::Config;

mod conf;
mod log;
mod ui;

pub fn run() {
    #[cfg(feature = "renderer-gtk")]
    ui::gtk::run();

    #[cfg(feature = "renderer-wayland")]
    ui::wl::run();
}

#[cfg(all(feature = "renderer-gtk", feature = "renderer-wayland"))]
compile_error!(
    "features `renderer-gtk` and `renderer-wayland` are mutually exclusive; enable exactly one"
);

#[cfg(not(any(feature = "renderer-gtk", feature = "renderer-wayland")))]
compile_error!("enable exactly one renderer: `renderer-gtk` or `renderer-wayland`");

#[cfg(feature = "renderer-gtk")]
const fn color_feature_count() -> usize {
    let mut n = 0;
    if cfg!(feature = "black") {
        n += 1;
    }
    if cfg!(feature = "blue") {
        n += 1;
    }
    if cfg!(feature = "cyan") {
        n += 1;
    }
    if cfg!(feature = "green") {
        n += 1;
    }
    if cfg!(feature = "orange") {
        n += 1;
    }
    if cfg!(feature = "pink") {
        n += 1;
    }
    if cfg!(feature = "purple") {
        n += 1;
    }
    if cfg!(feature = "red") {
        n += 1;
    }
    if cfg!(feature = "white") {
        n += 1;
    }
    if cfg!(feature = "yellow") {
        n += 1;
    }
    n
}

#[cfg(feature = "renderer-gtk")]
const _: () = assert!(
    color_feature_count() == 1,
    "exactly one color feature must be enabled: black, blue, cyan, green, orange, pink, purple, red, white, yellow"
);

const fn workspace_backend_count() -> usize {
    let mut n = 0;
    if cfg!(feature = "niri") {
        n += 1;
    }
    if cfg!(feature = "hyprland") {
        n += 1;
    }
    if cfg!(feature = "sway") {
        n += 1;
    }
    n
}

const _: () = assert!(
    workspace_backend_count() <= 1,
    "zero or one of the workspace backends must be enabled: niri, hyprland, sway"
);
