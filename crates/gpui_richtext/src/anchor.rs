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
