mod assets;
mod icons;

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub use gpui_icons_macros::{file_assets, file_enum};
pub use icons::GpuiIcons;

/// A single asset source for all enabled icon collections.
#[derive(Default, Clone, Copy)]
pub struct IconAssets;

impl IconAssets {
    pub fn new() -> Self {
        Self
    }
}

impl AssetSource for IconAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        assets::EmbeddedIcons::get(path)
            .map(|data| data.map(Cow::Borrowed))
            .map_err(anyhow::Error::msg)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(assets::EmbeddedIcons::iter()
            .filter(|asset| asset.starts_with(path))
            .map(Into::into)
            .collect())
    }
}
