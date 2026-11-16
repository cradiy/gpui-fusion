use gpui::{
    App, BackdropShader, Bounds, Corners, Div, EffectUniforms, ElementId, Hsla, InteractiveElement,
    Interactivity, IntoElement, PaintBackdropEffect, ParentElement, Pixels, Point, RenderOnce,
    Rgba, StatefulInteractiveElement, StyleRefinement, Styled, Window, div, hsla, point, px,
};

const OPTICS_SLOT: usize = 0;
const TINT_SLOT: usize = 1;
const SURFACE_SLOT: usize = 2;
const LIGHT_SLOT: usize = 3;
const CORNERS_SLOT: usize = 4;
const SHAPE_SLOT: usize = 6;
const DEFORMATION_SLOT: usize = 7;

/// Localized silhouette displacement with optical normals following the contour.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiquidGlassDeformation {
    /// Influence location in normalized surface coordinates, clamped to `0..=1`.
    pub focus: Point<f32>,
    /// Displacement near the focus, in logical pixels; positive values expand outward.
    pub bulge: Pixels,
    /// Signed displacement of the secondary contour wave, in logical pixels.
    pub ripple: Pixels,
    /// Blend from the rounded rectangle to an ellipse, in `0..=1`.
    pub ellipticity: f32,
}

impl LiquidGlassDeformation {
    pub(crate) fn clamped(self, size: gpui::Size<Pixels>) -> Self {
        let limit = size.width.min(size.height).max(px(0.)) * 0.3;
        Self {
            focus: point(self.focus.x.clamp(0., 1.), self.focus.y.clamp(0., 1.)),
            bulge: self.bulge.clamp(-limit, limit),
            ripple: self.ripple.clamp(-limit, limit),
            ellipticity: self.ellipticity.clamp(0., 1.),
        }
    }
}

/// Height-field lens shading and interior magnification.
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct LiquidGlassOptics {
    /// Interior enlargement; zero preserves the background scale.
    pub magnification: f32,
    pub rim_strength: f32,
    pub top_reflection: f32,
    pub inner_shading: f32,
    pub caustic_strength: f32,
    /// Neutral environment reflection weighted by the lens normal, in `0..=1`.
    pub environment_reflection: f32,
    /// Fraction of the edge mip level used in the flat center, in `0..=1`.
    pub center_blur: f32,
    /// Direction toward the light in downward-Y coordinates.
    pub light_direction: Point<f32>,
    /// Blend toward directional lighting, in `0..=1`.
    pub directional_light: f32,
}

impl Default for LiquidGlassOptics {
    fn default() -> Self {
        Self {
            magnification: 0.,
            rim_strength: 0.,
            top_reflection: 0.16,
            inner_shading: 0.04,
            caustic_strength: 0.,
            environment_reflection: 0.,
            center_blur: 0.35 / 2.2,
            light_direction: point(0., -1.),
            directional_light: 0.,
        }
    }
}

/// A circle joined to the glass pane by a smooth union.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidGlassMerge {
    /// Circle center relative to the pane center, in logical pixels.
    pub offset: Point<Pixels>,
    pub radius: Pixels,
    /// Smooth-union distance in logical pixels; zero gives a hard union.
    pub smoothness: Pixels,
}

/// Shape controls shared by the glass material and foreground mask.
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct LiquidGlassShape {
    /// Superellipse exponent in `2..=7`; 2 produces circular corners.
    pub roundness: f32,
    pub merge: Option<LiquidGlassMerge>,
}

impl Default for LiquidGlassShape {
    fn default() -> Self {
        Self {
            roundness: 2.,
            merge: None,
        }
    }
}

impl LiquidGlassShape {
    pub(super) fn merge_uniform(self, scale: f32) -> [f32; 4] {
        self.merge.map_or([0.; 4], |merge| {
            [
                merge.offset.x.as_f32() * scale,
                merge.offset.y.as_f32() * scale,
                merge.radius.as_f32().max(0.) * scale,
                merge.smoothness.as_f32().max(0.) * scale,
            ]
        })
    }
}

/// Near and ambient drop shadows, sampled before refraction and blur.
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct LiquidGlassShadow {
    pub spread: Pixels,
    pub strength: f32,
    /// Shadow translation in logical pixels; positive Y points down.
    pub offset: Point<Pixels>,
}

impl Default for LiquidGlassShadow {
    fn default() -> Self {
        Self {
            spread: px(25.),
            strength: 0.,
            offset: point(px(0.), px(10.)),
        }
    }
}

/// Optical parameters for [`LiquidGlass`]. Layout and foreground styling use [`Styled`].
#[derive(Clone, Copy, Debug, PartialEq, uic_macros::Chainable)]
pub struct LiquidGlassStyle {
    pub optics: LiquidGlassOptics,
    pub shape: LiquidGlassShape,
    pub shadow: LiquidGlassShadow,
    /// Largest background sampling footprint, in logical pixels.
    /// Each doubling selects the next mip level; values at or below one remain sharp.
    pub blur_radius: Pixels,
    /// Maximum inward edge displacement, in logical pixels.
    pub refraction: Pixels,
    /// Width of the curved edge region, in logical pixels.
    pub thickness: Pixels,
    /// Relative RGB separation at refracted edges in `0..=1`; zero disables it.
    pub dispersion: f32,
    /// Body color; alpha controls the strength of tonal coloring, not surface opacity.
    pub tint: Hsla,
    /// Backdrop luminance influence on the body color, in `0..=1`.
    /// Zero keeps a constant body color; one gives the largest tonal variation.
    pub transmission: f32,
    /// Edge color wash; alpha controls strength without tinting the flat center.
    pub edge_color: Hsla,
    /// Opaque frost blended over the lens, in `0..=1`.
    pub frost: f32,
    pub frost_center: Hsla,
    pub frost_edge: Hsla,
}

impl LiquidGlassStyle {
    /// Clear height-field lens with a narrow rim and interior shading.
    pub fn optical() -> Self {
        Self {
            optics: LiquidGlassOptics::default(),
            shape: LiquidGlassShape::default(),
            shadow: LiquidGlassShadow {
                spread: px(34.),
                strength: 0.07,
                offset: point(px(0.), px(22.)),
            },
            blur_radius: px(2.2_f32.exp2()),
            refraction: px(37.),
            thickness: px(62.),
            dispersion: 0.12,
            tint: hsla(0., 0., 1., 0.),
            transmission: 0.6,
            edge_color: hsla(0., 0., 1., 0.),
            frost: 0.,
            frost_center: gpui::rgb(0xf9fafc).into(),
            frost_edge: gpui::rgb(0xe2e4e9).into(),
        }
    }

    /// Diffuse light glass with a partially frosted surface.
    pub fn frosted() -> Self {
        Self {
            blur_radius: px(18.),
            frost: 0.72,
            ..Self::optical()
        }
    }

    /// Optical glass with no background diffusion.
    pub fn clear() -> Self {
        Self {
            blur_radius: px(0.),
            ..Self::optical()
        }
    }

    /// Diffuse neutral glass for dark surfaces.
    pub fn dark() -> Self {
        Self {
            frost_center: gpui::rgb(0x2a2c31).into(),
            frost_edge: gpui::rgb(0x1c1d21).into(),
            ..Self::frosted()
        }
    }

    /// Packs material parameters in device pixels for backdrop rendering.
    pub fn uniforms(self, corners: Corners<Pixels>, scale: f32) -> EffectUniforms {
        let tint: Rgba = self.tint.into();
        let edge_color: Rgba = self.edge_color.into();
        let frost_center: Rgba = self.frost_center.into();
        let frost_edge: Rgba = self.frost_edge.into();
        EffectUniforms::new()
            .with_slot(
                OPTICS_SLOT,
                [
                    self.blur_radius.as_f32().clamp(1.0, 200.0).log2(),
                    if self.blur_radius > px(0.) { 1. } else { 0. },
                    self.refraction.as_f32().max(0.0) * scale,
                    self.thickness.as_f32().max(0.0) * scale,
                ],
            )
            .with_slot(TINT_SLOT, [tint.r, tint.g, tint.b, tint.a])
            .with_slot(5, [edge_color.r, edge_color.g, edge_color.b, edge_color.a])
            .with_slot(
                SURFACE_SLOT,
                [
                    self.frost.clamp(0., 1.),
                    self.optics.magnification.clamp(0., 1.),
                    self.dispersion.clamp(0., 1.),
                    self.optics.center_blur.clamp(0., 1.),
                ],
            )
            .with_slot(
                CORNERS_SLOT,
                [
                    corners.top_left.as_f32() * scale,
                    corners.top_right.as_f32() * scale,
                    corners.bottom_right.as_f32() * scale,
                    corners.bottom_left.as_f32() * scale,
                ],
            )
            .with_slot(
                8,
                [
                    self.optics.rim_strength.clamp(0., 1.),
                    self.optics.inner_shading.clamp(0., 1.),
                    self.optics.top_reflection.clamp(0., 1.),
                    self.optics.caustic_strength.clamp(0., 1.),
                ],
            )
            .with_slot(
                9,
                [
                    self.optics.light_direction.x,
                    self.optics.light_direction.y,
                    self.optics.directional_light.clamp(0., 1.),
                    self.optics.environment_reflection.clamp(0., 1.),
                ],
            )
            .with_slot(10, [self.transmission.clamp(0., 1.), 0., 0., scale])
            .with_slot(
                14,
                [
                    frost_center.r,
                    frost_center.g,
                    frost_center.b,
                    frost_center.a,
                ],
            )
            .with_slot(15, [frost_edge.r, frost_edge.g, frost_edge.b, frost_edge.a])
            .with_slot(11, [self.shape.roundness.clamp(2., 7.), 0., 0., 0.])
            .with_slot(12, self.shape.merge_uniform(scale))
            .with_slot(
                13,
                [
                    self.shadow.spread.as_f32().max(0.) * scale,
                    self.shadow.strength.clamp(0., 1.),
                    self.shadow.offset.x.as_f32() * scale,
                    self.shadow.offset.y.as_f32() * scale,
                ],
            )
    }
}

impl Default for LiquidGlassStyle {
    fn default() -> Self {
        Self::optical()
    }
}

/// A layout-neutral surface that applies a refractive material to its background.
///
/// The material samples content already painted in the same window. Children
/// are painted afterward and remain sharp. Sizing, individual corner radii,
/// foreground colors, padding, shadows, and interaction use the normal Div APIs.
pub struct LiquidGlass {
    div: Div,
    appearance: LiquidGlassStyle,
}

impl LiquidGlass {
    pub fn new() -> Self {
        Self::with_style(LiquidGlassStyle::default())
    }

    pub fn with_style(appearance: LiquidGlassStyle) -> Self {
        Self {
            div: div(),
            appearance,
        }
    }

    pub fn glass_style(mut self, appearance: LiquidGlassStyle) -> Self {
        self.appearance = appearance;
        self
    }

    /// Assigns a stable id for hover, click, focus, and other Div interactions.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.div.interactivity().element_id = Some(id.into());
        self
    }
}

impl Default for LiquidGlass {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for LiquidGlass {
    fn style(&mut self) -> &mut StyleRefinement {
        self.div.style()
    }
}

impl InteractiveElement for LiquidGlass {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.div.interactivity()
    }
}

impl StatefulInteractiveElement for LiquidGlass {}

impl ParentElement for LiquidGlass {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.div.extend(elements);
    }
}

impl IntoElement for LiquidGlass {
    type Element = gpui::ViewElement<Self>;

    fn into_element(self) -> Self::Element {
        gpui::ViewElement::new(self)
    }
}

impl RenderOnce for LiquidGlass {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let appearance = self.appearance;
        self.div.on_paint_before_children(
            move |bounds: Bounds<Pixels>, style, window: &mut Window, _| {
                let corners = style.corner_radii.to_pixels(window.rem_size());
                paint_liquid_glass(bounds, corners, appearance, window);
            },
        )
    }
}

/// Paints the material without owning layout, children, or interaction.
///
/// Bounds, corner radii, and appearance lengths are in logical pixels. Call
/// before painting foreground content, for example in a Div's
/// `on_paint_before_children` hook. Only previously painted content in the same
/// window is sampled.
pub fn paint_liquid_glass(
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    appearance: LiquidGlassStyle,
    window: &mut Window,
) {
    paint_deformed_liquid_glass(
        bounds,
        corners,
        appearance,
        LiquidGlassDeformation::default(),
        window,
    );
}

/// A uniformly rounded pane in logical window coordinates.
#[derive(Clone, Copy, Debug)]
pub struct LiquidGlassPane {
    pub bounds: Bounds<Pixels>,
    pub radius: Pixels,
}

/// Paints two panes as one smooth-union lens with a shared backdrop and contour.
/// Smoothness sets the neck's reach in logical pixels; zero gives a hard union.
/// Empty panes are omitted. Foreground content is painted separately.
pub fn paint_liquid_glass_union(
    panes: [LiquidGlassPane; 2],
    smoothness: Pixels,
    appearance: LiquidGlassStyle,
    window: &mut Window,
) {
    let [first, second] = panes;
    let empty = |pane: LiquidGlassPane| {
        pane.bounds.size.width <= px(0.) || pane.bounds.size.height <= px(0.)
    };
    if empty(first) || empty(second) {
        let pane = if empty(first) { second } else { first };
        if !empty(pane) {
            paint_liquid_glass(pane.bounds, Corners::all(pane.radius), appearance, window);
        }
        return;
    }
    let scale = window.raster_scale_factor();
    let offset = second.bounds.center() - first.bounds.center();
    let radius = second
        .radius
        .max(px(0.))
        .min(second.bounds.size.width.min(second.bounds.size.height) * 0.5);
    let uniforms = appearance
        .uniforms(
            Corners::all(first.radius).clamp_radii_for_quad_size(first.bounds.size),
            scale,
        )
        .with_slot(
            11,
            [
                appearance.shape.roundness.clamp(2., 7.),
                second.bounds.size.width.as_f32() * scale,
                second.bounds.size.height.as_f32() * scale,
                radius.as_f32() * scale,
            ],
        )
        .with_slot(
            12,
            [
                offset.x.as_f32() * scale,
                offset.y.as_f32() * scale,
                0.,
                smoothness.as_f32().max(0.) * scale,
            ],
        );
    let sampling_reach = appearance.refraction.max(px(0.))
        * (1. + appearance.dispersion.clamp(0., 1.) * 0.35)
        + first
            .bounds
            .size
            .width
            .max(first.bounds.size.height)
            .max(second.bounds.size.width)
            .max(second.bounds.size.height)
            * appearance.optics.magnification.clamp(0., 1.);
    paint_glass_shader(
        first.bounds,
        LiquidGlassDeformation::default(),
        appearance.blur_radius.clamp(px(0.), px(200.)),
        sampling_reach,
        liquid_glass_shader(),
        uniforms,
        window,
    );
}

/// Paints glass with a localized bulge and a secondary contour wave.
/// Bounds describe the undeformed surface; painting extends beyond them without
/// changing layout or hit regions. Ancestor clipping still applies.
pub fn paint_deformed_liquid_glass(
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    appearance: LiquidGlassStyle,
    deformation: LiquidGlassDeformation,
    window: &mut Window,
) {
    let scale = window.raster_scale_factor();
    let uniforms = appearance.uniforms(corners.clamp_radii_for_quad_size(bounds.size), scale);
    let sampling_reach = appearance.refraction.max(px(0.))
        * (1. + appearance.dispersion.clamp(0., 1.) * 0.35)
        + bounds.size.width.max(bounds.size.height) * appearance.optics.magnification.clamp(0., 1.);
    paint_glass_shader(
        bounds,
        deformation,
        appearance.blur_radius.clamp(px(0.), px(200.)),
        sampling_reach,
        liquid_glass_shader(),
        uniforms,
        window,
    );
}

pub(super) fn paint_glass_shader(
    bounds: Bounds<Pixels>,
    deformation: LiquidGlassDeformation,
    blur_radius: Pixels,
    sampling_reach: Pixels,
    shader: BackdropShader,
    uniforms: EffectUniforms,
    window: &mut Window,
) {
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
        return;
    }
    let deformation = deformation.clamped(bounds.size);
    let bulge = deformation.bulge;
    let ripple = deformation.ripple;
    let scale = window.raster_scale_factor();
    let padding = bulge.abs() + ripple.abs() + sampling_reach.max(px(0.)) + px(1. / scale);
    let light = uniforms.slots()[LIGHT_SLOT];
    let center = bounds.center();
    let uniforms = uniforms
        .with_slot(
            LIGHT_SLOT,
            [light[0], light[1], light[2], center.x.as_f32() * scale],
        )
        .with_slot(
            SHAPE_SLOT,
            [
                bounds.size.width.as_f32() * scale,
                bounds.size.height.as_f32() * scale,
                1. + deformation.ellipticity,
                center.y.as_f32() * scale,
            ],
        )
        .with_slot(
            DEFORMATION_SLOT,
            [
                deformation.focus.x.clamp(0., 1.),
                deformation.focus.y.clamp(0., 1.),
                bulge.as_f32() * scale,
                ripple.as_f32() * scale,
            ],
        );
    let merged = uniforms.slots()[12];
    let pane = uniforms.slots()[11];
    let mut capture = bounds;
    if merged[2] > 0. || (pane[1] > 0. && pane[2] > 0.) {
        let half_size = if pane[1] > 0. && pane[2] > 0. {
            gpui::size(px(pane[1] * 0.5 / scale), px(pane[2] * 0.5 / scale))
        } else {
            gpui::size(px(merged[2] / scale), px(merged[2] / scale))
        };
        let other_center = center + point(px(merged[0] / scale), px(merged[1] / scale));
        let other = Bounds::new(
            other_center - point(half_size.width, half_size.height),
            gpui::size(half_size.width * 2., half_size.height * 2.),
        );
        capture = capture.union(&other).dilate(px(merged[3] * 0.25 / scale));
    }
    let paint_bounds = capture.dilate(padding);
    let shadow = uniforms.slots()[13];
    if shadow[0] > 0. && shadow[1] > 0. {
        let shadow_padding =
            px((shadow[0] * 1.5 + shadow[2].abs().max(shadow[3].abs()) * 1.6) / scale);
        window.paint_backdrop_effect(
            PaintBackdropEffect::new(
                capture.dilate(bulge.abs() + ripple.abs() + shadow_padding),
                px(0.),
                liquid_glass_shadow_shader(),
            )
            .uniforms(uniforms),
        );
    }
    window.paint_backdrop_effect(
        PaintBackdropEffect::new(paint_bounds, blur_radius.max(px(0.)), shader).uniforms(uniforms),
    );
}

/// Contour shadow sampled by the glass background passes.
pub fn liquid_glass_shadow_shader() -> BackdropShader {
    BackdropShader::wgsl(concat!(
        include_str!("shaders/liquid_glass_shape.wgsl"),
        "\n",
        include_str!("shaders/liquid_glass_shadow.wgsl"),
    ))
}

/// Portable backdrop shader used by [`LiquidGlass`].
pub fn liquid_glass_shader() -> BackdropShader {
    BackdropShader::wgsl(concat!(
        include_str!("shaders/liquid_glass_shape.wgsl"),
        "\n",
        include_str!("shaders/liquid_glass.wgsl"),
        "\n",
        "fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> { return liquid_glass_material(input, params); }"
    )).with_mipmapped_backdrop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_parameters_scale_geometry_but_not_color() {
        let appearance = LiquidGlassStyle::optical()
            .refraction(px(5.))
            .thickness(px(12.))
            .dispersion(0.03);
        let uniforms = appearance.uniforms(Corners::all(px(3.)), 2.);
        assert_eq!(uniforms.slots()[OPTICS_SLOT], [2.2, 1., 10., 24.]);
        assert_eq!(uniforms.slots()[CORNERS_SLOT], [6.; 4]);
        assert_eq!(uniforms.slots()[SURFACE_SLOT], [0., 0., 0.03, 0.35 / 2.2]);
        assert_eq!(uniforms.slots()[8][0], appearance.optics.rim_strength);
    }

    #[test]
    fn container_supports_styles_children_and_interaction() {
        let glass = LiquidGlass::new()
            .id("glass-button")
            .w(px(160.0))
            .rounded_full()
            .p_2()
            .text_color(gpui::rgb(0xffffff))
            .cursor_pointer()
            .on_click(|_, _, _| {})
            .child("Play");
        let _: gpui::ViewElement<LiquidGlass> = glass.into_element();
    }

    #[test]
    fn glass_shaders_are_valid_wgsl() {
        for shader in [
            liquid_glass_shader(),
            liquid_glass_shadow_shader(),
            crate::fluted_glass_shader(),
        ] {
            let source = gpui::compose_backdrop_shader_wgsl(&shader);
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("glass shader must validate");
            let (_, translation) = naga::back::msl::write_string(
                &module,
                &info,
                &naga::back::msl::Options {
                    lang_version: (2, 0),
                    ..Default::default()
                },
                &naga::back::msl::PipelineOptions::default(),
            )
            .expect("glass shader must translate to MSL");
            assert!(translation.entry_point_names.iter().all(Result::is_ok));
        }
    }
}
