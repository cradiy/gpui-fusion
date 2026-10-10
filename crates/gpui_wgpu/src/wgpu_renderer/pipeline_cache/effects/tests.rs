use super::*;
use crate::{WgpuContext, WgpuExternalRendererConfig};
use gpui::{DevicePixels, size};

#[test]
#[ignore = "requires a GPU adapter"]
fn custom_pipelines_reuse_isolate_and_release() -> anyhow::Result<()> {
    let context = WgpuContext::new_headless()?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let create = |context: &WgpuContext, alpha_mode| {
        WgpuRenderer::new_external(
            context,
            WgpuExternalRendererConfig {
                size: size(DevicePixels(32), DevicePixels(32)),
                format,
                alpha_mode,
                target_usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
        )
    };
    let source = "fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> { return sample_effect_image(input, input.uv); }";
    let shader = EffectShader::wgsl_two_images(source);
    let backdrop = BackdropShader::wgsl(
        "fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> { return sample_raw_backdrop(input, vec2<f32>(0.0)); }",
    );
    let kinds = [
        EffectPipelineKind::Quad,
        EffectPipelineKind::Subtree,
        EffectPipelineKind::SubtreeImages,
    ];
    let renderer = create(&context, wgpu::CompositeAlphaMode::Opaque)?;
    let mut originals = Vec::new();
    for kind in kinds {
        let pipeline = renderer.shared_effect_pipeline(format, &shader, kind)?;
        let weak = Arc::downgrade(&pipeline);
        assert!(
            originals
                .iter()
                .all(|other: &Weak<wgpu::RenderPipeline>| !other.ptr_eq(&weak))
        );
        originals.push(weak);
    }
    let original_backdrop = Arc::downgrade(&renderer.shared_backdrop_effect_pipeline(&backdrop)?);
    drop(renderer);

    let renderer = create(&context, wgpu::CompositeAlphaMode::Opaque)?;
    for (kind, original) in kinds.into_iter().zip(&originals) {
        let pipeline = renderer.shared_effect_pipeline(format, &shader, kind)?;
        assert!(
            original.ptr_eq(&Arc::downgrade(&pipeline)),
            "reuse after the last renderer is dropped"
        );
    }
    assert!(original_backdrop.ptr_eq(&Arc::downgrade(
        &renderer.shared_backdrop_effect_pipeline(&backdrop)?
    )));
    let different_format = renderer.shared_effect_pipeline(
        wgpu::TextureFormat::Rgba16Float,
        &shader,
        EffectPipelineKind::Quad,
    )?;
    assert!(!originals[0].ptr_eq(&Arc::downgrade(&different_format)));
    drop(different_format);
    let transparent = create(&context, wgpu::CompositeAlphaMode::PreMultiplied)?;
    let different_alpha =
        transparent.shared_effect_pipeline(format, &shader, EffectPipelineKind::Quad)?;
    assert!(!originals[0].ptr_eq(&Arc::downgrade(&different_alpha)));
    drop((different_alpha, transparent));

    let active = renderer.shared_effect_pipeline(format, &shader, EffectPipelineKind::Quad)?;
    for index in 0..RETAINED_EFFECT_CAPACITY {
        let variant = EffectShader::wgsl_two_images(format!("// variant {index}\n{source}"));
        drop(renderer.shared_effect_pipeline(format, &variant, EffectPipelineKind::Quad)?);
        // Keep a second entry hot while the first remains alive outside the LRU.
        drop(renderer.shared_effect_pipeline(format, &shader, EffectPipelineKind::Subtree)?);
    }
    assert!(
        originals[1].upgrade().is_some(),
        "cache hits refresh recency"
    );
    assert!(
        originals[2].upgrade().is_none(),
        "unused configurations are evicted"
    );
    assert!(original_backdrop.upgrade().is_none());
    let shared = renderer.shared_effect_pipeline(format, &shader, EffectPipelineKind::Quad)?;
    assert!(
        Arc::ptr_eq(&active, &shared),
        "live evicted pipelines remain shareable"
    );
    drop((active, shared));

    let invalid = EffectShader::wgsl("invalid WGSL");
    assert!(
        renderer
            .shared_effect_pipeline(format, &invalid, EffectPipelineKind::Quad)
            .is_err()
    );
    assert!(originals[0].upgrade().is_some());
    drop(renderer);

    let mut replacement = WgpuContext::new_headless()?;
    replacement.pipeline_cache = context.pipeline_cache.clone();
    let renderer = create(&replacement, wgpu::CompositeAlphaMode::Opaque)?;
    assert!(
        originals
            .iter()
            .all(|pipeline| pipeline.upgrade().is_none()),
        "device replacement releases old custom pipelines"
    );
    let pipeline = renderer.shared_effect_pipeline(format, &shader, EffectPipelineKind::Quad)?;
    assert!(!originals[0].ptr_eq(&Arc::downgrade(&pipeline)));
    let weak = Arc::downgrade(&pipeline);
    drop((pipeline, renderer, context));
    assert!(weak.upgrade().is_some());
    drop(replacement);
    assert!(
        weak.upgrade().is_none(),
        "context release must release retained pipelines"
    );
    Ok(())
}
