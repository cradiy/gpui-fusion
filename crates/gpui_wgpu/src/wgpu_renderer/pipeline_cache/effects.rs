use super::{WgpuBindGroupLayouts, WgpuRenderer};
use crate::WgpuContext;
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
    /// Captures preparation work for this renderer's current device, format and alpha mode.
    /// The returned closure may run on a worker thread and populates the normal bounded cache.
    #[cfg(not(target_family = "wasm"))]
    pub fn prepare_effect_pipelines(
        &self,
        quads: Vec<EffectShader>,
        subtrees: Vec<EffectShader>,
    ) -> Option<Box<dyn FnOnce() -> anyhow::Result<()> + Send>> {
        let resources = self.resources.as_ref()?;
        if self.device_lost() {
            return None;
        }
        let context = resources.capture_context.clone();
        let layouts = resources.bind_group_layouts.clone();
        let format = self.surface_config.format;
        let alpha = self.surface_config.alpha_mode;
        let samples = self.rendering_params.path_sample_count;
        let dual_source = self.dual_source_blending;
        Some(Box::new(move || {
            // Isolated raster captures render into a premultiplied target.
            // Their base pipelines must be ready before the first capture too.
            let (_, pipelines) = Self::shared_pipelines(
                &context,
                format,
                wgpu::CompositeAlphaMode::PreMultiplied,
                samples,
                dual_source,
            );
            pipelines.prepare();
            let mut alphas = vec![alpha];
            if alpha != wgpu::CompositeAlphaMode::PreMultiplied {
                alphas.push(wgpu::CompositeAlphaMode::PreMultiplied);
            }
            for alpha in alphas {
                for (shaders, kind) in [
                    (quads.iter(), EffectPipelineKind::Quad),
                    (subtrees.iter(), EffectPipelineKind::Subtree),
                ] {
                    for shader in shaders {
                        let key = EffectPipelineKey {
                            shader: shader.id().as_u64(),
                            format,
                            alpha,
                            kind,
                        };
                        cached_pipeline(&context, &layouts, key, || {
                            Self::create_effect_pipeline(
                                &context.device,
                                &layouts,
                                format,
                                alpha,
                                &shader,
                                matches!(kind, EffectPipelineKind::Subtree),
                                false,
                            )
                        })?;
                    }
                }
            }
            Ok(())
        }))
    }

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
        cached_pipeline(
            &resources.capture_context,
            &resources.bind_group_layouts,
            key,
            create,
        )
    }
}

fn cached_pipeline(
    context: &WgpuContext,
    layouts: &Arc<WgpuBindGroupLayouts>,
    key: EffectPipelineKey,
    create: impl FnOnce() -> anyhow::Result<wgpu::RenderPipeline>,
) -> anyhow::Result<Arc<wgpu::RenderPipeline>> {
    let existing = {
        let mut cache = context.pipeline_cache.lock().unwrap();
        cache.set_device(&context.device);
        cache.effects.pipelines.get(&key).and_then(Weak::upgrade)
    };
    // Driver compilation can be slow; never hold the shared cache lock across it.
    let candidate = match existing {
        Some(pipeline) => pipeline,
        None => Arc::new(create()?),
    };
    let mut cache = context.pipeline_cache.lock().unwrap();
    cache.set_device(&context.device);
    cache.layouts.get_or_insert_with(|| layouts.clone());
    let cache = &mut cache.effects;
    let pipeline = cache
        .pipelines
        .get(&key)
        .and_then(Weak::upgrade)
        .unwrap_or(candidate);
    cache.pipelines.insert(key, Arc::downgrade(&pipeline));
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
