use super::placement::{DropdownPlacement, DropdownSide};
use crate::style::{ComputedStyle, Resolved};
use gpui::{prelude::*, *};

/// Controls whether the trigger remains visible while its menu is expanded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropdownPresentation {
    /// Keeps the button visible and opens a separate anchored panel.
    #[default]
    Popover,
    /// Hides the button as its panel opens and restores it on close.
    Morph,
}

pub(super) struct MorphMenu {
    pub content: AnyElement,
    pub trigger_content: AnyElement,
    pub anchor: Bounds<Pixels>,
    pub progress: f32,
    pub placement: DropdownPlacement,
    pub trigger_radius: Pixels,
    pub style: ComputedStyle,
}
impl IntoElement for MorphMenu {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for MorphMenu {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (AnyElement, AnyElement, Bounds<Pixels>);
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let target_size = window.layout_bounds(*layout).size;
        let target_origin = self
            .placement
            .bounds(self.anchor, target_size, window.viewport_size())
            .origin;
        let p = self.progress;
        let bounds = Bounds {
            origin: self.anchor.origin + (target_origin - self.anchor.origin) * p,
            size: size(
                self.anchor.size.width + (target_size.width - self.anchor.size.width) * p,
                self.anchor.size.height + (target_size.height - self.anchor.size.height) * p,
            ),
        };
        let mut refinement = self.style.to_gpui();
        let radius = refinement
            .corner_radii
            .top_left
            .unwrap_or_default()
            .to_pixels(window.rem_size());
        refinement = refinement.rounded(self.trigger_radius + (radius - self.trigger_radius) * p);
        let glass = self.style.material;
        let surface_style = self.style.clone();
        let mut background = div().on_paint_before_children(move |bounds, style, window, _| {
            paint_glass_surface(
                &surface_style,
                bounds,
                style.corner_radii.to_pixels(window.rem_size()),
                window,
            );
        });
        background.style().refine(&refinement);
        background = background.w(bounds.size.width).h(bounds.size.height);
        if glass.is_some() {
            background = background
                .bg(transparent_black())
                .border_color(transparent_black());
        } else {
            background = background.shadow_md();
        }
        let mut background = background.into_any_element();
        background.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            window,
            cx,
        );
        background.prepaint_at(bounds.origin, window, cx);
        let mut trigger = div()
            .w(self.anchor.size.width)
            .h(self.anchor.size.height)
            .opacity((1. - p * 3.).max(0.))
            .child(std::mem::replace(
                &mut self.trigger_content,
                div().into_any_element(),
            ))
            .into_any_element();
        trigger.layout_as_root(
            size(
                AvailableSpace::Definite(self.anchor.size.width),
                AvailableSpace::Definite(self.anchor.size.height),
            ),
            window,
            cx,
        );
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            trigger.prepaint_at(bounds.origin, window, cx);
            self.content.prepaint_at(bounds.origin, window, cx);
        });
        (background, trigger, bounds)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.0.paint(window, cx);
        window.with_content_mask(Some(ContentMask { bounds: state.2 }), |window| {
            state.1.paint(window, cx);
            self.content.paint(window, cx);
        });
    }
}

pub(super) fn paint_glass_surface(
    style: &ComputedStyle,
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
    window: &mut Window,
) {
    let Some(glass) = style.material else {
        return;
    };
    gpui_effects::paint_liquid_glass(bounds, corners, glass, window);
    let background = style
        .background_gradient
        .clone()
        .unwrap_or_else(|| match style.background {
            Resolved::Value(color) => color.into(),
            _ => transparent_black().into(),
        });
    window.paint_quad(quad(
        bounds,
        corners,
        background,
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    ));
}

pub(crate) struct PlacedMenu {
    pub content: AnyElement,
    pub settled_trigger: AnyElement,
    pub anchor: Bounds<Pixels>,
    pub placement: DropdownPlacement,
    pub progress: f32,
    pub style: ComputedStyle,
}
impl IntoElement for PlacedMenu {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PlacedMenu {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (AnyElement, Bounds<Pixels>);
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let size = window.layout_bounds(*layout).size;
        let trigger_size = self.settled_trigger.layout_as_root(
            gpui::size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
            window,
            cx,
        );
        let (target, side) = self.placement.resolve(
            Bounds::new(self.anchor.origin, trigger_size),
            size,
            window.viewport_size(),
        );
        let mut bounds = target;
        let progress = self.progress.clamp(0., 1.);
        match side {
            DropdownSide::Bottom | DropdownSide::Top => {
                bounds.size.height *= progress;
                if side == DropdownSide::Top {
                    bounds.origin.y = target.bottom() - bounds.size.height;
                }
            }
            DropdownSide::Left | DropdownSide::Right => {
                bounds.size.width *= progress;
                if side == DropdownSide::Left {
                    bounds.origin.x = target.right() - bounds.size.width;
                }
            }
            DropdownSide::Overlay => {
                bounds.size.width *= progress;
                bounds.size.height *= progress;
                bounds.origin =
                    target.center() - point(bounds.size.width * 0.5, bounds.size.height * 0.5);
            }
        }
        let mut background = div();
        background.style().refine(&self.style.to_gpui());
        let mut background = background
            .w(bounds.size.width)
            .h(bounds.size.height)
            .opacity(progress * self.style.to_gpui().opacity.unwrap_or(1.))
            .shadow_md()
            .into_any_element();
        background.layout_as_root(
            gpui::size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            window,
            cx,
        );
        background.prepaint_at(bounds.origin, window, cx);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.content.prepaint_at(target.origin, window, cx);
        });
        (background, bounds)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.0.paint(window, cx);
        window.with_content_mask(Some(ContentMask { bounds: state.1 }), |window| {
            self.content.paint(window, cx);
        });
    }
}
