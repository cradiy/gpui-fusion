#![cfg(not(target_family = "wasm"))]
use gpui::{Bounds, ContentMask, DevicePixels, ScaledPixels, Scene, point, px, size};
use gpui_wgpu::WgpuOffscreenRenderer;

#[test]
#[ignore = "requires a GPU adapter"]
fn particle_and_fluid_simulations_render_and_replay() -> anyhow::Result<()> {
    use gpui_effects::{Fluid, FluidOptions, FluidSplat, ParticleSpawn, Particles};
    use std::time::Duration;

    let mut renderer = WgpuOffscreenRenderer::new(size(DevicePixels(128), DevicePixels(64)))?;

    let mut particles = Particles::new(128);
    particles.emit(ParticleSpawn {
        from: point(px(32.), px(32.)),
        to: point(px(32.), px(32.)),
        count: 32,
        speed: px(0.)..px(0.),
        color: gpui::rgb(0xff8060),
        ..Default::default()
    });
    let mut fluid = Fluid::new(FluidOptions {
        resolution: 32,
        ..Default::default()
    });
    fluid.splat(FluidSplat {
        from: point(px(32.), px(32.)),
        to: point(px(32.), px(32.)),
        color: gpui::rgb(0x60a0ff),
        ..Default::default()
    });
    let mut scene = Scene::default();
    scene.insert_primitive(gpui::ParticleDraw {
        order: 0,
        bounds: bounds(0., 0., 64., 64.),
        content_mask: ContentMask {
            bounds: bounds(0., 0., 128., 64.),
        },
        scale_factor: 1.,
        opacity: 1.,
        frame: particles.advance(Duration::from_millis(16)),
    });
    scene.insert_primitive(gpui::FluidDraw {
        order: 0,
        bounds: bounds(64., 0., 64., 64.),
        content_mask: ContentMask {
            bounds: bounds(0., 0., 128., 64.),
        },
        scale_factor: 1.,
        opacity: 1.,
        frame: fluid.advance(Duration::from_millis(16)),
    });
    scene.finish();
    // New particles fade in from zero alpha; advance a subsequent simulation frame.
    renderer.render_rgba(&scene)?;
    scene.particles[0].frame = particles.advance(Duration::from_millis(60));
    scene.fluids[0].frame = fluid.advance(Duration::from_millis(60));
    let result = renderer.render_rgba(&scene)?;
    for (x, channel) in [(32, 0), (96, 2)] {
        assert!(
            result[(32 * 128 + x) * 4 + channel] > 20,
            "simulation at x={x} must produce visible pixels"
        );
    }
    assert_eq!(
        renderer.render_rgba(&scene)?,
        result,
        "replaying a scene must not advance its simulations"
    );
    Ok(())
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds::new(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(width), ScaledPixels(height)),
    )
}
