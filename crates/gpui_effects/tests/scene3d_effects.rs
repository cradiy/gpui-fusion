#![cfg(not(target_family = "wasm"))]

use gpui::{
    Bounds, ContentMask, DevicePixels, EffectQuad, EffectShader, Mesh3d, MeshDraw3d, MeshTexture3d,
    MeshVertex3d, Primitive, ScaledPixels, Scene, Scene3dFrame, SubtreeLayer, point, rgba, size,
};
use gpui_wgpu::WgpuOffscreenRenderer;
use std::{rc::Rc, sync::Arc};

const IDENTITY: [[f32; 4]; 4] = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

#[test]
#[ignore = "requires a GPU adapter"]
fn viewport_bloom_and_grading_preserve_nested_clipping_and_group_opacity() {
    let mut renderer =
        WgpuOffscreenRenderer::new(size(DevicePixels(64), DevicePixels(64))).unwrap();
    let captured = scene(layer(
        bounds(8., 8., 48., 48.),
        Scene::default(),
        vec![mesh(0.5, 0xff8040ff, MeshTexture3d::None)],
        1.,
    ));
    let mut outer = layer(bounds(0., 0., 64., 64.), captured, vec![], 1.);
    outer.scene3d = None;
    let bloom = gpui::SubtreeEffectPass {
        shader: gpui_effects::subtree_identity_shader(),
        uniforms: gpui::EffectUniforms::new()
            .with_slot(0, [0.4, 0.1, 1.6, 0.])
            .with_slot(1, [12., 0., 0., 0.]),
        time: 0.,
        images: Default::default(),
        bloom: Some(gpui::SubtreeBloomPass {
            extract: gpui_effects::bloom_extract_shader(),
            blur: gpui_effects::bloom_blur_shader(),
            composite: gpui_effects::bloom_composite_shader(),
            downsample: 1,
        }),
        feedback: None,
        distance_field: None,
        particles: None,
        particle_transition: None,
    };
    let grading = gpui::SubtreeEffectPass {
        shader: gpui_effects::subtree_color_adjust_shader(),
        uniforms: gpui::EffectUniforms::new().with_slot(0, [0., 1., 1., 0.]),
        bloom: None,
        ..bloom.clone()
    };

    for extent in [64, 80, 64] {
        renderer.resize(size(DevicePixels(extent), DevicePixels(extent)));
        let pixel = |image: &[u8], x: usize, y: usize| -> [u8; 4] {
            image[(y * extent as usize + x) * 4..][..4]
                .try_into()
                .unwrap()
        };
        let original = renderer.render_rgba(&scene(outer.clone())).unwrap();
        assert_eq!(pixel(&original, 13, 32), [0, 0, 0, 255]);
        assert!(pixel(&original, 32, 32)[0] > pixel(&original, 32, 32)[1]);

        let mut processed = outer.clone();
        processed.intermediate_effects = vec![bloom.clone(), grading.clone()].into();
        let image = renderer.render_rgba(&scene(processed.clone())).unwrap();
        for (x, y) in [(32, 32), (13, 32)] {
            let p = pixel(&image, x, y);
            assert!(p[0] > 0);
            assert!(p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1);
        }
        assert_eq!(pixel(&image, 32, 32)[3], 255);
        assert_eq!(
            renderer.render_rgba(&scene(processed.clone())).unwrap(),
            image
        );

        processed.composite.opacity = 0.5;
        let faded = renderer.render_rgba(&scene(processed.clone())).unwrap();
        for (x, y) in [(32, 32), (13, 32)] {
            for channel in 0..3 {
                let linear = |value: u8| {
                    let value = f32::from(value) / 255.;
                    if value <= 0.04045 {
                        value / 12.92
                    } else {
                        ((value + 0.055) / 1.055).powf(2.4)
                    }
                };
                let full = linear(pixel(&image, x, y)[channel]);
                let faded = linear(pixel(&faded, x, y)[channel]);
                assert!(
                    (2. * faded - full).abs() <= 0.015,
                    "linear opacity: {faded} != {}",
                    full * 0.5
                );
            }
        }
        processed.composite.content_mask.bounds = bounds(0., 0., 32., 64.);
        let clipped = renderer.render_rgba(&scene(processed)).unwrap();
        assert_eq!(pixel(&clipped, 40, 32), [0, 0, 0, 255]);
        assert_eq!(pixel(&clipped, 20, 32), pixel(&faded, 20, 32));
        assert_eq!(
            renderer.render_rgba(&scene(outer.clone())).unwrap(),
            original
        );
    }
}

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    )
}

fn mesh(z: f32, color: u32, texture: MeshTexture3d) -> MeshDraw3d {
    let vertices = [
        (-0.7, -0.7, [0., 1.]),
        (0.7, -0.7, [1., 1.]),
        (0.7, 0.7, [1., 0.]),
        (-0.7, 0.7, [0., 0.]),
    ]
    .map(|(x, y, uv)| MeshVertex3d {
        position: [x, y, z],
        normal: [0., 0., 1.],
        uv,
    });
    MeshDraw3d {
        mesh_passes: Default::default(),
        custom_material: None,
        gpu_geometry: None,
        render_bounds: None,
        cast_shadows: true,
        receive_shadows: true,
        output_id: 1,
        mesh: Mesh3d::new(vertices.to_vec(), vec![0, 1, 2, 0, 2, 3]),
        model: IDENTITY,
        normal: IDENTITY,
        color: rgba(color),
        texture,
        sampling: Default::default(),
        uv_set: 0,
        image_color_space: Default::default(),
        pbr: None,
        metallic_roughness_texture: None,
        emissive_texture: None,
        normal_texture: None,
        normal_scale: 1.,
        occlusion_texture: None,
        occlusion_strength: 1.,
        unlit: true,
        alpha_cutoff: 0.5,
        double_sided: true,
        alpha_mode: gpui::AlphaMode3d::Mask,
        sort_depth: 0.,
    }
}

fn layer(
    region: Bounds<ScaledPixels>,
    mut source: Scene,
    objects: Vec<MeshDraw3d>,
    opacity: f32,
) -> SubtreeLayer {
    source.finish();
    SubtreeLayer {
        scene3d: Some(Arc::new(Scene3dFrame {
            pick_capture: None,
            occlusion_groups: Default::default(),
            depth_background: Default::default(),
            viewport_quality: Default::default(),
            background: None,
            specular_environment: None,
            directional_shadow: None,
            ui_texture: None,
            view_projection: IDENTITY,
            world_to_view: [IDENTITY[0], IDENTITY[1], IDENTITY[2], [0., 0., -3., 1.]],
            camera_position: [0., 0., 3.],
            orthographic_view_direction: None,
            light_direction: [0., 0., 1.],
            light: [1.; 4],
            lights: None,
            ambient: 0.3,
            diffuse_environment: None,
            color_output: Default::default(),
            objects: objects.into(),
        })),
        scene: Rc::new(source),
        second_scene: None,
        intermediate_effects: Arc::default(),
        composite: EffectQuad {
            order: 0,
            bounds: region,
            effect_bounds: region,
            transformation: Default::default(),
            content_mask: ContentMask { bounds: region },
            shader: EffectShader::wgsl_image(
                "fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> { return sample_effect_image(input, input.uv); }",
            ),
            uniforms: Default::default(),
            time: 0.,
            corner_radii: Default::default(),
            opacity,
            image_tile: None,
            second_image_tile: None,
            third_image_tile: None,
            fourth_image_tile: None,
        },
    }
}

fn scene(layer: SubtreeLayer) -> Scene {
    let mut scene = Scene::default();
    scene.insert_primitive(Primitive::SubtreeLayer(layer));
    scene.finish();
    scene
}
