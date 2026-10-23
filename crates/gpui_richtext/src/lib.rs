//! Extensible read-only text documents and a selectable GPUI presentation.
//!
//! This experimental API currently supports uniform-size text flow. It does not
//! yet provide embedded objects, independent block layout, or virtualization.

mod code_block;
pub mod compose;
mod document;
mod projection;
mod styles;
mod view;

pub use code_block::{CodeBlock, CodeBlockStyle};
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
pub use view::RichText;
