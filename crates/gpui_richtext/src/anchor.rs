use gpui::SharedString;

/// A named inline destination. Children contribute selectable content.
pub struct Anchor(pub SharedString);

/// An inline link to a named destination within the same rich-text view.
pub struct AnchorLink {
    pub target: SharedString,
}

/// A numbered footnote containing block content.
pub struct Footnote {
    pub number: u32,
}

/// Footnote metrics relative to the inherited body font size.
#[derive(Clone, Copy)]
pub struct FootnoteStyle {
    pub font_scale: f32,
    pub line_height: f32,
}

impl Default for FootnoteStyle {
    fn default() -> Self {
        Self {
            font_scale: 0.875,
            line_height: 1.55,
        }
    }
}

/// A return link to a footnote's reference, with separate theme styling.
pub struct FootnoteBacklink {
    pub target: SharedString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    External,
    Anchor,
}

#[derive(Clone, Debug)]
pub struct AnchorSpan {
    pub name: SharedString,
    pub offset: usize,
}
