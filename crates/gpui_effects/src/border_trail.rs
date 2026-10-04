use gpui::{
    BorderGradient, Corners, Div, EffectShader, EffectUniforms, PaintEffect, Pixels, Rgba, Size,
    div, outline, px, rgb,
};

/// Where gradient colors are anchored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderTrailMode {
    /// Colors travel with the light, from its head to its fading tail.
    #[default]
    Trail,
    /// Colors remain fixed on the complete border; the traveling light brightens them.
    Border,
}

/// A light segment traveling clockwise along a rounded border.
#[derive(Clone, Debug)]
pub struct BorderTrailOptions {
    /// Turns around the perimeter, starting at the top-left corner's top tangent.
    /// Values wrap. Decrease progress for counterclockwise travel and set `reverse`.
    pub progress: f32,
    /// Tail length in logical pixels, capped below one full perimeter.
    pub length: Pixels,
    /// Illuminated width inside the outer edge, in logical pixels.
    pub width: Pixels,
    pub color: Rgba,
    /// Optional standard border gradient, replacing `color` when present.
    pub gradient: Option<BorderGradient>,
    pub mode: BorderTrailMode,
    /// Unlit border opacity in `Border` mode, relative to `opacity`.
    pub base_opacity: f32,
    pub opacity: f32,
    /// Trail behind counterclockwise travel instead of clockwise travel.
    pub reverse: bool,
}

impl Default for BorderTrailOptions {
    fn default() -> Self {
        Self {
            progress: 0.,
            length: px(180.),
            width: px(2.),
            color: rgb(0xa2b8ff),
            gradient: None,
            mode: BorderTrailMode::Trail,
            base_opacity: 0.25,
            opacity: 1.,
            reverse: false,
        }
    }
}

impl BorderTrailOptions {
    /// Uses GPUI's border gradient, including arbitrary stops, midpoints and color space.
    pub fn gradient(mut self, gradient: BorderGradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Shader parameters from resolved logical geometry and raster scale.
    pub fn uniforms(
        &self,
        size: Size<Pixels>,
        corners: Corners<Pixels>,
        scale: f32,
    ) -> EffectUniforms {
        if !self.progress.is_finite()
            || !self.opacity.is_finite()
            || !self.base_opacity.is_finite()
            || !f32::from(self.length).is_finite()
            || !f32::from(self.width).is_finite()
            || self.length <= px(0.)
            || self.width <= px(0.)
        {
            return EffectUniforms::new();
        }
        let corners = corners.clamp_radii_for_quad_size(size);
        EffectUniforms::new()
            .with_slot(
                0,
                [
                    self.progress.rem_euclid(1.),
                    f32::from(self.length) * scale,
                    f32::from(self.width) * scale,
                    self.opacity.clamp(0., 1.),
                ],
            )
            .with_slot(1, [self.color.r, self.color.g, self.color.b, self.color.a])
            .with_slot(
                2,
                [
                    f32::from(corners.top_left) * scale,
                    f32::from(corners.top_right) * scale,
                    f32::from(corners.bottom_right) * scale,
                    f32::from(corners.bottom_left) * scale,
                ],
            )
            .with_slot(
                3,
                [
                    if self.reverse { -1. } else { 1. },
                    if self.mode == BorderTrailMode::Border {
                        1.
                    } else {
                        0.
                    },
                    f32::from(Self::palette_width(size)) * scale * 0.5,
                    self.base_opacity.clamp(0., 1.),
                ],
            )
    }

    fn palette_width(size: Size<Pixels>) -> Pixels {
        px(4.).min(size.width * 0.5).min(size.height * 0.5)
    }
}

/// A styled container with a traveling border light beneath its children.
/// Uses resolved corner radii and leaves layout and child input unchanged.
/// Gradient mode captures only a border palette; child content is not captured.
/// The caller advances `progress` and schedules animation frames.
pub fn border_trail(options: BorderTrailOptions) -> Div {
    div().on_paint_before_children(move |bounds, style, window, _| {
        let corners = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let uniforms = options.uniforms(bounds.size, corners, window.raster_scale_factor());
        if uniforms.slots()[0][3] <= 0. {
            return;
        }
        if let Some(gradient) = &options.gradient {
            if !window.supports_subtree_effects() {
                window.with_subtree_effect(
                    bounds,
                    border_trail_gradient_shader(),
                    uniforms,
                    0.,
                    options.opacity,
                    |window| {
                        window.paint_quad(
                            outline(bounds, rgb(0xffffff), Default::default())
                                .corner_radii(corners)
                                .border_widths(options.width)
                                .border_gradient(gradient.clone()),
                        )
                    },
                );
                return;
            }
            window.with_subtree_effect(
                bounds,
                border_trail_gradient_shader(),
                uniforms,
                0.,
                1.,
                |window| {
                    window.with_effect_source_bounds(bounds, |window| {
                        window.paint_quad(
                            outline(bounds, rgb(0xffffff), Default::default())
                                .corner_radii(corners)
                                .border_widths(BorderTrailOptions::palette_width(bounds.size))
                                .border_gradient(gradient.clone()),
                        )
                    });
                },
            );
            return;
        }
        let _ =
            window.paint_effect(PaintEffect::new(bounds, border_trail_shader()).uniforms(uniforms));
    })
}

/// Slot 0: progress, tail length, width, opacity; slot 1: RGBA;
/// slot 2: TL/TR/BR/BL radii; slot 3: direction, border mode, palette inset, base opacity.
/// All lengths are device pixels.
pub fn border_trail_shader() -> EffectShader {
    EffectShader::wgsl(format!(
        "{}\nfn trail_color(input: EffectInput, params: EffectParams, t: f32) -> vec4<f32> {{ return params.slots[1]; }}",
        include_str!("shaders/border_trail.wgsl")
    ))
}

/// Image shader for a captured GPUI border gradient. Shares uniforms with the solid shader.
pub fn border_trail_gradient_shader() -> EffectShader {
    EffectShader::wgsl_image(concat!(
        include_str!("shaders/border_trail.wgsl"),
        "\n",
        include_str!("shaders/border_trail_gradient.wgsl")
    ))
}
