use gpui::{BackdropShader, Bounds, Corners, Hsla, Pixels, Point, Window, hsla, point, px};

use super::liquid_glass::{LiquidGlassAppearance, LiquidGlassDeformation, paint_glass_shader};

/// A convex optical surface with a clear center and a folded edge.
/// All geometry is in logical pixels. The edge samples the surrounding scene;
/// reflection and dispersion retain that scene's colors.
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct LiquidGlassLensAppearance {
    pub blur_radius: Pixels,
    pub clarity: f32,
    pub tint: Hsla,
    pub saturation: f32,
    pub brightness: f32,
    /// Inward sampling reach of the curved body.
    pub refraction: Pixels,
    /// Width of the curved body near the silhouette.
    pub thickness: Pixels,
    /// Distance beyond the silhouette sampled by the grazing-angle reflection.
    pub edge_refraction: Pixels,
    /// Width of the recessed outer edge before it transitions into the curved body.
    pub edge_width: Pixels,
    /// Center magnification in `1..=1.25`.
    pub magnification: f32,
    /// Strength of the grazing-angle scene reflection in `0..=1`.
    pub fresnel: f32,
    pub dispersion: f32,
    pub highlight: f32,
    /// Internal absorption near the edge; does not cast an exterior shadow.
    pub edge_shadow: f32,
    pub rim_width: Pixels,
    pub light_direction: Point<f32>,
}

impl Default for LiquidGlassLensAppearance {
    fn default() -> Self {
        Self {
            blur_radius: px(0.8),
            clarity: 1.,
            tint: hsla(0., 0., 1., 0.),
            saturation: 1.,
            brightness: 1.,
            refraction: px(8.),
            thickness: px(12.),
            edge_refraction: px(3.),
            edge_width: px(5.),
            magnification: 1.025,
            fresnel: 0.35,
            dispersion: 0.045,
            highlight: 0.25,
            edge_shadow: 0.,
            rim_width: px(0.6),
            light_direction: point(-0.6, -0.8),
        }
    }
}

impl LiquidGlassLensAppearance {
    pub(super) fn uniforms(self, corners: Corners<Pixels>, scale: f32) -> gpui::EffectUniforms {
        LiquidGlassAppearance {
            blur_radius: self.blur_radius,
            clarity: self.clarity,
            tint: self.tint,
            saturation: self.saturation,
            brightness: self.brightness,
            refraction: self.refraction,
            thickness: self.thickness,
            dispersion: self.dispersion,
            highlight: self.highlight,
            edge_shadow: self.edge_shadow,
            rim_width: self.rim_width,
            light_direction: self.light_direction,
            ..LiquidGlassAppearance::clear()
        }
        .uniforms(corners, scale)
        .with_slot(
            5,
            [
                self.magnification.clamp(1., 1.25),
                self.edge_refraction.as_f32().max(0.) * scale,
                self.edge_width.as_f32().max(0.) * scale,
                self.fresnel.clamp(0., 1.),
            ],
        )
    }
}

/// Paints a lens over previously painted content without changing layout or hit regions.
/// The capture includes the outward reflection reach and localized deformation.
pub fn paint_liquid_glass_lens(
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    appearance: LiquidGlassLensAppearance,
    deformation: LiquidGlassDeformation,
    window: &mut Window,
) {
    let corners = corners.clamp_radii_for_quad_size(bounds.size);
    let uniforms = appearance.uniforms(corners, window.raster_scale_factor());
    let reach = appearance.edge_refraction.max(px(0.)) + appearance.thickness.max(px(0.)) * 0.25;
    paint_glass_shader(
        bounds,
        deformation,
        appearance.blur_radius,
        reach,
        liquid_glass_lens_shader(),
        uniforms,
        window,
    );
}

/// Portable convex lens shader. Slot 5 contains magnification, outward reach,
/// folded-edge width, and Fresnel reflection strength; other slots share glass geometry.
pub fn liquid_glass_lens_shader() -> BackdropShader {
    BackdropShader::wgsl(concat!(
        include_str!("shaders/liquid_glass_shape.wgsl"),
        "\n",
        include_str!("shaders/liquid_glass.wgsl"),
        "\n",
        include_str!("shaders/liquid_glass_lens.wgsl")
    ))
}
