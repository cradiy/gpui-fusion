use crate::{
    Code, CodeBlock, Document, Emphasis, Group, HardBreak, InlineStyle, Link, Mark, NodeId,
    Strikethrough, Strong, TextTheme, Underline,
};
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
    StyledInline(InlineStyle),
    Paragraph(HighlightStyle),
    Blocks(InlineStyle),
    CodeBlock {
        block: CodeBlock,
        style: InlineStyle,
    },
    Link {
        target: SharedString,
        text: Option<SharedString>,
        style: InlineStyle,
    },
}

#[derive(Default)]
struct Compilation {
    text: String,
    spans: Vec<TextSpan>,
    links: Vec<LinkSpan>,
    inside_link: bool,
    blocks: Vec<BlockSpan>,
}

impl Compilation {
    fn start_block(&mut self) -> usize {
        if !self.blocks.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.len()
    }

    fn code_block(&mut self, id: NodeId, block: CodeBlock, style: InlineStyle) -> Result<()> {
        let mut end = 0;
        for (range, _) in &block.highlights {
            if range.start < end
                || range.start > range.end
                || !block.text.is_char_boundary(range.start)
                || !block.text.is_char_boundary(range.end)
            {
                bail!("invalid code highlight range at node {id:?}");
            }
            end = range.end;
        }
        let start = self.start_block();
        self.text.push_str(&block.text);
        let mut offset = 0;
        let mut push = |range: Range<usize>, highlight: HighlightStyle| {
            self.spans.push(TextSpan {
                node: id,
                range: start + range.start..start + range.end,
                style: style.highlight.highlight(highlight),
                font_family: style.font_family.clone(),
            });
        };
        for (range, highlight) in block.highlights {
            if offset < range.start {
                push(offset..range.start, HighlightStyle::default());
            }
            if !range.is_empty() {
                push(range.clone(), highlight);
            }
            offset = range.end;
        }
        if offset < block.text.len() || block.text.is_empty() {
            push(offset..block.text.len(), HighlightStyle::default());
        }
        self.blocks.push(BlockSpan {
            node: id,
            range: start..self.text.len(),
            kind: BlockKind::Code {
                language: block.language,
            },
        });
        Ok(())
    }
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
        Self::with_theme(TextTheme::default())
    }

    /// Register standard nodes with application-supplied inline style presets.
    pub fn with_theme(theme: TextTheme) -> Self {
        let mut registry = Self::new();
        registry.register::<Text>(|text| {
            Ok(Contribution::Text(
                text.0.clone(),
                HighlightStyle::default(),
            ))
        });
        registry.register::<Span>(|span| Ok(Contribution::Inline(span.0)));
        registry.register::<Paragraph>(|_| Ok(Contribution::Paragraph(HighlightStyle::default())));
        registry.register::<Group>(|_| Ok(Contribution::Blocks(InlineStyle::default())));
        registry.register::<Link>(move |link| {
            Ok(Contribution::Link {
                target: link.link.clone(),
                text: link.text.clone(),
                style: theme.link.clone(),
            })
        });
        registry.register::<InlineStyle>(|style| Ok(Contribution::StyledInline(style.clone())));
        registry.register::<Strong>(move |_| Ok(Contribution::StyledInline(theme.strong.clone())));
        registry
            .register::<Emphasis>(move |_| Ok(Contribution::StyledInline(theme.emphasis.clone())));
        registry.register::<Underline>(move |_| {
            Ok(Contribution::StyledInline(theme.underline.clone()))
        });
        registry.register::<Strikethrough>(move |_| {
            Ok(Contribution::StyledInline(theme.strikethrough.clone()))
        });
        registry.register::<Mark>(move |_| Ok(Contribution::StyledInline(theme.mark.clone())));
        registry.register::<Code>(move |_| Ok(Contribution::StyledInline(theme.code.clone())));
        registry.register::<CodeBlock>(move |block| {
            Ok(Contribution::CodeBlock {
                block: block.clone(),
                style: theme.code_block.clone(),
            })
        });
        registry.register::<HardBreak>(|_| {
            Ok(Contribution::Text("\n".into(), HighlightStyle::default()))
        });
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
        let mut output = Compilation::default();
        for root in document.roots.iter() {
            self.append(document, *root, true, InlineStyle::default(), &mut output)?;
        }
        Ok(Projection {
            text: output.text.into(),
            spans: output.spans,
            links: output.links,
            blocks: output.blocks,
        })
    }

    fn append(
        &self,
        document: &Document,
        id: NodeId,
        root: bool,
        inherited: InlineStyle,
        output: &mut Compilation,
    ) -> Result<()> {
        let node = &document.nodes[&id];
        let Some(resolve) = self.resolvers.get(&node.payload.as_ref().type_id()) else {
            bail!("unregistered payload on node {id:?}");
        };
        let contribution = resolve(node.payload.as_ref())?;
        if root
            != matches!(
                contribution,
                Contribution::Paragraph(_)
                    | Contribution::Blocks(_)
                    | Contribution::CodeBlock { .. }
            )
        {
            bail!(
                "node {id:?}: expected {} content",
                if root { "block" } else { "inline" }
            );
        }
        match contribution {
            Contribution::Text(value, style) => {
                if !node.children.is_empty() {
                    bail!("text node {id:?} cannot have children");
                }
                let start = output.text.len();
                output.text.push_str(&value);
                output.spans.push(TextSpan {
                    node: id,
                    range: start..output.text.len(),
                    style: inherited.highlight.highlight(style),
                    font_family: inherited.font_family,
                });
            }
            Contribution::Paragraph(style) => {
                let start = output.start_block();
                let style = inherited.inherit(&style.into());
                for child in &node.children {
                    self.append(document, *child, false, style.clone(), output)?;
                }
                output.blocks.push(BlockSpan {
                    node: id,
                    range: start..output.text.len(),
                    kind: BlockKind::Paragraph,
                });
            }
            Contribution::CodeBlock { block, style } => {
                if !node.children.is_empty() {
                    bail!("code block {id:?} cannot have children");
                }
                output.code_block(id, block, inherited.inherit(&style))?;
            }
            Contribution::Inline(style) => {
                let style = inherited.inherit(&style.into());
                for child in &node.children {
                    self.append(document, *child, false, style.clone(), output)?;
                }
            }
            Contribution::StyledInline(style) => {
                let style = inherited.inherit(&style);
                for child in &node.children {
                    self.append(document, *child, false, style.clone(), output)?;
                }
            }
            Contribution::Blocks(style) => {
                let style = inherited.inherit(&style);
                for child in &node.children {
                    self.append(document, *child, true, style.clone(), output)?;
                }
            }
            Contribution::Link {
                target,
                text,
                style,
            } => {
                if output.inside_link {
                    bail!("nested link at node {id:?}");
                }
                if target.trim().is_empty() {
                    bail!("empty link target at node {id:?}");
                }
                let start = output.text.len();
                output.inside_link = true;
                let style = inherited.inherit(&style);
                if node.children.is_empty() {
                    let label = text.as_ref().unwrap_or(&target);
                    output.text.push_str(label);
                    output.spans.push(TextSpan {
                        node: id,
                        range: start..output.text.len(),
                        style: style.highlight,
                        font_family: style.font_family,
                    });
                } else {
                    if text.is_some() {
                        bail!("link {id:?} cannot combine text and child content");
                    }
                    for child in &node.children {
                        self.append(document, *child, false, style.clone(), output)?;
                    }
                }
                output.inside_link = false;
                if start < output.text.len() {
                    output.links.push(LinkSpan {
                        node: id,
                        target,
                        range: start..output.text.len(),
                    });
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
    pub font_family: Option<SharedString>,
}

/// Immutable text and source mapping, compiled outside the paint path.
pub struct Projection {
    text: SharedString,
    spans: Vec<TextSpan>,
    links: Vec<LinkSpan>,
    blocks: Vec<BlockSpan>,
}

/// A leaf block's range, excluding separators inserted between blocks.
#[derive(Clone, Debug)]
pub struct BlockSpan {
    pub node: NodeId,
    pub range: Range<usize>,
    pub kind: BlockKind,
}

#[derive(Clone, Debug)]
pub enum BlockKind {
    Paragraph,
    Code { language: Option<SharedString> },
}

/// An actionable range in the projected text; the destination is opaque to GPUI.
#[derive(Clone, Debug)]
pub struct LinkSpan {
    pub node: NodeId,
    pub target: SharedString,
    pub range: Range<usize>,
}

impl Projection {
    pub fn text(&self) -> &SharedString {
        &self.text
    }
    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }
    pub fn links(&self) -> &[LinkSpan] {
        &self.links
    }
    pub fn blocks(&self) -> &[BlockSpan] {
        &self.blocks
    }

    /// Map a leaf-relative UTF-8 position to the shared selection stream.
    /// This checks scalar boundaries; user selection additionally uses GPUI's
    /// grapheme and shaping-cluster handling across the complete stream.
    pub fn offset(&self, node: NodeId, byte_offset: usize) -> Option<usize> {
        let span = self.spans.iter().find(|span| span.node == node)?;
        let end = self
            .spans
            .iter()
            .rev()
            .find(|span| span.node == node)?
            .range
            .end;
        if byte_offset > end - span.range.start {
            return None;
        }
        let offset = span.range.start + byte_offset;
        self.text.is_char_boundary(offset).then_some(offset)
    }

    pub(crate) fn styled_text(&self) -> StyledText {
        StyledText::new(self.text.clone())
            .with_highlights(
                self.spans
                    .iter()
                    .filter(|span| !span.range.is_empty())
                    .map(|span| (span.range.clone(), span.style)),
            )
            .with_font_family_overrides(self.spans.iter().filter_map(|span| {
                if span.range.is_empty() {
                    return None;
                }
                span.font_family
                    .clone()
                    .map(|family| (span.range.clone(), family))
            }))
    }
}
