use crate::{AtlasKey, DevicePixels, PlatformAtlas, RenderImageParams, Size};
use collections::FxHashMap;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

mod svg_cache;

/// Backend-side ownership tracking for transient images and bounded SVG tiles.
#[doc(hidden)]
#[derive(Default)]
pub struct AtlasImageLifetimes {
    images: Mutex<FxHashMap<RenderImageParams, Vec<Weak<()>>>>,
    svgs: Mutex<svg_cache::SvgCache>,
}

impl AtlasImageLifetimes {
    pub fn retain(&self, key: &RenderImageParams, lifetime: Weak<()>) {
        let mut entries = self.images.lock();
        let owners = entries.entry(key.clone()).or_default();
        owners.retain(|owner| owner.strong_count() > 0);
        if !owners.iter().any(|owner| owner.ptr_eq(&lifetime)) {
            owners.push(lifetime);
        }
    }

    pub fn retain_svg(&self, key: &AtlasKey, size: Size<DevicePixels>) -> Arc<()> {
        self.svgs.lock().retain(key, size)
    }

    pub fn collect(&self, atlas: &dyn PlatformAtlas) {
        self.svgs.lock().collect(atlas);
        self.images.lock().retain(|key, owners| {
            owners.retain(|owner| owner.strong_count() > 0);
            if owners.is_empty() {
                atlas.remove(&AtlasKey::TransientImage(key.clone()));
                false
            } else {
                true
            }
        });
    }
}
