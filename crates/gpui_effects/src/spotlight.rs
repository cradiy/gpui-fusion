use gpui::{
    Corners, Div, EffectShader, EffectUniforms, PaintEffect, Pixels, Point, Rgba, Size, div, point,
    px, rgb,
};

/// Local surface light and rounded-edge illumination.
#[derive(Clone, Copy, Debug)]
pub struct SpotlightOptions {
    /// Normalized surface coordinates. Values outside 0..=1 allow nearby light sources.
    pub center: Point<f32>,
    pub color: Rgba,
    /// Light support radius in logical pixels.
    pub radius: Pixels,
    /// Width of the illuminated inner edge in logical pixels.
    pub edge_width: Pixels,
    pub surface_opacity: f32,
    pub edge_opacity: f32,
    /// Overall light opacity, independent of the content. Zero skips painting.
    pub strength: f32,
}

impl Default for SpotlightOptions {
    fn default() -> Self {
        Self {
            center: point(0.5, 0.5),
            color: rgb(0xa8c7ff),
            radius: px(180.),
            edge_width: px(2.),
            surface_opacity: 0.13,
            edge_opacity: 0.9,
            strength: 1.,
        }
    }
}

fn nonnegative(value: f32) -> f32 {
    if value.is_finite() { value.max(0.) } else { 0. }
}

impl SpotlightOptions {
    /// Parameters for [`spotlight_shader`], using resolved logical geometry and raster scale.
    pub fn uniforms(
        self,
        size: Size<Pixels>,
        corners: Corners<Pixels>,
        scale: f32,
    ) -> EffectUniforms {
        let center_valid = self.center.x.is_finite() && self.center.y.is_finite();
        let strength = if center_valid && self.radius > px(0.) {
            nonnegative(self.strength).min(1.)
        } else {
            0.
        };
        let center = if center_valid {
            self.center
        } else {
            point(0., 0.)
        };
        let corners = corners.clamp_radii_for_quad_size(size);
        EffectUniforms::new()
            .with_slot(
                0,
                [
                    center.x * f32::from(size.width) * scale,
                    center.y * f32::from(size.height) * scale,
                    nonnegative(f32::from(self.radius)) * scale,
                    strength,
                ],
            )
            .with_slot(1, [self.color.r, self.color.g, self.color.b, self.color.a])
            .with_slot(
                2,
                [
                    nonnegative(self.surface_opacity).min(1.),
                    nonnegative(self.edge_opacity).min(1.),
                    nonnegative(f32::from(self.edge_width)) * scale,
                    0.,
                ],
            )
            .with_slot(
                3,
                [
                    f32::from(corners.top_left) * scale,
                    f32::from(corners.top_right) * scale,
                    f32::from(corners.bottom_right) * scale,
                    f32::from(corners.bottom_left) * scale,
                ],
            )
    }
}

/// A styled container illuminated beneath its children, without a subtree capture.
/// The caller supplies pointer coordinates and animates `strength` for entry and exit.
/// Children, layout and pointer targets are unchanged. Ancestor clipping applies.
pub fn spotlight(options: SpotlightOptions) -> Div {
    div().on_paint_before_children(move |bounds, style, window, _| {
        let corners = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let uniforms = options.uniforms(bounds.size, corners, window.raster_scale_factor());
        if uniforms.slots()[0][3] <= 0. || uniforms.slots()[0][2] <= 0. {
            return;
        }
        let _ = window.paint_effect(
            PaintEffect::new(bounds, spotlight_shader())
                .uniforms(uniforms)
                .corner_radii(corners),
        );
    })
}

/// Slot 0: center.xy, support radius, strength; slot 1: RGBA light color;
/// slot 2: surface opacity, edge opacity, edge width, unused; slot 3: TL/TR/BR/BL radii.
/// All lengths are device pixels.
pub fn spotlight_shader() -> EffectShader {
    EffectShader::wgsl(include_str!("shaders/spotlight.wgsl"))
}
