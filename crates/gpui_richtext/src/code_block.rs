use gpui::{HighlightStyle, SharedString};
use std::ops::Range;

/// Surface styling for code blocks, independent of syntax highlighting.
#[derive(Clone)]
pub struct CodeBlockStyle {
    pub background: gpui::Hsla,
    pub border_color: gpui::Hsla,
    pub corner_radius: gpui::Pixels,
    pub padding: gpui::Pixels,
}

impl Default for CodeBlockStyle {
    fn default() -> Self {
        Self {
            background: gpui::rgba(0x64748b18).into(),
            border_color: gpui::rgba(0x64748b30).into(),
            corner_radius: gpui::px(8.),
            padding: gpui::px(12.),
        }
    }
}

/// A read-only block of source text. No parser or syntax engine is required.
#[derive(Clone)]
pub struct CodeBlock {
    pub text: SharedString,
    pub language: Option<SharedString>,
    /// Sorted, non-overlapping UTF-8 byte ranges into `text`.
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
}

impl CodeBlock {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            language: None,
            highlights: Vec::new(),
        }
    }

    /// Metadata for application-provided syntax highlighting and block tooling.
    pub fn language(mut self, language: impl Into<SharedString>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Apply syntax colors supplied by the application. Ranges are validated
    /// during compilation; source text is never rewritten by highlighting.
    pub fn highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        self.highlights = highlights.into_iter().collect();
        self
    }
}
