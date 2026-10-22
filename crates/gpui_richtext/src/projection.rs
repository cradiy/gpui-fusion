use crate::{Document, NodeId};
use anyhow::{Result, bail};
use gpui::{HighlightStyle, SharedString, StyledText};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::Range,
};

/// A text leaf. Its style inherits from ancestor contributions.
pub struct Text(pub SharedString);
/// An inline style container.
pub struct Span(pub HighlightStyle);
/// A paragraph containing text leaves and inline containers.
pub struct Paragraph;

/// The semantic contribution of a registered payload type.
pub enum Contribution {
    Text(SharedString, HighlightStyle),
    Inline(HighlightStyle),
    Paragraph(HighlightStyle),
}

type Resolver = Box<dyn Fn(&dyn Any) -> Result<Contribution>>;

/// View-specific typed node resolution. Unknown payloads are errors.
///
/// Resolving a document creates an immutable projection; replacing registrations
/// affects only subsequently compiled projections.
#[derive(Default)]
pub struct Registry {
    resolvers: HashMap<TypeId, Resolver>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_standard_nodes() -> Self {
        let mut registry = Self::new();
        registry.register::<Text>(|text| {
            Ok(Contribution::Text(
                text.0.clone(),
                HighlightStyle::default(),
            ))
        });
        registry.register::<Span>(|span| Ok(Contribution::Inline(span.0)));
        registry.register::<Paragraph>(|_| Ok(Contribution::Paragraph(HighlightStyle::default())));
        registry
    }

    /// Register or replace the resolver for a Rust payload type.
    pub fn register<T: Any>(&mut self, resolve: impl Fn(&T) -> Result<Contribution> + 'static) {
        self.resolvers.insert(
            TypeId::of::<T>(),
            Box::new(move |payload| {
                let payload = payload
                    .downcast_ref::<T>()
                    .expect("registry payload type matches its key");
                resolve(payload)
            }),
        );
    }

    /// Flatten paragraphs into one shaping and selection stream.
    ///
    /// Paragraphs are separated by two newlines, including empty paragraphs.
    /// Styling boundaries do not add text or interrupt shaping explicitly.
    pub fn compile(&self, document: &Document) -> Result<Projection> {
        let mut output = Projection {
            text: SharedString::default(),
            spans: Vec::new(),
        };
        let mut text = String::new();
        for (index, root) in document.roots.iter().enumerate() {
            if index > 0 {
                text.push_str("\n\n");
            }
            self.append(
                document,
                *root,
                true,
                HighlightStyle::default(),
                &mut text,
                &mut output.spans,
            )?;
        }
        output.text = text.into();
        Ok(output)
    }

    fn append(
        &self,
        document: &Document,
        id: NodeId,
        root: bool,
        inherited: HighlightStyle,
        text: &mut String,
        spans: &mut Vec<TextSpan>,
    ) -> Result<()> {
        let node = &document.nodes[&id];
        let Some(resolve) = self.resolvers.get(&node.payload.as_ref().type_id()) else {
            bail!("unregistered payload on node {id:?}");
        };
        let contribution = resolve(node.payload.as_ref())?;
        if root != matches!(contribution, Contribution::Paragraph(_)) {
            bail!("node {id:?}: roots must be paragraphs and paragraphs cannot be nested");
        }
        match contribution {
            Contribution::Text(value, style) => {
                if !node.children.is_empty() {
                    bail!("text node {id:?} cannot have children");
                }
                let start = text.len();
                text.push_str(&value);
                spans.push(TextSpan {
                    node: id,
                    range: start..text.len(),
                    style: inherited.highlight(style),
                });
            }
            Contribution::Inline(style) | Contribution::Paragraph(style) => {
                let style = inherited.highlight(style);
                for child in &node.children {
                    self.append(document, *child, false, style, text, spans)?;
                }
            }
        }
        Ok(())
    }
}

/// A leaf's UTF-8 byte range in the projected text. Separators have no leaf ID.
#[derive(Clone, Debug)]
pub struct TextSpan {
    pub node: NodeId,
    pub range: Range<usize>,
    pub style: HighlightStyle,
}

/// Immutable text and source mapping, compiled outside the paint path.
pub struct Projection {
    text: SharedString,
    spans: Vec<TextSpan>,
}

impl Projection {
    pub fn text(&self) -> &SharedString {
        &self.text
    }
    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }

    /// Map a leaf-relative UTF-8 position to the shared selection stream.
    /// This checks scalar boundaries; user selection additionally uses GPUI's
    /// grapheme and shaping-cluster handling across the complete stream.
    pub fn offset(&self, node: NodeId, byte_offset: usize) -> Option<usize> {
        let span = self.spans.iter().find(|span| span.node == node)?;
        if byte_offset > span.range.len() {
            return None;
        }
        let offset = span.range.start + byte_offset;
        self.text.is_char_boundary(offset).then_some(offset)
    }

    pub(crate) fn styled_text(&self) -> StyledText {
        StyledText::new(self.text.clone()).with_highlights(
            self.spans
                .iter()
                .filter(|span| !span.range.is_empty())
                .map(|span| (span.range.clone(), span.style)),
        )
    }
}
