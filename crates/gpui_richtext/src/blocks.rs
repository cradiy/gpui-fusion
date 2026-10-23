/// Semantic heading depth. Typography is configured through `TextTheme::headings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingLevel {
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// A heading containing inline children.
pub struct Heading(pub HeadingLevel);

/// A quoted paragraph containing inline children.
pub struct Quote;

/// A read-only list marker included in projected and copied text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
    Bullet,
    Number(u32),
    Task { checked: bool },
}

impl ListMarker {
    pub(crate) fn text(self) -> String {
        match self {
            Self::Bullet => "• ".into(),
            Self::Number(number) => format!("{number}. "),
            Self::Task { checked: false } => "[ ] ".into(),
            Self::Task { checked: true } => "[x] ".into(),
        }
    }
}

/// One list paragraph. Numbering is supplied by the document producer.
pub struct ListItem(pub ListMarker);

/// Colors for read-only task markers, independent of the text font.
#[derive(Clone)]
pub struct TaskMarkerStyle {
    pub border: gpui::Hsla,
    pub checked_background: gpui::Hsla,
    pub check: gpui::Hsla,
}

impl Default for TaskMarkerStyle {
    fn default() -> Self {
        Self {
            border: gpui::rgb(0x94a3b8).into(),
            checked_background: gpui::rgb(0x25866b).into(),
            check: gpui::rgb(0xffffff).into(),
        }
    }
}
