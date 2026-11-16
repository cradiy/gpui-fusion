use crate::components::dropdown::{DropdownAlign, DropdownPlacement, DropdownSide};
use gpui::{Bounds, Pixels, Point, Size, point, px};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverSide {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Preferred placement; flips to the opposite side and shifts inside the viewport as needed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverPlacement {
    side: PopoverSide,
    align: PopoverAlign,
    gap: Pixels,
    offset: Point<Pixels>,
}
impl Default for PopoverPlacement {
    fn default() -> Self {
        Self::new(PopoverSide::Bottom)
    }
}
impl PopoverPlacement {
    pub fn new(side: PopoverSide) -> Self {
        Self {
            side,
            align: PopoverAlign::Start,
            gap: px(10.),
            offset: Point::default(),
        }
    }
    pub fn align(mut self, align: PopoverAlign) -> Self {
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
    pub(super) fn resolve(
        self,
        anchor: Bounds<Pixels>,
        size: Size<Pixels>,
        viewport: Size<Pixels>,
        margin: Pixels,
    ) -> (Bounds<Pixels>, Point<Pixels>) {
        let inset = point(margin, margin);
        let placement = DropdownPlacement::new(match self.side {
            PopoverSide::Top => DropdownSide::Top,
            PopoverSide::Bottom => DropdownSide::Bottom,
            PopoverSide::Left => DropdownSide::Left,
            PopoverSide::Right => DropdownSide::Right,
        })
        .align(match self.align {
            PopoverAlign::Start => DropdownAlign::Start,
            PopoverAlign::Center => DropdownAlign::Center,
            PopoverAlign::End => DropdownAlign::End,
        })
        .gap(self.gap)
        .offset(self.offset);
        let (mut bounds, side) = placement.resolve(
            Bounds::new(anchor.origin - inset, anchor.size),
            size,
            gpui::size(
                (viewport.width - margin * 2.).max(px(0.)),
                (viewport.height - margin * 2.).max(px(0.)),
            ),
        );
        bounds.origin += inset;
        let direction = match side {
            DropdownSide::Top => point(px(0.), px(1.)),
            DropdownSide::Bottom => point(px(0.), px(-1.)),
            DropdownSide::Left => point(px(1.), px(0.)),
            DropdownSide::Right => point(px(-1.), px(0.)),
            DropdownSide::Overlay => Point::default(),
        };
        (bounds, direction)
    }
}
