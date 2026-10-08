use serde::{Deserialize, Serialize};
use std::str::FromStr;

use gpui::{WindowDecorations, WindowOptions, px};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::Display)]
#[cfg_attr(not(target_os = "linux"), derive(Default))]
pub enum TitleBarMode {
    #[cfg_attr(not(target_os = "linux"), default)]
    Compact,
    Hide,
    System,
}

impl FromStr for TitleBarMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s.to_ascii_lowercase().as_str() {
            "compact" => Self::Compact,
            "hide" | "hidden" | "none" => Self::Hide,
            "system" | "native" => Self::System,
            _ => return Err(()),
        };
        Ok(val)
    }
}

impl<'de> Deserialize<'de> for TitleBarMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse()
            .map_err(|_| serde::de::Error::custom(format!("unknown title bar mode: {s}")))
    }
}

impl Serialize for TitleBarMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Self::Compact => "compact",
            Self::Hide => "hide",
            Self::System => "system",
        })
    }
}

impl TitleBarMode {
    pub fn apply_to_window_options(self, options: &mut WindowOptions) {
        match self {
            Self::System => {
                options.window_decorations = Some(WindowDecorations::Server);
                if let Some(titlebar) = options.titlebar.as_mut() {
                    titlebar.appears_transparent = false;
                }
            }
            Self::Compact | Self::Hide => {
                options.window_decorations = Some(WindowDecorations::Client);
                if let Some(titlebar) = options.titlebar.as_mut() {
                    titlebar.appears_transparent = true;
                    titlebar.traffic_light_position = Some(gpui::point(px(9.), px(9.)));
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
impl Default for TitleBarMode {
    fn default() -> Self {
        use crate::desktop::LinuxDesktop;
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let Ok(desktop) = desktop.parse::<LinuxDesktop>() else {
            return Self::Compact;
        };
        match desktop {
            LinuxDesktop::niri | LinuxDesktop::Hyprland => Self::Hide,
            LinuxDesktop::GNOME => Self::Compact,
            _ => Self::System,
        }
    }
}
