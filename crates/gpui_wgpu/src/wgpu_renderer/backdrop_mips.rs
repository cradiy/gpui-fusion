pub(super) struct BackdropMips {
    pipeline: wgpu::RenderPipeline,
    levels: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

impl BackdropMips {
    pub fn new(device: &wgpu::Device, texture: &wgpu::Texture) -> Self {
        let source = include_str!("backdrop_mips.wgsl").replace(
            "const SRGB: bool = false;",
            if texture.format().is_srgb() {
                "const SRGB: bool = true;"
            } else {
                "const SRGB: bool = false;"
            },
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("backdrop_mips"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("backdrop_mips"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let views: Vec<_> = (0..texture.mip_level_count())
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let layout = pipeline.get_bind_group_layout(0);
        let levels = views
            .windows(2)
            .map(|pair| {
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("backdrop_mip_source"),
                    layout: &layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&pair[0]),
                    }],
                });
                (pair[1].clone(), bind)
            })
            .collect();
        Self { pipeline, levels }
    }

    pub fn generate(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        rect: [u32; 4],
        level_count: u32,
    ) {
        for (index, (view, bind)) in self
            .levels
            .iter()
            .take(level_count.saturating_sub(1) as usize)
            .enumerate()
        {
            let divisor = 1 << (index + 1);
            let width = (texture.width() / divisor).max(1);
            let height = (texture.height() / divisor).max(1);
            // Normalized coordinates also cover odd-sized viewports.
            let left = (rect[0] as f64 * width as f64 / texture.width() as f64).floor() as u32;
            let top = (rect[1] as f64 * height as f64 / texture.height() as f64).floor() as u32;
            let right = (rect[2] as f64 * width as f64 / texture.width() as f64).ceil() as u32;
            let bottom = (rect[3] as f64 * height as f64 / texture.height() as f64).ceil() as u32;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop_mip_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_scissor_rect(left, top, right.min(width) - left, bottom.min(height) - top);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
