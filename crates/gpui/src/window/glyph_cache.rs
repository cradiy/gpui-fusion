use crate::{DevicePixels, PlatformAtlas, RenderGlyphParams, Size};
use collections::FxHashMap;
use std::sync::Arc;

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 32 * 1024;

struct Entry {
    lifetime: Arc<()>,
    bytes: usize,
    last_used: u64,
}

/// Cached glyph pixels are disposable only after every recorded scene releases them.
#[derive(Default)]
pub(super) struct GlyphCache {
    entries: FxHashMap<RenderGlyphParams, Entry>,
    bytes: usize,
    frame: u64,
}

impl GlyphCache {
    pub(super) fn retain(
        &mut self,
        params: &RenderGlyphParams,
        size: Size<DevicePixels>,
    ) -> Arc<()> {
        let entry = self.entries.entry(params.clone()).or_insert_with(|| {
            let bytes_per_pixel = if params.is_emoji || params.subpixel_rendering {
                4
            } else {
                1
            };
            let bytes =
                size.width.0.max(0) as usize * size.height.0.max(0) as usize * bytes_per_pixel;
            self.bytes += bytes;
            Entry {
                lifetime: Arc::new(()),
                bytes,
                last_used: self.frame,
            }
        });
        entry.last_used = self.frame;
        entry.lifetime.clone()
    }

    pub(super) fn finish_frame(&mut self, atlas: &dyn PlatformAtlas) {
        self.frame += 1;
        if self.bytes <= MAX_BYTES && self.entries.len() <= MAX_ENTRIES {
            return;
        }
        let mut unused: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| Arc::strong_count(&entry.lifetime) == 1)
            .map(|(key, entry)| (entry.last_used, key.clone()))
            .collect();
        unused.sort_unstable_by_key(|(frame, _)| *frame);
        for (_, key) in unused {
            if self.bytes <= MAX_BYTES / 2 && self.entries.len() <= MAX_ENTRIES / 2 {
                break;
            }
            let entry = self.entries.remove(&key).unwrap();
            self.bytes -= entry.bytes;
            atlas.remove(&key.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontId, GlyphId, Scene, platform::TestAtlas, px, size};
    use std::borrow::Cow;

    #[test]
    fn eviction_preserves_replayed_glyphs_until_the_scene_is_released() {
        let atlas = TestAtlas::new();
        let mut cache = GlyphCache::default();
        let mut recorded = Scene::default();
        let mut replayed = Scene::default();
        let params = RenderGlyphParams {
            font_id: FontId(0),
            glyph_id: GlyphId(1),
            font_size: px(48.),
            subpixel_variant: Default::default(),
            scale_factor: 1.,
            is_emoji: true,
            subpixel_rendering: false,
            dilation: 0,
            blur_radius: 0,
        };
        let tile_size = size(DevicePixels(1024), DevicePixels(1024));
        for index in 0..24 {
            let key = RenderGlyphParams {
                font_size: px(48. + index as f32),
                ..params.clone()
            };
            atlas
                .get_or_insert_with(&key.clone().into(), &mut || {
                    Ok(Some((tile_size, Cow::Borrowed(&[]))))
                })
                .unwrap();
            let lifetime = cache.retain(&key, tile_size);
            if index == 0 {
                recorded.retain_atlas_tile(lifetime.clone());
                replayed.replay(0..recorded.len(), &recorded);
                recorded.clear();
            }
            drop(lifetime);
            cache.finish_frame(&atlas);
            assert!(cache.bytes <= MAX_BYTES);
            assert!(
                atlas
                    .get_or_insert_with(&params.clone().into(), &mut || Ok(None))
                    .unwrap()
                    .is_some()
            );
        }
        replayed.clear();
        for index in 24..30 {
            let key = RenderGlyphParams {
                font_size: px(48. + index as f32),
                ..params.clone()
            };
            drop(cache.retain(&key, tile_size));
            cache.finish_frame(&atlas);
        }
        assert!(
            atlas
                .get_or_insert_with(&params.into(), &mut || Ok(None))
                .unwrap()
                .is_none()
        );
    }
}
