use gpui::{prelude::*, *};

use super::ModalLayer;

pub(super) struct SheetFrame {
    pub panel: AnyElement,
    pub layer: WeakEntity<ModalLayer>,
    pub expansion: f32,
    pub drag_offset: Pixels,
    pub motion: f32,
}

impl IntoElement for SheetFrame {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for SheetFrame {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: size(relative(1.).into(), relative(1.).into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Percentage heights resolve intrinsically under MaxContent, then fill the chosen detent.
        let natural = self.panel.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::MaxContent,
            ),
            window,
            cx,
        );
        let collapsed = natural.height.min(bounds.size.height * 0.72);
        let gap = bounds.size.height - collapsed;
        let desired = collapsed + gap * self.expansion - self.drag_offset;
        let height = desired.clamp(collapsed, bounds.size.height);
        let offset = (collapsed - desired).max(px(0.));
        let measured = self.panel.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(height),
            ),
            window,
            cx,
        );
        let _ = self.layer.update(cx, |layer, _| {
            layer.sheet_height = height;
            layer.sheet_collapsed_height = collapsed;
            layer.sheet_max_height = bounds.size.height;
        });
        self.panel.prepaint_at(
            point(
                bounds.origin.x + (bounds.size.width - measured.width) / 2.,
                bounds.bottom() - height + offset + height * self.motion,
            ),
            window,
            cx,
        );
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.panel.paint(window, cx);
    }
}
