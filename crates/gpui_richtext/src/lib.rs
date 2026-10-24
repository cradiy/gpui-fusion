//! Extensible read-only text documents and a selectable GPUI presentation.
//!
//! Headings support distinct typography metrics; table cells wrap independently.
//! Images render as blocks; arbitrary inline objects and virtualization are not provided.

mod anchor;
mod blocks;
pub use anchor::{Anchor, AnchorLink, AnchorSpan, Footnote, FootnoteBacklink, LinkKind};
mod code_block;
pub mod compose;
mod containers;
mod document;
#[cfg(feature = "markdown")]
pub mod markdown;
mod projection;
mod styles;
#[cfg(feature = "syntax-highlighting")]
pub mod syntax;
mod table;
mod theme;
mod view;
pub use blocks::{BlockQuote, Image, ListEntry, Rule};

pub use blocks::{
    Heading, HeadingLevel, HeadingStyle, ListItem, ListMarker, Quote, TaskMarkerStyle,
};
pub use code_block::{CodeBlock, CodeBlockLabels, CodeBlockStyle};
pub use compose::NodeTree;
pub use containers::{
    Alert, AlertKind, AlertStyle, DefinitionDescription, DefinitionList, DefinitionTerm,
};
pub use document::{Document, Node, NodeId};
pub use projection::{
    BlockKind, BlockSpan, Contribution, LinkSpan, Paragraph, Projection, Registry, Span, Text,
    TextSpan,
};
pub use styles::{
    Code, Emphasis, Group, HardBreak, InlineStyle, Link, Mark, Strikethrough, Strong, TextTheme,
    Underline,
};
pub use table::{
    CellVerticalAlign, ColumnWidth, Table, TableBlock, TableCell, TableCellSpan, TableColumn,
    TableRow, TableRowSpan, TableStyle,
};
pub use theme::RichTextTheme;
pub use view::RichText;
