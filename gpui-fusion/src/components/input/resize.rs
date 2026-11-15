use gpui::{Bounds, CursorStyle, Pixels, Point, Size, px, size};

/// Axes that a multiline field's resize handle can change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputResize {
    #[default]
    Disabled,
    Vertical,
    Horizontal,
    Both,
}
impl InputResize {
    pub(super) fn cursor(self) -> CursorStyle {
        match self {
            Self::Vertical => CursorStyle::ResizeUpDown,
            Self::Horizontal => CursorStyle::ResizeLeftRight,
            Self::Both => CursorStyle::ResizeUpRightDownLeft,
            Self::Disabled => CursorStyle::Arrow,
        }
    }
}

pub(super) struct ResizeState {
    pub axes: InputResize,
    pub bounds: Bounds<Pixels>,
    pub dimensions: Option<Size<Pixels>>,
    pub dragging: Option<(Point<Pixels>, Size<Pixels>)>,
    min: Size<Pixels>,
    max: Size<Pixels>,
}
impl Default for ResizeState {
    fn default() -> Self {
        Self {
            axes: InputResize::Disabled,
            bounds: Bounds::default(),
            dimensions: None,
            dragging: None,
            min: size(px(120.), px(64.)),
            max: size(px(4096.), px(2048.)),
        }
    }
}
impl ResizeState {
    pub fn limits(&mut self, min: Size<Pixels>, max: Size<Pixels>) {
        assert!(
            [min.width, min.height, max.width, max.height]
                .iter()
                .all(|value| value.as_f32().is_finite() && *value > px(0.)),
            "input resize limits must be finite and positive"
        );
        assert!(
            min.width <= max.width && min.height <= max.height,
            "input resize minimum must not exceed maximum"
        );
        self.min = min;
        self.max = max;
    }
    pub fn begin(&mut self, pointer: Point<Pixels>) {
        self.dragging = Some((pointer, self.bounds.size));
    }
    pub fn move_to(&mut self, pointer: Point<Pixels>) -> bool {
        let Some((origin, initial)) = self.dragging else {
            return false;
        };
        let delta = pointer - origin;
        let dimensions = size(
            if matches!(self.axes, InputResize::Horizontal | InputResize::Both) {
                (initial.width + delta.x).clamp(self.min.width, self.max.width)
            } else {
                initial.width
            },
            if matches!(self.axes, InputResize::Vertical | InputResize::Both) {
                (initial.height + delta.y).clamp(self.min.height, self.max.height)
            } else {
                initial.height
            },
        );
        if self.dimensions == Some(dimensions) {
            return false;
        }
        self.dimensions = Some(dimensions);
        true
    }
}
