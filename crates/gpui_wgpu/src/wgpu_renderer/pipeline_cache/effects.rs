use super::WgpuRenderer;
use gpui::{BackdropShader, EffectShader};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};

const RETAINED_EFFECT_CAPACITY: usize = 32;

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in super::super) enum EffectPipelineKind {
    Quad,
    Subtree,
    SubtreeImages,
    Backdrop,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct EffectPipelineKey {
    shader: u64,
    format: wgpu::TextureFormat,
    alpha: wgpu::CompositeAlphaMode,
    kind: EffectPipelineKind,
}

#[derive(Default)]
pub(super) struct EffectPipelineCache {
    pipelines: HashMap<EffectPipelineKey, Weak<wgpu::RenderPipeline>>,
    retained: VecDeque<(EffectPipelineKey, Arc<wgpu::RenderPipeline>)>,
}

impl WgpuRenderer {
    pub(in super::super) fn shared_effect_pipeline(
        &self,
        format: wgpu::TextureFormat,
        shader: &EffectShader,
        kind: EffectPipelineKind,
    ) -> anyhow::Result<Arc<wgpu::RenderPipeline>> {
        let key = EffectPipelineKey {
            shader: shader.id().as_u64(),
            format,
            alpha: self.surface_config.alpha_mode,
            kind,
        };
        self.cached_effect_pipeline(key, || {
            Self::create_effect_pipeline(
                &self.resources().device,
                &self.resources().bind_group_layouts,
                format,
                key.alpha,
                shader,
                matches!(
                    kind,
                    EffectPipelineKind::Subtree | EffectPipelineKind::SubtreeImages
                ),
                kind == EffectPipelineKind::SubtreeImages,
            )
        })
    }

    pub(in super::super) fn shared_backdrop_effect_pipeline(
        &self,
        shader: &BackdropShader,
    ) -> anyhow::Result<Arc<wgpu::RenderPipeline>> {
        let key = EffectPipelineKey {
            shader: shader.id().as_u64(),
            format: self.surface_config.format,
            alpha: self.surface_config.alpha_mode,
            kind: EffectPipelineKind::Backdrop,
        };
        self.cached_effect_pipeline(key, || {
            Self::create_backdrop_effect_pipeline(
                &self.resources().device,
                &self.resources().bind_group_layouts,
                key.format,
                key.alpha,
                shader,
            )
        })
    }

    fn cached_effect_pipeline(
        &self,
        key: EffectPipelineKey,
        create: impl FnOnce() -> anyhow::Result<wgpu::RenderPipeline>,
    ) -> anyhow::Result<Arc<wgpu::RenderPipeline>> {
        let resources = self.resources();
        let mut cache = resources.capture_context.pipeline_cache.lock().unwrap();
        cache.set_device(&resources.device);
        cache
            .layouts
            .get_or_insert_with(|| resources.bind_group_layouts.clone());
        let cache = &mut cache.effects;
        let pipeline = match cache.pipelines.get(&key).and_then(Weak::upgrade) {
            Some(pipeline) => pipeline,
            None => {
                let pipeline = Arc::new(create()?);
                cache.pipelines.insert(key, Arc::downgrade(&pipeline));
                pipeline
            }
        };
        if let Some(index) = cache.retained.iter().position(|(cached, _)| *cached == key) {
            cache.retained.remove(index);
        } else if cache.retained.len() == RETAINED_EFFECT_CAPACITY {
            cache.retained.pop_front();
        }
        cache.retained.push_back((key, pipeline.clone()));
        cache
            .pipelines
            .retain(|_, pipeline| pipeline.strong_count() > 0);
        Ok(pipeline)
    }
}
