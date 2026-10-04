use gpui::{Div, EffectShader, EffectUniforms, PaintEffect, Point, Rgba, div};

/// One color source in a four-point gradient.
#[derive(Clone, Copy, Debug)]
pub struct GradientPoint {
    /// Normalized position within the surface, clamped to 0..=1.
    pub position: Point<f32>,
    /// Source color, including opacity.
    pub color: Rgba,
    /// Influence radius as a multiple of the surface's shorter edge.
    /// Positive finite values are clamped to 0.01..=4; other values disable the point.
    pub radius: f32,
}

impl GradientPoint {
    /// Creates a color source with a radius of 0.65 times the shorter edge.
    pub fn new(position: Point<f32>, color: impl Into<Rgba>) -> Self {
        Self {
            position,
            color: color.into(),
            radius: 0.65,
        }
    }

    /// Sets the influence radius relative to the shorter edge.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

/// Smoothly blends four color sources beneath a styled container's children.
/// The caller owns the points; this element does not schedule animation frames.
pub fn point_gradient(points: [GradientPoint; 4]) -> Div {
    let uniforms = point_gradient_uniforms(points);
    div().on_paint_before_children(move |bounds, style, window, _| {
        let corners = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let _ = window.paint_effect(
            PaintEffect::new(bounds, point_gradient_shader())
                .uniforms(uniforms)
                .corner_radii(corners),
        );
    })
}

/// Two slots per point: normalized x/y, radius, unused; then straight RGBA.
pub fn point_gradient_uniforms(points: [GradientPoint; 4]) -> EffectUniforms {
    fn unit(value: f32) -> f32 {
        if value.is_finite() {
            value.clamp(0., 1.)
        } else {
            0.
        }
    }
    let mut uniforms = EffectUniforms::new();
    for (index, point) in points.into_iter().enumerate() {
        let valid = point.position.x.is_finite()
            && point.position.y.is_finite()
            && point.radius.is_finite()
            && point.radius > 0.;
        uniforms.set_slot(
            index * 2,
            [
                unit(point.position.x),
                unit(point.position.y),
                if valid {
                    point.radius.clamp(0.01, 4.)
                } else {
                    0.
                },
                0.,
            ],
        );
        uniforms.set_slot(
            index * 2 + 1,
            [
                unit(point.color.r),
                unit(point.color.g),
                unit(point.color.b),
                unit(point.color.a),
            ],
        );
    }
    uniforms
}

/// The four-point gradient shader, parameterized by [`point_gradient_uniforms`].
pub fn point_gradient_shader() -> EffectShader {
    EffectShader::wgsl(include_str!("shaders/point_gradient.wgsl"))
}
