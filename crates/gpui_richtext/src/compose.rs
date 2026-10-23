use crate::{
    Code, Document, Emphasis, Group, HardBreak, Link, Mark, Node, NodeId, Paragraph, Span,
    Strikethrough, Strong, Text, Underline,
};
use anyhow::{Result, bail};
use gpui::{HighlightStyle, SharedString};
use std::{any::Any, sync::Arc};

pub fn code_block(text: impl Into<SharedString>) -> NodeTree {
    node(crate::CodeBlock::new(text))
}

/// A composable node subtree. Components describe content, not independent views.
pub struct NodeTree {
    payload: Arc<dyn Any + Send + Sync>,
    children: Vec<NodeTree>,
}

impl NodeTree {
    pub fn child(mut self, child: NodeTree) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = NodeTree>) -> Self {
        self.children.extend(children);
        self
    }
}

/// Compose any registered payload type, including application-defined types.
pub fn node(payload: impl Any + Send + Sync) -> NodeTree {
    NodeTree {
        payload: Arc::new(payload),
        children: Vec::new(),
    }
}

pub fn text(value: impl Into<SharedString>) -> NodeTree {
    node(Text(value.into()))
}
pub fn paragraph() -> NodeTree {
    node(Paragraph)
}
pub fn group() -> NodeTree {
    node(Group)
}
pub fn link(target: impl Into<SharedString>) -> NodeTree {
    node(Link::new(target))
}
pub fn link_with_text(target: impl Into<SharedString>, text: impl Into<SharedString>) -> NodeTree {
    node(Link::new(target).text(text))
}
pub fn span(style: HighlightStyle) -> NodeTree {
    node(Span(style))
}
pub fn strong() -> NodeTree {
    node(Strong)
}
pub fn emphasis() -> NodeTree {
    node(Emphasis)
}
pub fn underline() -> NodeTree {
    node(Underline)
}
pub fn strikethrough() -> NodeTree {
    node(Strikethrough)
}
pub fn mark() -> NodeTree {
    node(Mark)
}
pub fn code() -> NodeTree {
    node(Code)
}
pub fn hard_break() -> NodeTree {
    node(HardBreak)
}

impl Document {
    /// Build a snapshot from composed roots, assigning IDs in reading order.
    /// Generated IDs are local to this snapshot and are not stable across edits.
    /// Use `Document::new` when the application supplies persistent node identities.
    pub fn from_roots(roots: impl IntoIterator<Item = NodeTree>) -> Result<Self> {
        let mut pending: Vec<_> = roots.into_iter().map(|root| (root, None, 0)).collect();
        pending.reverse();
        let mut nodes: Vec<Node> = Vec::new();
        let mut roots = Vec::new();
        while let Some((tree, parent, depth)) = pending.pop() {
            if nodes.len() >= 100_000 || depth > 256 {
                bail!("document exceeds node count or nesting limit");
            }
            let index = nodes.len();
            let id = NodeId(index as u64 + 1);
            if let Some(parent) = parent {
                let parent: &mut Node = &mut nodes[parent];
                parent.children.push(id);
            } else {
                roots.push(id);
            }
            nodes.push(Node {
                id,
                payload: tree.payload,
                children: Vec::new(),
            });
            pending.extend(
                tree.children
                    .into_iter()
                    .rev()
                    .map(|child| (child, Some(index), depth + 1)),
            );
        }
        Self::new(nodes, roots)
    }
}
