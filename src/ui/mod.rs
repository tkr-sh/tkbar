mod components;

#[cfg(feature = "renderer-gtk")]
pub(crate) mod gtk;
#[cfg(feature = "renderer-wayland")]
pub(crate) mod wl;

pub(crate) use components::ComponentKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "doc", derive(serde::Serialize))]
#[cfg_attr(feature = "config", derive(serde::Deserialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
#[cfg_attr(
    not(feature = "config"),
    allow(
        dead_code,
        reason = "non-default positions are only reachable through the config feature"
    )
)]
pub enum BarPosition {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}
