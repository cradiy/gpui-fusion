use super::{SceneEncoding, WgpuRenderer};
use gpui::Scene;

#[cfg(test)]
mod tests;

pub(super) struct PresentationTarget {
    pub texture: wgpu::Texture,
    view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    bindings: wgpu::BindGroup,
}

impl PresentationTarget {
    fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let (texture, view) = WgpuRenderer::create_backdrop_intermediate(
            device,
            "gpui_presentation_source",
            config.format,
            config.width,
            config.height,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpui_presentation"),
            source: wgpu::ShaderSource::Wgsl(gpui_render::PRESENTATION_WGSL.into()),
        });
        let bindings_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpui_presentation"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpui_presentation"),
            bind_group_layouts: &[Some(&bindings_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpui_presentation"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_present"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_present"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpui_presentation"),
            layout: &bindings_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        Self {
            texture,
            view,
            pipeline,
            bindings,
        }
    }

    fn encode(&self, target: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gpui_presentation"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl WgpuRenderer {
    pub(super) fn encode_presented_scene(
        &mut self,
        scene: &Scene,
        target: &wgpu::Texture,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<SceneEncoding> {
        // Subtree shaders can fall back to painting into their parent's target.
        let mut needs_backdrop = false;
        self.visit_rendered_scenes(scene, &mut |scene| {
            needs_backdrop |= !scene.backdrop_blurs.is_empty();
        });
        let indirect = !self
            .surface_config
            .usage
            .contains(wgpu::TextureUsages::COPY_SRC)
            && needs_backdrop;
        let intermediate = if indirect {
            if self.resources().presentation.is_none() {
                let presentation =
                    PresentationTarget::new(&self.resources().device, &self.surface_config);
                self.resources_mut().presentation = Some(presentation);
            }
            let presentation = self.resources().presentation.as_ref().unwrap();
            Some((presentation.texture.clone(), presentation.view.clone()))
        } else {
            None
        };
        let (scene_texture, scene_view) = intermediate
            .as_ref()
            .map(|(texture, view)| (texture, view))
            .unwrap_or((target, view));
        let encoded = self.encode_scene(
            scene,
            scene_texture,
            scene_view,
            encoder,
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            true,
            &[],
        )?;
        if indirect && encoded == SceneEncoding::Complete {
            self.resources()
                .presentation
                .as_ref()
                .unwrap()
                .encode(view, encoder);
        }
        Ok(encoded)
    }
}
