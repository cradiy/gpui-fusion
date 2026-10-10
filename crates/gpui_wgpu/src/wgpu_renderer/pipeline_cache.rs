use super::{WgpuBindGroupLayouts, WgpuPipelines, WgpuRenderer};
use crate::WgpuContext;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};

mod effects;
pub(super) use effects::EffectPipelineKind;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    format: wgpu::TextureFormat,
    alpha: wgpu::CompositeAlphaMode,
    samples: u32,
    dual_source: bool,
}

const RETAINED_PIPELINE_CAPACITY: usize = 4;

/// Device-local layouts and bounded strong caches for base and custom pipelines.
/// Weak entries share evicted configurations while renderers still use them.
#[derive(Default)]
pub(crate) struct PipelineCache {
    device: Weak<wgpu::Device>,
    layouts: Option<Arc<WgpuBindGroupLayouts>>,
    pipelines: HashMap<PipelineKey, Weak<WgpuPipelines>>,
    retained: VecDeque<(PipelineKey, Arc<WgpuPipelines>)>,
    effects: effects::EffectPipelineCache,
}

impl PipelineCache {
    fn set_device(&mut self, device: &Arc<wgpu::Device>) {
        let device = Arc::downgrade(device);
        if !self.device.ptr_eq(&device) {
            *self = Self {
                device,
                ..Default::default()
            };
        }
    }
}

impl WgpuRenderer {
    pub(super) fn shared_pipelines(
        context: &WgpuContext,
        format: wgpu::TextureFormat,
        alpha: wgpu::CompositeAlphaMode,
        samples: u32,
        dual_source: bool,
    ) -> (Arc<WgpuBindGroupLayouts>, Arc<WgpuPipelines>) {
        let mut cache = context.pipeline_cache.lock().unwrap();
        cache.set_device(&context.device);
        let layouts = cache.layouts.clone().unwrap_or_else(|| {
            let layouts = Arc::new(Self::create_bind_group_layouts(&context.device));
            cache.layouts = Some(layouts.clone());
            layouts
        });
        let key = PipelineKey {
            format,
            alpha,
            samples,
            dual_source,
        };
        let pipelines = cache
            .pipelines
            .get(&key)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let pipelines = Arc::new(Self::create_pipelines(
                    &context.device,
                    &layouts,
                    format,
                    alpha,
                    samples,
                    dual_source,
                ));
                cache.pipelines.insert(key, Arc::downgrade(&pipelines));
                pipelines
            });
        if let Some(index) = cache.retained.iter().position(|(cached, _)| *cached == key) {
            cache.retained.remove(index);
        } else if cache.retained.len() == RETAINED_PIPELINE_CAPACITY {
            cache.retained.pop_front();
        }
        cache.retained.push_back((key, pipelines.clone()));
        cache
            .pipelines
            .retain(|_, pipeline| pipeline.strong_count() > 0);
        (layouts, pipelines)
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn pipelines_survive_rebuilds_with_bounded_retention_and_device_isolation() -> anyhow::Result<()>
    {
        let context = WgpuContext::new_headless()?;
        let create = |context: &WgpuContext, variant: usize| {
            WgpuRenderer::shared_pipelines(
                context,
                [
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureFormat::Bgra8Unorm,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                ][variant / 2],
                if variant.is_multiple_of(2) {
                    wgpu::CompositeAlphaMode::Opaque
                } else {
                    wgpu::CompositeAlphaMode::PreMultiplied
                },
                1,
                false,
            )
        };
        let (layouts, opaque) = create(&context, 0);
        let weak_layouts = Arc::downgrade(&layouts);
        let weak_opaque = Arc::downgrade(&opaque);
        drop((layouts, opaque));
        let cloned_context = context.clone();
        let (layouts, same) = create(&cloned_context, 0);
        assert!(weak_layouts.ptr_eq(&Arc::downgrade(&layouts)));
        assert!(weak_opaque.ptr_eq(&Arc::downgrade(&same)));
        drop((layouts, same, cloned_context));

        let (_, active) = create(&context, 1);
        assert!(!weak_opaque.ptr_eq(&Arc::downgrade(&active)));
        let (_, third) = create(&context, 2);
        let weak_third = Arc::downgrade(&third);
        drop(third);
        drop(create(&context, 3));
        drop(create(&context, 0));
        drop(create(&context, 4));
        assert!(
            weak_opaque.upgrade().is_some(),
            "cache hits refresh recency"
        );

        let (_, shared) = create(&context, 1);
        assert!(
            Arc::ptr_eq(&active, &shared),
            "live evicted pipelines remain shareable"
        );
        drop(shared);
        assert!(
            weak_third.upgrade().is_none(),
            "unused LRU entries are released"
        );
        drop(create(&context, 5));
        drop(create(&context, 2));
        assert!(weak_opaque.upgrade().is_none());
        let weak_active = Arc::downgrade(&active);
        drop(active);

        let mut other = WgpuContext::new_headless()?;
        let (other_layouts, other_pipeline) = create(&other, 1);
        assert!(!weak_layouts.ptr_eq(&Arc::downgrade(&other_layouts)));
        assert!(!weak_active.ptr_eq(&Arc::downgrade(&other_pipeline)));
        drop((other_layouts, other_pipeline));

        // Reusing a cache with a replacement device discards the old device's entries.
        other.pipeline_cache = context.pipeline_cache.clone();
        let (replacement_layouts, replacement) = create(&other, 1);
        assert!(weak_active.upgrade().is_none());
        assert!(weak_layouts.upgrade().is_none());
        let weak_replacement = Arc::downgrade(&replacement);
        let weak_replacement_layouts = Arc::downgrade(&replacement_layouts);
        drop((replacement, replacement_layouts, context, other));
        assert!(weak_replacement.upgrade().is_none());
        assert!(weak_replacement_layouts.upgrade().is_none());
        Ok(())
    }
}
