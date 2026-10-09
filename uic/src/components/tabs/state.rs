use gpui::{Bounds, Context, FocusHandle, Pixels, ScrollHandle, Window, point, px};

pub(super) struct State {
    pub scroll: ScrollHandle,
    pub focus: FocusHandle,
    pub selected: Option<usize>,
    pub geometry: Option<(Pixels, Pixels, Pixels)>,
}

impl State {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            selected: None,
            geometry: None,
        }
    }

    pub fn reveal(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let viewport = self.scroll.bounds();
        let offset = self.scroll.offset();
        let geometry = (
            viewport.size.width,
            bounds.left() - viewport.left() - offset.x,
            bounds.size.width,
        );
        if self.geometry.is_some_and(|previous| {
            (previous.0 - geometry.0).abs() < px(0.5)
                && (previous.1 - geometry.1).abs() < px(0.5)
                && (previous.2 - geometry.2).abs() < px(0.5)
        }) {
            return;
        }
        self.geometry = Some(geometry);
        let delta = if bounds.size.width > viewport.size.width || bounds.left() < viewport.left() {
            viewport.left() - bounds.left()
        } else if bounds.right() > viewport.right() {
            viewport.right() - bounds.right()
        } else {
            px(0.)
        };
        if delta.abs() > px(0.5) {
            let scroll = self.scroll.clone();
            window.defer(cx, move |window, _| {
                scroll.set_offset(point(offset.x + delta, offset.y));
                window.refresh();
            });
        }
    }
}
