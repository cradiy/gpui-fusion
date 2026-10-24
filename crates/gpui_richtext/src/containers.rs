use gpui::{Hsla, SharedString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// A semantic callout containing block nodes. Titles are supplied by the application.
#[derive(Clone, Debug)]
pub struct Alert {
    pub kind: AlertKind,
    pub title: Option<SharedString>,
}

impl Alert {
    pub fn new(kind: AlertKind) -> Self {
        Self { kind, title: None }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

#[derive(Clone)]
pub struct AlertStyle {
    pub accent: Hsla,
    pub background: Hsla,
    pub header_background: Hsla,
    pub header_border: Hsla,
    pub header_icon: Option<SharedString>,
    /// Localized title used when a node has no explicit title.
    pub title: Option<SharedString>,
}

/// A sequence of terms followed by one or more descriptions per term.
pub struct DefinitionList;
/// Inline content naming a term.
pub struct DefinitionTerm;
/// Block content describing the preceding term.
pub struct DefinitionDescription;
