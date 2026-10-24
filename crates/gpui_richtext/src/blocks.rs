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

/// Heading metrics relative to the inherited body font size.
#[derive(Clone, Copy, Debug)]
pub struct HeadingStyle {
    pub font_scale: f32,
    /// Line height as a multiple of the heading's font size.
    pub line_height: f32,
}

impl HeadingStyle {
    pub fn defaults() -> [Self; 6] {
        [2., 1.65, 1.35, 1.15, 1., 0.9].map(|font_scale| Self {
            font_scale,
            line_height: 1.3,
        })
    }
}

/// A quoted paragraph containing inline children.
pub struct Quote;

/// A quotation containing block nodes, including nested quotations.
pub struct BlockQuote;
/// A list entry containing paragraphs and nested block nodes.
pub struct ListEntry(pub ListMarker);
/// A thematic break between blocks.
pub struct Rule;
/// A block image loaded through the application's GPUI asset and HTTP sources.
#[derive(Clone, Debug)]
pub struct Image {
    pub source: gpui::SharedString,
    pub alt: gpui::SharedString,
    pub link: Option<gpui::SharedString>,
    pub title: Option<gpui::SharedString>,
}

impl Image {
    pub fn new(source: impl Into<gpui::SharedString>, alt: impl Into<gpui::SharedString>) -> Self {
        Self {
            source: source.into(),
            alt: alt.into(),
            link: None,
            title: None,
        }
    }

    pub fn link(mut self, target: impl Into<gpui::SharedString>) -> Self {
        self.link = Some(target.into());
        self
    }

    pub fn title(mut self, title: impl Into<gpui::SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

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
