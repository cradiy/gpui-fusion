use crate::{AtlasKey, DevicePixels, PlatformAtlas, Size};
use collections::FxHashMap;
use std::sync::Arc;

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 1024;

struct Entry {
    lifetime: Arc<()>,
    bytes: usize,
    last_used: u64,
}

#[derive(Default)]
pub(super) struct SvgCache {
    entries: FxHashMap<AtlasKey, Entry>,
    bytes: usize,
    clock: u64,
}

impl SvgCache {
    pub(super) fn retain(&mut self, key: &AtlasKey, size: Size<DevicePixels>) -> Arc<()> {
        let channels = match key {
            AtlasKey::Svg(_) => 1,
            AtlasKey::ColorSvg(_) => 4,
            _ => panic!("SVG cache requires an SVG atlas key"),
        };
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.entry(key.clone()).or_insert_with(|| {
            let bytes = size.width.0.max(0) as usize * size.height.0.max(0) as usize * channels;
            self.bytes += bytes;
            Entry {
                lifetime: Arc::new(()),
                bytes,
                last_used: self.clock,
            }
        });
        entry.last_used = self.clock;
        entry.lifetime.clone()
    }

    pub(super) fn collect(&mut self, atlas: &dyn PlatformAtlas) {
        if self.bytes <= MAX_BYTES && self.entries.len() <= MAX_ENTRIES {
            return;
        }
        let mut unused: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| Arc::strong_count(&entry.lifetime) == 1)
            .map(|(key, entry)| (entry.last_used, key.clone()))
            .collect();
        unused.sort_unstable_by_key(|(used, _)| *used);
        for (_, key) in unused {
            if self.bytes <= MAX_BYTES / 2 && self.entries.len() <= MAX_ENTRIES / 2 {
                break;
            }
            let entry = self.entries.remove(&key).unwrap();
            self.bytes -= entry.bytes;
            atlas.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ObjectFit, RenderColorSvgParams, RenderSvgParams, Scene, platform::TestAtlas, px, size,
    };
    use std::borrow::Cow;

    fn key(index: usize, colored: bool, side: i32) -> AtlasKey {
        let path = format!("vector-{index}.svg").into();
        let size = size(DevicePixels(side), DevicePixels(side));
        if colored {
            RenderColorSvgParams {
                path,
                size,
                logical_size: size.map(|value| px(value.0 as f32)),
                object_fit: ObjectFit::Contain,
                current_color: None,
                fill_color: None,
                text_color: None,
            }
            .into()
        } else {
            RenderSvgParams { path, size }.into()
        }
    }

    fn upload(atlas: &TestAtlas, key: &AtlasKey, side: i32) -> Arc<()> {
        let size = size(DevicePixels(side), DevicePixels(side));
        atlas
            .get_or_insert_with(key, &mut || Ok(Some((size, Cow::Borrowed(&[])))))
            .unwrap();
        atlas.retain_svg(key, size)
    }

    fn cached(atlas: &TestAtlas, key: &AtlasKey) -> bool {
        atlas
            .get_or_insert_with(key, &mut || Ok(None))
            .unwrap()
            .is_some()
    }

    #[test]
    fn svg_budget_preserves_shared_and_replayed_scenes() {
        let atlas = TestAtlas::new();
        let key = key(0, true, 2304);
        let mut first = Scene::default();
        let mut second = Scene::default();
        first.retain_atlas_tile(upload(&atlas, &key, 2304));
        second.retain_atlas_tile(
            atlas.retain_svg(&key, size(DevicePixels(2304), DevicePixels(2304))),
        );
        let mut replay = Scene::default();
        replay.replay(0..first.len(), &first);
        first.clear();
        atlas.collect_unused_images();
        assert!(cached(&atlas, &key));
        second.clear();
        atlas.collect_unused_images();
        assert!(cached(&atlas, &key));
        replay.clear();
        atlas.collect_unused_images();
        assert!(!cached(&atlas, &key));
    }

    #[test]
    fn svg_budget_accounts_for_color_and_evicts_oldest_unused_tiles() {
        for colored in [false, true] {
            let atlas = TestAtlas::new();
            let channels = if colored { 4 } else { 1 };
            let count = MAX_BYTES / (1024 * 1024 * channels);
            for index in 0..count {
                drop(upload(&atlas, &key(index, colored, 1024), 1024));
            }
            atlas.collect_unused_images();
            assert!(cached(&atlas, &key(0, colored, 1024)));
            drop(atlas.retain_svg(
                &key(0, colored, 1024),
                size(DevicePixels(1024), DevicePixels(1024)),
            ));
            drop(upload(&atlas, &key(count, colored, 1024), 1024));
            atlas.collect_unused_images();
            assert!(cached(&atlas, &key(0, colored, 1024)));
            assert!(!cached(&atlas, &key(1, colored, 1024)));
            assert!(cached(&atlas, &key(count, colored, 1024)));
        }
    }
}
