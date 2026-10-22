//! Extensible read-only text documents and a selectable GPUI presentation.
//!
//! This experimental API currently supports uniform-size text flow. It does not
//! yet provide embedded objects, independent block layout, or virtualization.

mod document;
mod projection;
mod view;

pub use document::{Document, Node, NodeId};
pub use projection::{Contribution, Paragraph, Projection, Registry, Span, Text, TextSpan};
pub use view::RichText;
