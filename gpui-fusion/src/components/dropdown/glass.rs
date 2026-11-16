use super::{motion::phase, placement::DropdownPlacement};
use crate::style::{ComputedStyle, Resolved};
use gpui::{prelude::*, *};
use gpui_effects::{LiquidGlassPane, paint_liquid_glass_union};

pub(super) struct GlassMenu {
    pub content: AnyElement,
    pub trigger_content: AnyElement,
    pub anchor: Bounds<Pixels>,
    pub progress: f32,
    pub placement: DropdownPlacement,
    pub trigger_radius: Pixels,
    pub trigger_scale: [f32; 2],
    pub keep_trigger: bool,
    pub style: ComputedStyle,
}

pub(super) struct GlassFrame {
    panes: [LiquidGlassPane; 2],
    smoothness: Pixels,
    trigger: AnyElement,
}

impl IntoElement for GlassMenu {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for GlassMenu {
    type RequestLayoutState = LayoutId;
    type PrepaintState = GlassFrame;
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
    ) -> GlassFrame {
        let target = self.placement.bounds(
            self.anchor,
            window.layout_bounds(*layout).size,
            window.viewport_size(),
        );
        let p = self.progress;
        let center = self.anchor.center();
        let anchor_size = size(
            self.anchor.size.width * self.trigger_scale[0],
            self.anchor.size.height * self.trigger_scale[1],
        );
        let radius = (self.trigger_radius * self.trigger_scale[0].min(self.trigger_scale[1]))
            .min(anchor_size.height.min(anchor_size.width) * 0.5);
        let delta = target.center() - center;
        // The departure axis follows the fitted destination, including edge flips.
        let direction = if target.top() >= self.anchor.bottom() {
            point(0., 1.)
        } else if target.bottom() <= self.anchor.top() {
            point(0., -1.)
        } else if target.left() >= self.anchor.right() {
            point(1., 0.)
        } else if target.right() <= self.anchor.left() {
            point(-1., 0.)
        } else if delta.x.abs() / self.anchor.size.width > delta.y.abs() / self.anchor.size.height {
            point(delta.x.as_f32().signum(), 0.)
        } else {
            point(0., delta.y.as_f32().signum())
        };
        let reach =
            anchor_size.width * direction.x.abs() + anchor_size.height * direction.y.abs() + px(8.);
        let detached_center = center + point(reach * direction.x, reach * direction.y);
        let travel = phase(p, 0.02, 0.68);
        let grow = phase(p, 0.40, 1.);
        let moving_center = center + (detached_center - center) * travel;
        let capsule_origin =
            moving_center - point(anchor_size.width * 0.5, anchor_size.height * 0.5);
        let bounds = Bounds {
            origin: capsule_origin + (target.origin - capsule_origin) * grow,
            size: size(
                anchor_size.width + (target.size.width - anchor_size.width) * grow,
                anchor_size.height + (target.size.height - anchor_size.height) * grow,
            ),
        };
        let menu_radius = self
            .style
            .to_gpui()
            .corner_radii
            .top_left
            .unwrap_or_default()
            .to_pixels(window.rem_size());
        let remaining = if self.keep_trigger { 1. } else { 0. };
        let source_size = size(
            anchor_size.width * remaining,
            anchor_size.height * remaining,
        );
        let source = Bounds {
            origin: center - point(source_size.width * 0.5, source_size.height * 0.5),
            size: source_size,
        };
        let smoothness =
            self.anchor.size.height * (0.2 * phase(p, 0., 0.18) * (1. - phase(p, 0.44, 0.75)));
        let mut trigger = div()
            .w(self.anchor.size.width)
            .h(self.anchor.size.height)
            .opacity(if self.keep_trigger {
                1.
            } else {
                1. - phase(p, 0.02, 0.3)
            })
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
        trigger.prepaint_at(self.anchor.origin, window, cx);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.content.prepaint_at(bounds.origin, window, cx)
        });
        GlassFrame {
            panes: [
                LiquidGlassPane {
                    bounds: source,
                    radius: self.trigger_radius
                        * self.trigger_scale[0].min(self.trigger_scale[1])
                        * remaining,
                },
                LiquidGlassPane {
                    bounds,
                    radius: radius + (menu_radius - radius) * grow,
                },
            ],
            smoothness,
            trigger,
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        state: &mut GlassFrame,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(mut material) = self.style.material {
            if let Resolved::Value(color) = self.style.background {
                material.tint = color.composite_over(material.tint);
            }
            paint_liquid_glass_union(state.panes, state.smoothness, material, window);
        }
        state.trigger.paint(window, cx);
        window.with_content_mask(
            Some(ContentMask {
                bounds: state.panes[1].bounds,
            }),
            |window| self.content.paint(window, cx),
        );
    }
}
