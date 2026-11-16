use gpui::{Bounds, Pixels, Point, Size, point, px};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropdownSide {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
    Overlay,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropdownAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Preferred placement in logical pixels. Opposite-side fallback and viewport fitting are automatic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropdownPlacement {
    side: DropdownSide,
    align: DropdownAlign,
    gap: Pixels,
    offset: Point<Pixels>,
}
impl Default for DropdownPlacement {
    fn default() -> Self {
        Self::new(DropdownSide::Bottom)
    }
}
impl DropdownPlacement {
    pub fn new(side: DropdownSide) -> Self {
        Self {
            side,
            align: DropdownAlign::Start,
            gap: px(6.),
            offset: point(px(0.), px(0.)),
        }
    }
    pub fn align(mut self, align: DropdownAlign) -> Self {
        self.align = align;
        self
    }
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap.max(px(0.));
        self
    }
    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = offset;
        self
    }
    pub(crate) fn bounds(
        self,
        anchor: Bounds<Pixels>,
        size: Size<Pixels>,
        viewport: Size<Pixels>,
    ) -> Bounds<Pixels> {
        self.resolve(anchor, size, viewport).0
    }

    pub(super) fn resolve(
        self,
        anchor: Bounds<Pixels>,
        size: Size<Pixels>,
        viewport: Size<Pixels>,
    ) -> (Bounds<Pixels>, DropdownSide) {
        let factor = match self.align {
            DropdownAlign::Start => 0.,
            DropdownAlign::Center => 0.5,
            DropdownAlign::End => 1.,
        };
        let x = anchor.left() + (anchor.size.width - size.width) * factor;
        let y = anchor.top() + (anchor.size.height - size.height) * factor;
        let position = |side| match side {
            DropdownSide::Bottom => point(x, anchor.bottom() + self.gap),
            DropdownSide::Top => point(x, anchor.top() - self.gap - size.height),
            DropdownSide::Left => point(anchor.left() - self.gap - size.width, y),
            DropdownSide::Right => point(anchor.right() + self.gap, y),
            DropdownSide::Overlay => point(x, y),
        } + self.offset;
        let opposite = match self.side {
            DropdownSide::Bottom => DropdownSide::Top,
            DropdownSide::Top => DropdownSide::Bottom,
            DropdownSide::Left => DropdownSide::Right,
            DropdownSide::Right => DropdownSide::Left,
            DropdownSide::Overlay => DropdownSide::Overlay,
        };
        let overflow = |origin: Point<Pixels>| {
            (-origin.x).max(px(0.))
                + (-origin.y).max(px(0.))
                + (origin.x + size.width - viewport.width).max(px(0.))
                + (origin.y + size.height - viewport.height).max(px(0.))
        };
        let preferred = position(self.side);
        let fallback = position(opposite);
        let (origin, side) = if overflow(fallback) < overflow(preferred) {
            (fallback, opposite)
        } else {
            (preferred, self.side)
        };
        (
            Bounds {
                origin: point(
                    origin
                        .x
                        .clamp(px(0.), (viewport.width - size.width).max(px(0.))),
                    origin
                        .y
                        .clamp(px(0.), (viewport.height - size.height).max(px(0.))),
                ),
                size,
            },
            side,
        )
    }
}
