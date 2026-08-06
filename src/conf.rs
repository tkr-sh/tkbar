//! Configuration of the bar.
//!
//! Parsing configuration is a new attack surface and bloats the binary with extra
//! dependencies, so file-based configuration lives behind the `config` feature.
//!
//! The configuration file is optional: when it is missing, the hardcoded default
//! is used. An invalid file is a hard error: silently falling back would hide
//! typos from the user.

use crate::ui::{BarPosition, Component};

#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "config", serde(deny_unknown_fields))]
pub struct Config {
    #[cfg_attr(feature = "config", serde(default))]
    pub position: BarPosition,
    #[cfg_attr(feature = "config", serde(default = "default_bar_size_px"))]
    pub bar_size_px: usize,
    #[cfg_attr(feature = "config", serde(default = "default_components"))]
    pub components: Vec<Component>,
}

impl Config {
    fn hardcoded() -> Self {
        Config {
            position: BarPosition::default(),
            bar_size_px: default_bar_size_px(),
            components: default_components(),
        }
    }
}

pub static CONFIG: std::sync::LazyLock<Config> = std::sync::LazyLock::new(load);

#[cfg(feature = "config")]
fn load() -> Config {
    load_from(config_path().as_deref())
}

#[cfg(feature = "config")]
fn load_from(path: Option<&std::path::Path>) -> Config {
    let Some(path) = path else {
        crate::log::warn(
            "config",
            "could not determine the configuration directory, using defaults",
        );
        return Config::hardcoded();
    };

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Config::hardcoded(),
        Err(err) => {
            crate::log::warn(
                "config",
                &format!("cannot read {}: {err}, using defaults", path.display()),
            );
            return Config::hardcoded();
        },
    };

    match toml::from_str(&text) {
        Ok(config) => config,
        Err(err) => {
            crate::log::error("config", &format!("{}: {err}", path.display()));
            std::process::exit(1);
        },
    }
}

#[cfg(not(feature = "config"))]
fn load() -> Config {
    Config::hardcoded()
}

#[cfg(feature = "config")]
fn config_path() -> Option<std::path::PathBuf> {
    Some(
        directories::BaseDirs::new()?
            .config_dir()
            .join("tkbar")
            .join("config.toml"),
    )
}

const fn default_bar_size_px() -> usize {
    72
}

fn default_components() -> Vec<Component> {
    vec![
        Component::Logo('󱄅'),
        Component::Workspaces,
        Component::Spacer,
        Component::Battery,
        Component::Wifi,
        Component::Brightness,
        Component::Volume,
        Component::Clock,
    ]
}

#[cfg(all(test, feature = "config"))]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let config: Config = toml::from_str(
            r#"
            position = "top"
            bar_size_px = 90
            components = [
                { logo = "󱄅" },
                "workspaces",
                "spacer",
                "battery",
                "wifi",
                "brightness",
                "volume",
                "clock",
            ]
            "#,
        )
        .unwrap();

        assert_eq!(config.position, BarPosition::Top);
        assert_eq!(config.bar_size_px, 90);
        assert_eq!(config.components.len(), 8);
        assert!(matches!(
            config.components.first(),
            Some(Component::Logo('󱄅'))
        ));
    }

    #[test]
    fn empty_file_falls_back_to_field_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.position, BarPosition::Left);
        assert_eq!(config.bar_size_px, default_bar_size_px());
        assert_eq!(config.components.len(), default_components().len());
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(toml::from_str::<Config>("bogus = 1").is_err());
    }

    #[test]
    fn rejects_unknown_component() {
        assert!(toml::from_str::<Config>(r#"components = ["coffee"]"#).is_err());
    }

    #[test]
    fn rejects_unknown_position() {
        assert!(toml::from_str::<Config>(r#"position = "diagonal""#).is_err());
    }

    #[test]
    fn missing_file_falls_back_to_hardcoded() {
        let config = load_from(Some(std::path::Path::new("/nonexistent/tkbar.toml")));
        assert_eq!(config.position, BarPosition::Left);
        assert_eq!(config.bar_size_px, default_bar_size_px());
        assert_eq!(config.components.len(), default_components().len());
    }

    #[test]
    fn loads_partial_config_from_file() {
        let dir = std::env::temp_dir().join(format!("tkbar-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "bar_size_px = 42\n").unwrap();

        let config = load_from(Some(&path));
        assert_eq!(config.bar_size_px, 42);
        assert_eq!(config.components.len(), default_components().len());

        std::fs::remove_dir_all(dir).unwrap();
    }
}
