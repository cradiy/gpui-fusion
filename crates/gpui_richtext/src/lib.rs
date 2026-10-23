//! Extensible read-only text documents and a selectable GPUI presentation.
//!
//! Text flows use uniform typography metrics; table cells wrap independently.
//! Embedded objects, arbitrary block layouts, and virtualization are not provided.

mod blocks;
mod code_block;
pub mod compose;
mod document;
mod projection;
mod styles;
mod table;
mod view;

pub use blocks::{Heading, HeadingLevel, ListItem, ListMarker, Quote, TaskMarkerStyle};
pub use code_block::{CodeBlock, CodeBlockLabels, CodeBlockStyle};
pub use compose::NodeTree;
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
pub use view::RichText;
