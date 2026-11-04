use crate::{App, Asset, Global, ImageCacheError, RenderImage, Task, hash};
use futures::future::Shared;
use std::{any::TypeId, sync::Arc};

/// Retention budget for unused static images loaded by `img` without a custom cache.
/// Install as an application global. Visible scenes and external image owners are
/// protected even when they exceed the budget. Animations and custom caches are excluded
/// from this budget. Unused entries are checked after drawing. Closing the last window
/// also detaches all managed animation entries; external owners and decode tasks remain valid.
#[derive(Clone, Copy, Debug)]
pub struct ImageCacheLimits {
    /// Maximum decoded pixel bytes retained for unused images. Defaults to 32 MiB.
    pub max_unused_bytes: usize,
    /// Maximum completed unused entries, including errors. Defaults to 128.
    pub max_unused_entries: usize,
}

impl Default for ImageCacheLimits {
    fn default() -> Self {
        Self {
            max_unused_bytes: 32 * 1024 * 1024,
            max_unused_entries: 128,
        }
    }
}

impl Global for ImageCacheLimits {}

type ImageTask = Shared<Task<Result<Arc<RenderImage>, ImageCacheError>>>;

impl App {
    pub(crate) fn track_image_asset<A: Asset>(&mut self, source: &A::Source) {
        self.image_asset_clock = self.image_asset_clock.saturating_add(1);
        self.image_asset_access
            .insert((TypeId::of::<A>(), hash(source)), self.image_asset_clock);
    }

    pub(crate) fn prune_image_assets(&mut self, discard_unused: bool) {
        let limits = self
            .try_global::<ImageCacheLimits>()
            .copied()
            .unwrap_or_default();
        let mut unused = Vec::new();
        let mut bytes = 0usize;
        for (key, accessed) in &self.image_asset_access {
            let Some(task) = self
                .loading_assets
                .get(key)
                .and_then(|task| task.downcast_ref::<ImageTask>())
            else {
                continue;
            };
            let Some(result) = task.peek() else {
                if discard_unused {
                    unused.push((*key, *accessed, 0));
                }
                continue;
            };
            let size = match result {
                Ok(image) => {
                    if image.animation().is_some() {
                        if discard_unused {
                            unused.push((*key, *accessed, 0));
                        }
                        continue;
                    }
                    if Arc::strong_count(image) > 1
                        || image
                            .atlas_lifetime
                            .as_ref()
                            .is_none_or(|lifetime| Arc::strong_count(lifetime) > 1)
                    {
                        continue;
                    }
                    (0..image.frame_count())
                        .filter_map(|frame| image.as_bytes(frame))
                        .map(<[u8]>::len)
                        .sum()
                }
                Err(_) => 0,
            };
            bytes = bytes.saturating_add(size);
            unused.push((*key, *accessed, size));
        }
        let mut count = unused.len();
        if !discard_unused && bytes <= limits.max_unused_bytes && count <= limits.max_unused_entries
        {
            return;
        }
        unused.sort_unstable_by_key(|(_, accessed, _)| *accessed);
        for (key, _, size) in unused {
            if !discard_unused
                && bytes <= limits.max_unused_bytes
                && count <= limits.max_unused_entries
            {
                break;
            }
            self.loading_assets.remove(&key);
            self.image_asset_access.remove(&key);
            bytes = bytes.saturating_sub(size);
            count -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppContext as _, ImgResourceLoader, Resource};
    use futures::FutureExt;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::{io::Cursor, path::PathBuf, sync::Weak};

    fn insert_image(cx: &mut App, name: &str) -> (Resource, Weak<RenderImage>) {
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::new(7, 11))
            .write_to(&mut encoded, ImageFormat::Png)
            .unwrap();
        let image = super::super::load_image(
            encoded.get_ref(),
            Some(ImageFormat::Png),
            &cx.svg_renderer(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        cache_image(cx, name, image)
    }

    fn cache_image(
        cx: &mut App,
        name: &str,
        image: Arc<RenderImage>,
    ) -> (Resource, Weak<RenderImage>) {
        let weak = Arc::downgrade(&image);
        let source = Resource::from(PathBuf::from(name));
        let task: ImageTask = Task::ready(Ok(image)).shared();
        drop(task.clone().now_or_never().unwrap());
        cx.loading_assets.insert(
            (TypeId::of::<ImgResourceLoader>(), hash(&source)),
            Box::new(task),
        );
        cx.track_image_asset::<ImgResourceLoader>(&source);
        (source, weak)
    }

    fn insert_animation(cx: &mut App) -> (Resource, Weak<RenderImage>) {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            encoder
                .set_repeat(image::codecs::gif::Repeat::Infinite)
                .unwrap();
            for index in 0..3 {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        RgbaImage::from_pixel(9, 5, image::Rgba([20 + index * 5, 70, 140, 255])),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(80, 1),
                    ))
                    .unwrap();
            }
        }
        let image = super::super::load_image(
            &bytes,
            Some(ImageFormat::Gif),
            &cx.svg_renderer(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        cache_image(cx, "animated-cache.gif", image)
    }

    struct AnimationView(Resource);

    impl crate::Render for AnimationView {
        fn render(
            &mut self,
            _: &mut crate::Window,
            _: &mut crate::Context<Self>,
        ) -> impl crate::IntoElement {
            use crate::{InteractiveElement as _, Styled as _};
            crate::img(crate::ImageSource::Resource(self.0.clone()))
                .id("animation")
                .w(crate::px(36.))
                .h(crate::px(20.))
        }
    }

    #[crate::test]
    fn animated_images_release_only_after_the_last_window_closes(cx: &mut crate::TestAppContext) {
        let (resource, image) = cx.update(|cx| {
            cx.set_global(ImageCacheLimits {
                max_unused_bytes: 0,
                max_unused_entries: 0,
            });
            insert_animation(cx)
        });
        let animation = Arc::downgrade(image.upgrade().unwrap().animation().unwrap());
        let windows = [0, 1].map(|_| cx.add_window(|_, _| AnimationView(resource.clone())));
        for window in windows {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
                .unwrap();
        }
        windows[0]
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert!(image.upgrade().is_some());
        assert!(animation.upgrade().is_some());
        windows[1]
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert!(image.upgrade().is_none());
        assert!(animation.upgrade().is_none());
    }

    #[crate::test(iterations = 20)]
    fn closing_windows_preserves_external_animation_owners_and_decode_tasks(
        cx: &mut crate::TestAppContext,
    ) {
        let (resource, weak) = cx.update(insert_animation);
        let image = weak.upgrade().unwrap();
        let source = image.animation().unwrap().clone();
        let weak_source = Arc::downgrade(&source);
        let frame = source.frame(2, &cx.background_executor);
        drop(source);
        let window = cx.add_window(|_, _| AnimationView(resource.clone()));
        window
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.update(|cx| {
            assert!(
                !cx.loading_assets
                    .contains_key(&(TypeId::of::<ImgResourceLoader>(), hash(&resource)))
            );
        });
        assert!(weak.upgrade().is_some());
        drop(image);
        cx.run_until_parked();
        let frame = frame
            .now_or_never()
            .expect("decode finishes after cache detachment")
            .unwrap();
        assert!(
            frame
                .as_bytes(0)
                .unwrap()
                .chunks_exact(4)
                .all(|pixel| pixel == [140, 70, 30, 255])
        );
        assert!(weak.upgrade().is_none());
        assert!(weak_source.upgrade().is_none());
    }

    #[crate::test]
    fn unused_image_budget_evicts_least_recently_used_pixels(cx: &mut App) {
        cx.set_global(ImageCacheLimits {
            max_unused_bytes: 2 * 7 * 11 * 4,
            max_unused_entries: 8,
        });
        let (old_source, old) = insert_image(cx, "old.png");
        let (_, middle) = insert_image(cx, "middle.png");
        let (_, newest) = insert_image(cx, "newest.png");
        cx.track_image_asset::<ImgResourceLoader>(&old_source);
        cx.prune_image_assets(false);
        assert!(old.upgrade().is_some());
        assert!(middle.upgrade().is_none());
        assert!(newest.upgrade().is_some());
        cx.prune_image_assets(true);
        assert!(old.upgrade().is_none());
        assert!(newest.upgrade().is_none());
        assert!(cx.image_asset_access.is_empty());
    }

    #[crate::test]
    fn image_cache_protects_recorded_scenes_and_external_owners(cx: &mut App) {
        cx.set_global(ImageCacheLimits {
            max_unused_bytes: 0,
            max_unused_entries: 0,
        });
        let (_, weak) = insert_image(cx, "protected.png");
        let image = weak.upgrade().unwrap();
        let mut original = crate::Scene::default();
        original.retain_atlas_tile(image.atlas_lifetime.clone().unwrap());
        let mut replay = crate::Scene::default();
        replay.replay(0..original.len(), &original);
        drop(original);
        drop(image);
        cx.prune_image_assets(false);
        assert!(
            weak.upgrade().is_some(),
            "cached scene replay must keep the decoded image available"
        );
        let external = weak.upgrade().unwrap();
        drop(replay);
        cx.prune_image_assets(false);
        assert!(
            weak.upgrade().is_some(),
            "external users must not lose their cache entry"
        );
        drop(external);
        cx.prune_image_assets(false);
        assert!(weak.upgrade().is_none());
    }

    #[crate::test]
    fn closing_windows_discards_pending_managed_images_only(cx: &mut App) {
        let pending = Resource::from(PathBuf::from("pending.png"));
        let task: ImageTask = cx
            .background_executor()
            .spawn(futures::future::pending())
            .shared();
        let key = (TypeId::of::<ImgResourceLoader>(), hash(&pending));
        cx.loading_assets.insert(key, Box::new(task));
        cx.track_image_asset::<ImgResourceLoader>(&pending);
        let (unmanaged, weak) = insert_image(cx, "unmanaged.png");
        cx.image_asset_access
            .remove(&(TypeId::of::<ImgResourceLoader>(), hash(&unmanaged)));
        cx.prune_image_assets(false);
        assert!(cx.loading_assets.contains_key(&key));
        cx.prune_image_assets(true);
        assert!(!cx.loading_assets.contains_key(&key));
        assert!(weak.upgrade().is_some());
    }

    #[crate::test]
    fn unused_image_entry_budget_includes_failures(cx: &mut App) {
        cx.set_global(ImageCacheLimits {
            max_unused_bytes: usize::MAX,
            max_unused_entries: 1,
        });
        let (source, weak) = insert_image(cx, "earlier.png");
        let failed = Resource::from(PathBuf::from("failed.png"));
        let task: ImageTask = Task::ready(Err(anyhow::anyhow!("decode failed").into())).shared();
        drop(task.clone().now_or_never().unwrap());
        cx.loading_assets.insert(
            (TypeId::of::<ImgResourceLoader>(), hash(&failed)),
            Box::new(task),
        );
        cx.track_image_asset::<ImgResourceLoader>(&failed);
        cx.prune_image_assets(false);
        assert!(weak.upgrade().is_none());
        assert!(
            !cx.loading_assets
                .contains_key(&(TypeId::of::<ImgResourceLoader>(), hash(&source)))
        );
        cx.remove_asset::<ImgResourceLoader>(&failed);
        assert!(cx.image_asset_access.is_empty());
    }

    #[crate::test]
    fn default_image_cache_survives_cached_views_and_shared_windows(
        cx: &mut crate::TestAppContext,
    ) {
        use crate::{
            Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window, div, img,
        };
        use std::{cell::Cell, rc::Rc};

        struct Content {
            source: Resource,
            renders: Rc<Cell<usize>>,
        }
        impl Render for Content {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.renders.set(self.renders.get() + 1);
                img(crate::ImageSource::Resource(self.source.clone()))
                    .w(crate::px(24.))
                    .h(crate::px(24.))
            }
        }
        struct Root {
            content: Option<Entity<Content>>,
        }
        impl Render for Root {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().children(
                    self.content
                        .iter()
                        .map(|content| content.clone().cached(div().size_full().style().clone())),
                )
            }
        }
        let (source, weak) = cx.update(|cx| {
            cx.set_global(ImageCacheLimits {
                max_unused_bytes: 0,
                max_unused_entries: 0,
            });
            insert_image(cx, "shared-view.png")
        });
        let renders = Rc::new(Cell::new(0));
        let windows = [0, 1].map(|_| {
            cx.add_window(|_, cx| Root {
                content: Some(cx.new(|_| Content {
                    source: source.clone(),
                    renders: renders.clone(),
                })),
            })
        });
        for window in windows {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
                .unwrap();
        }
        let rendered = renders.get();
        for window in windows {
            window.update(cx, |_, _, cx| cx.notify()).unwrap();
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
                .unwrap();
        }
        assert_eq!(renders.get(), rendered, "the child paint must be replayed");
        windows[0]
            .update(cx, |root, _, cx| {
                root.content = None;
                cx.notify();
            })
            .unwrap();
        cx.update_window(windows[0].into(), |_, window, cx| window.draw(cx).clear())
            .unwrap();
        assert!(
            weak.upgrade().is_some(),
            "another window still displays this image"
        );
        windows[1]
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        assert!(
            weak.upgrade().is_none(),
            "the last displaying window has released the image"
        );
    }
}
