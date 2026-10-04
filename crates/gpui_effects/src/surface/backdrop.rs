use gpui::prelude::*;
use gpui::{
    App, BackdropShader, Bounds, EffectUniforms, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, PaintBackdropEffect, Pixels, Point, Style,
    StyleRefinement, Styled, Window,
};

/// Constructs a styled element that composites a custom shader over scene
/// content already painted behind it.
pub fn backdrop_effect(shader: BackdropShader) -> BackdropEffect {
    BackdropEffect::new(shader)
}

/// Returns the shader used by [`crate::FrostedGlass`].
pub fn frosted_glass_shader() -> BackdropShader {
    BackdropShader::wgsl(include_str!("shaders/frosted_glass.wgsl"))
}

/// A styled leaf element that samples both raw and blurred scene content.
///
/// The renderer prepares the backdrop inputs; the shader handles only material
/// compositing. A normal style background and border are painted afterwards,
/// so they can provide a tint and a graceful fallback on unsupported backends.
pub struct BackdropEffect {
    shader: BackdropShader,
    uniforms: EffectUniforms,
    time: f32,
    blur_radius: Pixels,
    opacity: f32,
    style: StyleRefinement,
}

impl BackdropEffect {
    /// Creates a backdrop element from a portable shader.
    pub fn new(shader: BackdropShader) -> Self {
        Self {
            shader,
            uniforms: EffectUniforms::default(),
            time: 0.0,
            blur_radius: gpui::px(0.0),
            opacity: 1.0,
            style: StyleRefinement::default(),
        }
    }

    /// Replaces all user-defined uniform slots.
    pub fn uniforms(mut self, uniforms: EffectUniforms) -> Self {
        self.uniforms = uniforms;
        self
    }

    /// Sets one four-component user uniform slot.
    pub fn uniform(mut self, index: usize, value: [f32; 4]) -> Self {
        self.uniforms.set_slot(index, value);
        self
    }

    /// Sets the radius used to prepare the blurred backdrop input.
    pub fn blur_radius(mut self, radius: Pixels) -> Self {
        self.blur_radius = radius;
        self
    }

    /// Sets normalized animation time supplied to the shader.
    pub fn time(mut self, time: f32) -> Self {
        self.time = time;
        self
    }

    /// Sets opacity applied after shader evaluation.
    pub fn effect_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Returns the shader backing this element.
    pub fn shader(&self) -> &BackdropShader {
        &self.shader
    }
}

impl IntoElement for BackdropEffect {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for BackdropEffect {
    type RequestLayoutState = Style;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let corner_radii = style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(bounds.size);
        let mouse = window.mouse_position();
        let pointer_active = bounds.contains(&mouse);
        let width = bounds.size.width.as_f32().max(f32::EPSILON);
        let height = bounds.size.height.as_f32().max(f32::EPSILON);
        let pointer = Point {
            x: (mouse.x.as_f32() - bounds.origin.x.as_f32()) / width,
            y: (mouse.y.as_f32() - bounds.origin.y.as_f32()) / height,
        };

        let effect = PaintBackdropEffect::new(bounds, self.blur_radius, self.shader.clone())
            .uniforms(self.uniforms)
            .time(self.time)
            .pointer(pointer, pointer_active)
            .corner_radii(corner_radii)
            .opacity(self.opacity);
        window.paint_backdrop_effect(effect);

        // Paint conventional tint, shadow, and border over the material. This
        // also leaves a useful translucent panel when backdrop shaders are not
        // supported by the active renderer.
        style.paint(bounds, window, cx, |_window, _cx| {});
    }
}

impl Styled for BackdropEffect {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frosted_shader_identity_is_stable() {
        assert_eq!(frosted_glass_shader().id(), frosted_glass_shader().id());
    }
}
