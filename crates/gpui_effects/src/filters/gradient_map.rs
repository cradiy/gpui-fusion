use gpui::{
    A11ySubtreeBuilder, AnyElement, App, Background, Bounds, ColorSpace, EffectShader,
    EffectUniforms, Element, ElementId, GlobalElementId, InspectorElementId, InteractiveElement,
    IntoElement, LayoutId, LinearColorStop, ParentElement, Pixels, Rgba, StyleRefinement, Styled,
    SubtreeInput, Window, fill, multi_linear_gradient,
};

/// Editable color ramp backed by GPUI's standard shared gradient storage.
#[derive(Clone, Debug)]
pub struct GradientMapPalette(Background);

impl GradientMapPalette {
    /// Stops must be ordered in 0..=1, with at least two entries.
    pub fn new(stops: impl AsRef<[LinearColorStop]>) -> Self {
        Self(multi_linear_gradient(90., stops))
    }

    /// Returns the ordered color stops.
    pub fn stops(&self) -> &[LinearColorStop] {
        self.0.gradient_stops()
    }

    /// Updates one stop using GPUI's copy-on-write gradient update path.
    /// Its position must stay between its neighbors. Notify the owning view after editing.
    pub fn set_stop(&mut self, index: usize, stop: LinearColorStop) {
        self.0.set_gradient_stop(index, stop);
    }

    /// Selects the standard GPUI color interpolation space.
    pub fn color_space(mut self, color_space: ColorSpace) -> Self {
        self.0 = self.0.color_space(color_space);
        self
    }

    /// Returns a horizontal background for displaying the same ramp in an editor.
    pub fn background(&self) -> Background {
        self.0.clone()
    }
}

/// Maps source luminance to an editable standard GPUI gradient.
pub fn subtree_gradient_map<E: IntoElement>(
    element: E,
    palette: GradientMapPalette,
) -> GradientMap<E::Element> {
    GradientMap {
        element: element.into_element(),
        gradient: palette.0,
        strength: 1.,
    }
}

/// A luminance map with caller-owned stops and unchanged layout and input geometry.
/// Opaque gradient colors preserve source alpha; translucent stops reduce it.
pub struct GradientMap<E: Element> {
    element: E,
    gradient: Background,
    strength: f32,
}

impl<E: Element> GradientMap<E> {
    /// Blends from the original at zero to the gradient map at one.
    /// Zero or a non-finite value bypasses capture.
    pub fn strength(mut self, strength: f32) -> Self {
        self.strength = if strength.is_finite() {
            strength.clamp(0., 1.)
        } else {
            0.
        };
        self
    }

    fn active(&self, window: &Window) -> bool {
        self.strength > 0. && window.supports_subtree_effects()
    }
}

impl<E: Element> IntoElement for GradientMap<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<E: Element> Element for GradientMap<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.element.id()
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        self.element.source_location()
    }
    fn a11y_role(&self) -> Option<accesskit::Role> {
        self.element.a11y_role()
    }
    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.element.write_a11y_info(node);
    }
    fn a11y_synthetic_children(
        &mut self,
        state: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        self.element.a11y_synthetic_children(state, builder);
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.element.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if self.active(window) {
            window.prepaint_subtree_effect(|window| {
                self.element
                    .prepaint(id, inspector, bounds, layout, window, cx)
            })
        } else {
            self.element
                .prepaint(id, inspector, bounds, layout, window, cx)
        }
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !self.active(window) {
            self.element
                .paint(id, inspector, bounds, layout, state, window, cx);
            return;
        }
        let stops = self.gradient.gradient_stops();
        let color = |color: gpui::Hsla| {
            let color: Rgba = color.into();
            [color.r, color.g, color.b, color.a]
        };
        let uniforms = EffectUniforms::new()
            .with_slot(0, [self.strength, 0., 0., 0.])
            .with_slot(1, color(stops[0].color))
            .with_slot(2, color(stops[stops.len() - 1].color));
        window.with_subtree_pair(
            bounds,
            gradient_map_shader(),
            uniforms,
            0.,
            1.,
            |input, window| match input {
                SubtreeInput::First => self
                    .element
                    .paint(id, inspector, bounds, layout, state, window, cx),
                SubtreeInput::Second => window.with_effect_source_bounds(bounds, |window| {
                    window.paint_quad(fill(bounds, self.gradient.clone()));
                }),
            },
        );
    }
}

impl<E: Element + Styled> Styled for GradientMap<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}
impl<E: Element + InteractiveElement> InteractiveElement for GradientMap<E> {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.element.interactivity()
    }
}
impl<E: Element + ParentElement> ParentElement for GradientMap<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.element.extend(elements);
    }
}

/// Two-image shader: source followed by a horizontal GPUI gradient.
/// Slot 0.x: strength; slots 1 and 2: straight RGBA at the dark and light endpoints.
pub fn gradient_map_shader() -> EffectShader {
    EffectShader::wgsl_two_images(include_str!("shaders/gradient_map.wgsl"))
}
