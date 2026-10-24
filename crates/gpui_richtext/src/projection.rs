use crate::{
    Code, CodeBlock, Document, Emphasis, Group, HardBreak, Heading, HeadingLevel, InlineStyle,
    Link, ListItem, ListMarker, Mark, NodeId, Quote, Strikethrough, Strong, TextTheme, Underline,
};
use anyhow::{Result, bail};
use gpui::{HighlightStyle, SharedString, StyledText};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ops::Range,
    sync::Arc,
};

/// A text leaf. Its style inherits from ancestor contributions.
pub struct Text(pub SharedString);
/// An inline style container.
pub struct Span(pub HighlightStyle);
/// A paragraph containing text leaves and inline containers.
pub struct Paragraph;

/// The semantic contribution of a registered payload type.
pub enum Contribution {
    MathInline(crate::MathInline),
    Graphic {
        graphic: crate::Graphic,
        code_language: Option<SharedString>,
    },
    Details(crate::Details),
    Footnote {
        marker: ListMarker,
        style: InlineStyle,
    },
    Anchor(SharedString),
    Alert(crate::Alert),
    DefinitionList,
    DefinitionTerm(InlineStyle),
    DefinitionDescription,
    Rule,
    Image(crate::Image),
    BlockQuote(InlineStyle),
    ListEntry {
        marker: ListMarker,
        style: InlineStyle,
    },
    Table(crate::Table),
    TableRow {
        header: bool,
        style: InlineStyle,
    },
    TableCell(InlineStyle),
    Text(SharedString, HighlightStyle),
    Inline(HighlightStyle),
    StyledInline(InlineStyle),
    Paragraph(HighlightStyle),
    Heading {
        level: HeadingLevel,
        style: InlineStyle,
    },
    Quote(InlineStyle),
    ListItem {
        marker: ListMarker,
        style: InlineStyle,
    },
    Blocks(InlineStyle),
    CodeBlock {
        block: CodeBlock,
        style: InlineStyle,
    },
    Link {
        kind: crate::LinkKind,
        target: SharedString,
        text: Option<SharedString>,
        title: Option<SharedString>,
        style: InlineStyle,
    },
}

impl Contribution {
    fn is_block(&self) -> bool {
        matches!(
            self,
            Contribution::Paragraph(_)
                | Contribution::Graphic { .. }
                | Contribution::Details(_)
                | Contribution::Alert(_)
                | Contribution::DefinitionList
                | Contribution::DefinitionTerm(_)
                | Contribution::DefinitionDescription
                | Contribution::Rule
                | Contribution::Image(_)
                | Contribution::BlockQuote(_)
                | Contribution::ListEntry { .. }
                | Contribution::Footnote { .. }
                | Contribution::Table(_)
                | Contribution::Heading { .. }
                | Contribution::Quote(_)
                | Contribution::ListItem { .. }
                | Contribution::Blocks(_)
                | Contribution::CodeBlock { .. }
        )
    }
}

#[derive(Default)]
struct Compilation {
    inline_math: Vec<InlineMathSpan>,
    cells: HashMap<NodeId, Arc<Projection>>,
    footnote: bool,
    anchors: Vec<crate::AnchorSpan>,
    #[cfg(feature = "syntax-highlighting")]
    highlights: HashMap<NodeId, Arc<crate::syntax::CachedHighlight>>,
    indent: usize,
    quote_depth: usize,
    quote_path: Vec<NodeId>,
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
            footnote: self.footnote,
            indent: self.indent,
            quote_depth: self.quote_depth,
            quote_path: self.quote_path.clone(),
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

#[cfg(test)]
mod cell_content_tests {
    use super::*;
    use crate::compose::*;

    #[test]
    fn cell_projections_keep_images_empty_links_and_global_source_coordinates() {
        let document = Document::from_roots([
            paragraph().child(text("Lead")),
            table().child(
                table_row()
                    .child(
                        table_cell()
                            .child(paragraph().child(text("Before")))
                            .child(node(
                                crate::Image::new("assets/diagram.svg", "図").link("app:diagram"),
                            ))
                            .child(paragraph().child(link_with_text("app:details", "Details"))),
                    )
                    .child(table_cell().child(node(
                        crate::Image::new("assets/empty.svg", "").link("app:empty"),
                    ))),
            ),
            paragraph().child(text("Tail")),
        ])
        .unwrap();
        let projection = Registry::with_standard_nodes().compile(&document).unwrap();
        let RenderPart::Table { rows, .. } = &projection.parts[1] else {
            panic!("missing table")
        };
        let first = &rows[0].cells[0];
        assert!(matches!(
            &first.projection.parts[1],
            RenderPart::Embed(BlockSpan {
                kind: BlockKind::Image(_),
                ..
            })
        ));
        assert_eq!(first.projection.text().as_ref(), "Before\n\n図\n\nDetails");
        let local = first.projection.links().last().unwrap();
        let global = projection
            .links()
            .iter()
            .find(|link| link.node == local.node)
            .unwrap();
        assert_eq!(global.range.start, first.offset + local.range.start);
        assert_eq!(&projection.text()[global.range.clone()], "Details");
        let empty = &rows[0].cells[1].projection;
        assert!(empty.text().is_empty());
        assert!(matches!(
            &empty.parts[0],
            RenderPart::Embed(BlockSpan {
                kind: BlockKind::Image(_),
                ..
            })
        ));
        assert_eq!(empty.links()[0].target.as_ref(), "app:empty");
        assert!(empty.links()[0].range.is_empty());
        assert!(projection.text().ends_with("\n\nTail"));
    }
}

#[cfg(all(test, feature = "syntax-highlighting"))]
mod highlight_cache_tests {
    use super::*;
    use crate::compose::*;

    #[test]
    fn unchanged_code_is_reused_and_removed_code_is_released() {
        let registry = Registry::with_standard_nodes();
        let document = Document::from_roots([node(
            CodeBlock::new("let enabled = false;\n").language("rust"),
        )])
        .unwrap();
        registry.compile(&document).unwrap();
        let original = registry
            .highlights
            .borrow()
            .values()
            .next()
            .unwrap()
            .clone();
        registry.compile(&document).unwrap();
        assert!(Arc::ptr_eq(
            &original,
            registry.highlights.borrow().values().next().unwrap()
        ));
        let weak = Arc::downgrade(&original);
        drop(original);
        registry
            .compile(&Document::from_roots([paragraph().child(text("No code remains."))]).unwrap())
            .unwrap();
        assert!(weak.upgrade().is_none());
        assert!(registry.highlights.borrow().is_empty());
        registry.compile(&document).unwrap();
        let weak = Arc::downgrade(registry.highlights.borrow().values().next().unwrap());
        registry.clear_highlight_cache();
        assert!(weak.upgrade().is_none());
    }
}

/// View-specific typed node resolution. Unknown payloads are errors.
///
/// Resolving a document creates an immutable projection; replacing registrations
/// affects only subsequently compiled projections.
/// Built-in syntax highlighting retains only the last successful document's
/// code blocks. Unchanged blocks reuse colors; growing blocks resume after the
/// last complete source line.
#[derive(Default)]
pub struct Registry {
    resolvers: HashMap<TypeId, Resolver>,
    #[cfg(feature = "syntax-highlighting")]
    highlighter: Option<Arc<crate::syntax::SyntaxHighlighter>>,
    #[cfg(feature = "syntax-highlighting")]
    highlights: std::cell::RefCell<HashMap<NodeId, Arc<crate::syntax::CachedHighlight>>>,
}

impl Registry {
    #[cfg(feature = "syntax-highlighting")]
    pub fn syntax_highlighter(
        mut self,
        highlighter: Option<Arc<crate::syntax::SyntaxHighlighter>>,
    ) -> Self {
        self.highlighter = highlighter;
        self.highlights.get_mut().clear();
        self
    }
    /// Release code highlight results retained from the last successful compilation.
    #[cfg(feature = "syntax-highlighting")]
    pub fn clear_highlight_cache(&self) {
        self.highlights.borrow_mut().clear();
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_standard_nodes() -> Self {
        Self::with_theme(TextTheme::default())
    }

    /// Register standard nodes with application-supplied inline style presets.
    pub fn with_theme(theme: TextTheme) -> Self {
        let mut registry = Self::new();
        #[cfg(feature = "syntax-highlighting")]
        {
            registry.highlighter = Some(Arc::new(crate::syntax::SyntaxHighlighter::default()));
        }
        registry.register::<crate::Rule>(|_| Ok(Contribution::Rule));
        registry.register::<crate::Anchor>(|anchor| Ok(Contribution::Anchor(anchor.0.clone())));
        let footnote_style = theme.footnote.clone();
        registry.register::<crate::Footnote>(move |note| {
            Ok(Contribution::Footnote {
                marker: ListMarker::Number(note.number),
                style: footnote_style.clone(),
            })
        });
        let anchor_style = theme.link.clone();
        let backlink_style = theme.footnote_backlink.clone();
        registry.register::<crate::FootnoteBacklink>(move |link| {
            Ok(Contribution::Link {
                kind: crate::LinkKind::Anchor,
                target: link.target.clone(),
                text: None,
                title: None,
                style: backlink_style.clone(),
            })
        });
        registry.register::<crate::AnchorLink>(move |link| {
            Ok(Contribution::Link {
                kind: crate::LinkKind::Anchor,
                target: link.target.clone(),
                text: None,
                title: link.title.clone(),
                style: anchor_style.clone(),
            })
        });
        registry.register::<crate::Alert>(|alert| Ok(Contribution::Alert(alert.clone())));
        registry.register::<crate::Details>(|details| Ok(Contribution::Details(details.clone())));
        registry.register::<crate::DefinitionList>(|_| Ok(Contribution::DefinitionList));
        let term_style = theme.definition_term.clone();
        registry.register::<crate::DefinitionTerm>(move |_| {
            Ok(Contribution::DefinitionTerm(term_style.clone()))
        });
        registry
            .register::<crate::DefinitionDescription>(|_| Ok(Contribution::DefinitionDescription));
        registry.register::<crate::Image>(|image| Ok(Contribution::Image(image.clone())));
        #[cfg(feature = "mermaid")]
        registry.register::<crate::Mermaid>(|value| {
            Ok(Contribution::Graphic {
                graphic: crate::Graphic::Mermaid(value.clone()),
                code_language: None,
            })
        });
        #[cfg(feature = "mermaid")]
        registry.register::<crate::MermaidCodeBlock>(|value| {
            Ok(Contribution::Graphic {
                graphic: crate::Graphic::Mermaid(crate::Mermaid(value.0.clone())),
                code_language: Some("mermaid".into()),
            })
        });
        #[cfg(feature = "math")]
        registry.register::<crate::MathBlock>(|value| {
            Ok(Contribution::Graphic {
                graphic: crate::Graphic::Math(value.clone()),
                code_language: None,
            })
        });
        #[cfg(feature = "math")]
        registry.register::<crate::MathInline>(|value| Ok(Contribution::MathInline(value.clone())));
        let quote_style = theme.quote.clone();
        registry.register::<crate::BlockQuote>(move |_| {
            Ok(Contribution::BlockQuote(quote_style.clone()))
        });
        let list_style = theme.list_item.clone();
        registry.register::<crate::ListEntry>(move |entry| {
            Ok(Contribution::ListEntry {
                marker: entry.0,
                style: list_style.clone(),
            })
        });
        registry.register::<crate::Table>(|table| Ok(Contribution::Table(table.clone())));
        registry.register::<crate::TableRow>(move |row| {
            Ok(Contribution::TableRow {
                header: row.header,
                style: if row.header {
                    theme.table_header.clone()
                } else {
                    InlineStyle::default()
                },
            })
        });
        registry.register::<crate::TableCell>(move |_| {
            Ok(Contribution::TableCell(theme.table_cell.clone()))
        });
        registry.register::<Text>(|text| {
            Ok(Contribution::Text(
                text.0.clone(),
                HighlightStyle::default(),
            ))
        });
        registry.register::<Span>(|span| Ok(Contribution::Inline(span.0)));
        registry.register::<Paragraph>(|_| Ok(Contribution::Paragraph(HighlightStyle::default())));
        registry.register::<Heading>(move |heading| {
            Ok(Contribution::Heading {
                level: heading.0,
                style: theme.headings[heading.0.index()].clone(),
            })
        });
        registry.register::<Quote>(move |_| Ok(Contribution::Quote(theme.quote.clone())));
        registry.register::<ListItem>(move |item| {
            Ok(Contribution::ListItem {
                marker: item.0,
                style: theme.list_item.clone(),
            })
        });
        registry.register::<Group>(|_| Ok(Contribution::Blocks(InlineStyle::default())));
        registry.register::<Link>(move |link| {
            Ok(Contribution::Link {
                kind: crate::LinkKind::External,
                target: link.link.clone(),
                text: link.text.clone(),
                title: link.title.clone(),
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
        registry.register::<crate::Keyboard>(move |_| {
            Ok(Contribution::StyledInline(theme.keyboard.clone()))
        });
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
    /// Blocks are separated by two newlines; consecutive list items use one.
    /// Styling boundaries do not add text or interrupt shaping explicitly.
    pub fn compile(&self, document: &Document) -> Result<Projection> {
        let mut output = Compilation::default();
        for root in document.roots.iter() {
            self.append(document, *root, true, InlineStyle::default(), &mut output)?;
        }
        let mut names = std::collections::HashSet::new();
        if output
            .anchors
            .iter()
            .any(|anchor| !names.insert(anchor.name.clone()))
        {
            bail!("duplicate document anchor");
        }
        #[cfg(feature = "syntax-highlighting")]
        {
            *self.highlights.borrow_mut() = output.highlights;
        }
        let mut projection = Projection {
            inline_math: output.inline_math,
            cells: output.cells,
            anchors: output.anchors,
            text: output.text.into(),
            spans: output.spans,
            links: output.links,
            blocks: output.blocks,
            parts: Vec::new(),
        };
        projection.prepare_parts();
        Ok(projection)
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
        let contribution = self.resolve_node(document, id)?;
        if root != contribution.is_block() {
            bail!(
                "node {id:?}: expected {} content",
                if root { "block" } else { "inline" }
            );
        }
        let is_footnote = matches!(contribution, Contribution::Footnote { .. });
        match contribution {
            Contribution::Anchor(name) => {
                if name.is_empty() {
                    bail!("empty anchor at node {id:?}");
                }
                output.anchors.push(crate::AnchorSpan {
                    name,
                    offset: output.text.len(),
                });
                for child in &node.children {
                    self.append(document, *child, false, inherited.clone(), output)?;
                }
            }
            container @ (Contribution::Alert(_) | Contribution::Details(_)) => {
                let mut body = Compilation {
                    footnote: output.footnote,
                    ..Default::default()
                };
                for child in &node.children {
                    self.append(document, *child, true, inherited.clone(), &mut body)?;
                }
                let start = output.start_block();
                output.text.push_str(&body.text);
                output
                    .inline_math
                    .extend(body.inline_math.iter().cloned().map(|mut span| {
                        span.range = start + span.range.start..start + span.range.end;
                        span
                    }));
                output
                    .anchors
                    .extend(body.anchors.iter().cloned().map(|mut anchor| {
                        anchor.offset += start;
                        anchor
                    }));
                output
                    .spans
                    .extend(body.spans.iter().cloned().map(|mut span| {
                        span.range = start + span.range.start..start + span.range.end;
                        span
                    }));
                output
                    .links
                    .extend(body.links.iter().cloned().map(|mut link| {
                        link.range = start + link.range.start..start + link.range.end;
                        link
                    }));
                #[cfg(feature = "syntax-highlighting")]
                output.highlights.extend(body.highlights);
                let mut projection = Projection {
                    inline_math: body.inline_math,
                    cells: body.cells,
                    anchors: body.anchors,
                    text: body.text.into(),
                    spans: body.spans,
                    links: body.links,
                    blocks: body.blocks,
                    parts: Vec::new(),
                };
                projection.prepare_parts();
                let body = Arc::new(projection);
                let kind = match container {
                    Contribution::Alert(alert) => BlockKind::Alert { alert, body },
                    Contribution::Details(details) => BlockKind::Details { details, body },
                    _ => unreachable!(),
                };
                output.blocks.push(BlockSpan {
                    footnote: output.footnote,
                    node: id,
                    range: start..output.text.len(),
                    indent: output.indent,
                    quote_depth: output.quote_depth,
                    quote_path: output.quote_path.clone(),
                    kind,
                });
            }
            Contribution::DefinitionList => {
                let mut has_term = false;
                let mut has_description = false;
                for child in &node.children {
                    match self.resolve_node(document, *child)? {
                        Contribution::DefinitionTerm(_) => {
                            if has_term && !has_description {
                                bail!("definition term requires a description");
                            }
                            has_term = true;
                            has_description = false;
                        }
                        Contribution::DefinitionDescription if has_term => has_description = true,
                        _ => bail!("definition list requires terms followed by descriptions"),
                    }
                    self.append(document, *child, true, inherited.clone(), output)?;
                }
                if has_term && !has_description {
                    bail!("definition term requires a description");
                }
            }
            Contribution::DefinitionDescription => {
                output.indent += 1;
                for child in &node.children {
                    self.append(document, *child, true, inherited.clone(), output)?;
                }
                output.indent -= 1;
            }
            leaf @ (Contribution::Rule | Contribution::Image(_) | Contribution::Graphic { .. }) => {
                if !node.children.is_empty() {
                    bail!("leaf block {id:?} cannot have children");
                }
                let start = output.start_block();
                let kind = match leaf {
                    Contribution::Graphic {
                        graphic,
                        code_language,
                    } => {
                        output.text.push_str(graphic.source());
                        BlockKind::Graphic {
                            graphic,
                            code_language,
                        }
                    }
                    Contribution::Image(image) => {
                        output.text.push_str(&image.alt);
                        if let Some(target) = &image.link {
                            if target.trim().is_empty() {
                                bail!("empty image link target at node {id:?}");
                            }
                            output.links.push(LinkSpan {
                                kind: image.link_kind,
                                node: id,
                                target: target.clone(),
                                title: image.link_title.clone(),
                                range: start..output.text.len(),
                            });
                        }
                        BlockKind::Image(image)
                    }
                    _ => BlockKind::Rule,
                };
                output.blocks.push(BlockSpan {
                    footnote: output.footnote,
                    node: id,
                    range: start..output.text.len(),
                    kind,
                    indent: output.indent,
                    quote_depth: output.quote_depth,
                    quote_path: output.quote_path.clone(),
                });
            }
            Contribution::BlockQuote(style) => {
                output.quote_depth += 1;
                output.quote_path.push(id);
                for child in &node.children {
                    self.append(document, *child, true, inherited.inherit(&style), output)?;
                }
                output.quote_depth -= 1;
                output.quote_path.pop();
            }
            Contribution::ListEntry { marker, style }
            | Contribution::Footnote { marker, style } => {
                let previous_footnote = output.footnote;
                output.footnote |= is_footnote;
                let style = inherited.inherit(&style);
                let start = output.start_block();
                output.text.push_str(&marker.text());
                output.spans.push(TextSpan {
                    node: id,
                    range: start..output.text.len(),
                    style: style.highlight,
                    font_family: style.font_family.clone(),
                });
                let first_paragraph = node.children.first().copied().filter(|child| {
                    matches!(
                        self.resolve_node(document, *child),
                        Ok(Contribution::Paragraph(_))
                    )
                });
                if let Some(first) = first_paragraph {
                    let Contribution::Paragraph(paragraph_style) =
                        self.resolve_node(document, first)?
                    else {
                        unreachable!()
                    };
                    for child in &document.nodes[&first].children {
                        self.append(
                            document,
                            *child,
                            false,
                            style.inherit(&paragraph_style.into()),
                            output,
                        )?;
                    }
                }
                output.blocks.push(BlockSpan {
                    footnote: output.footnote,
                    node: id,
                    range: start..output.text.len(),
                    kind: BlockKind::ListItem { marker },
                    indent: output.indent,
                    quote_depth: output.quote_depth,
                    quote_path: output.quote_path.clone(),
                });
                output.indent += 1;
                for child in node
                    .children
                    .iter()
                    .skip(usize::from(first_paragraph.is_some()))
                {
                    self.append(document, *child, true, style.clone(), output)?;
                }
                output.indent -= 1;
                output.footnote = previous_footnote;
            }
            Contribution::Table(table) => {
                self.append_table(document, id, table, inherited, output)?
            }
            Contribution::TableRow { .. } | Contribution::TableCell(_) => {
                bail!("table row or cell {id:?} outside its table parent");
            }
            Contribution::MathInline(value) => {
                if !node.children.is_empty() {
                    bail!("inline formula {id:?} cannot have children");
                }
                let start = output.text.len();
                output.text.push_str(&value.0);
                let range = start..output.text.len();
                output.inline_math.push(InlineMathSpan {
                    node: id,
                    range: range.clone(),
                    formula: value,
                });
                output.spans.push(TextSpan {
                    node: id,
                    range,
                    style: inherited.highlight,
                    font_family: inherited.font_family,
                });
            }
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
            block @ (Contribution::Paragraph(_)
            | Contribution::DefinitionTerm(_)
            | Contribution::Heading { .. }
            | Contribution::Quote(_)
            | Contribution::ListItem { .. }) => {
                let (kind, style, prefix) = match block {
                    Contribution::Paragraph(style) => (BlockKind::Paragraph, style.into(), None),
                    Contribution::DefinitionTerm(style) => (BlockKind::DefinitionTerm, style, None),
                    Contribution::Heading { level, style } => {
                        (BlockKind::Heading { level }, style, None)
                    }
                    Contribution::Quote(style) => (BlockKind::Quote, style, None),
                    Contribution::ListItem { marker, style } => {
                        (BlockKind::ListItem { marker }, style, Some(marker.text()))
                    }
                    _ => unreachable!(),
                };
                let start = if matches!(kind, BlockKind::ListItem { .. })
                    && output
                        .blocks
                        .last()
                        .is_some_and(|block| matches!(block.kind, BlockKind::ListItem { .. }))
                {
                    output.text.push('\n');
                    output.text.len()
                } else {
                    output.start_block()
                };
                let style = inherited.inherit(&style);
                if let Some(prefix) = prefix {
                    output.text.push_str(&prefix);
                    output.spans.push(TextSpan {
                        node: id,
                        range: start..output.text.len(),
                        style: style.highlight,
                        font_family: style.font_family.clone(),
                    });
                }
                for child in &node.children {
                    self.append(document, *child, false, style.clone(), output)?;
                }
                output.blocks.push(BlockSpan {
                    footnote: output.footnote,
                    indent: output.indent,
                    quote_depth: output.quote_depth,
                    quote_path: output.quote_path.clone(),
                    node: id,
                    range: start..output.text.len(),
                    kind,
                });
            }
            Contribution::CodeBlock { block, style } => {
                if !node.children.is_empty() {
                    bail!("code block {id:?} cannot have children");
                }
                #[cfg(feature = "syntax-highlighting")]
                let block = {
                    let mut block = block;
                    if block.highlights.is_empty()
                        && let Some(language) = &block.language
                        && let Some(highlighter) = &self.highlighter
                    {
                        let previous = self.highlights.borrow().get(&id).cloned();
                        let highlights = highlighter.highlight_cached(
                            block.text.clone(),
                            language,
                            previous.as_ref(),
                        )?;
                        block.highlights = highlights.ranges.clone();
                        output.highlights.insert(id, highlights);
                    }
                    block
                };
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
                kind,
                target,
                text,
                title,
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
                        kind,
                        node: id,
                        target,
                        title,
                        range: start..output.text.len(),
                    });
                }
            }
        }
        Ok(())
    }

    fn resolve_node(&self, document: &Document, id: NodeId) -> Result<Contribution> {
        let node = &document.nodes[&id];
        let Some(resolve) = self.resolvers.get(&node.payload.as_ref().type_id()) else {
            bail!("unregistered payload on node {id:?}");
        };
        resolve(node.payload.as_ref())
    }

    fn append_table(
        &self,
        document: &Document,
        id: NodeId,
        table: crate::Table,
        inherited: InlineStyle,
        output: &mut Compilation,
    ) -> Result<()> {
        let node = &document.nodes[&id];
        let Some(first) = node.children.first() else {
            bail!("table {id:?} requires a row");
        };
        let count = document.nodes[first].children.len();
        if count == 0 {
            bail!("table {id:?} requires a column");
        }
        let columns = if table.columns.is_empty() {
            vec![crate::TableColumn::default(); count]
        } else {
            if table.columns.len() != count {
                bail!("table {id:?} column count differs from its rows");
            }
            table.columns
        };
        for column in &columns {
            let width = match column.width {
                crate::ColumnWidth::Fixed(width) => f32::from(width),
                crate::ColumnWidth::Flex(weight) => weight,
            };
            if !width.is_finite() || width <= 0. {
                bail!("invalid column width in table {id:?}");
            }
        }
        let start = output.start_block();
        let mut rows = Vec::new();
        for (row_index, row_id) in node.children.iter().copied().enumerate() {
            let Contribution::TableRow { header, style } = self.resolve_node(document, row_id)?
            else {
                bail!("table {id:?} requires row children");
            };
            let row_node = &document.nodes[&row_id];
            if row_node.children.len() != count {
                bail!("inconsistent column count at row {row_id:?}");
            }
            if row_index > 0 {
                output.text.push('\n');
            }
            let row_style = inherited.inherit(&style);
            let mut cells = Vec::new();
            for (column_index, cell_id) in row_node.children.iter().copied().enumerate() {
                let Contribution::TableCell(style) = self.resolve_node(document, cell_id)? else {
                    bail!("table row {row_id:?} requires cell children");
                };
                if column_index > 0 {
                    output.text.push('\t');
                }
                let cell_start = output.text.len();
                let style = row_style.inherit(&style);
                let cell_node = &document.nodes[&cell_id];
                let block_content = cell_node
                    .children
                    .first()
                    .map(|child| {
                        self.resolve_node(document, *child)
                            .map(|contribution| contribution.is_block())
                    })
                    .transpose()?
                    .unwrap_or(false);
                let mut content = Compilation {
                    footnote: output.footnote,
                    ..Default::default()
                };
                for child in &cell_node.children {
                    self.append(document, *child, block_content, style.clone(), &mut content)?;
                }
                if cell_node.children.is_empty() {
                    content.spans.push(TextSpan {
                        node: cell_id,
                        range: 0..0,
                        style: style.highlight,
                        font_family: style.font_family,
                    });
                }
                output.text.push_str(&content.text);
                output
                    .inline_math
                    .extend(content.inline_math.iter().cloned().map(|mut span| {
                        span.range = cell_start + span.range.start..cell_start + span.range.end;
                        span
                    }));
                output
                    .spans
                    .extend(content.spans.iter().cloned().map(|mut span| {
                        span.range = cell_start + span.range.start..cell_start + span.range.end;
                        span
                    }));
                output
                    .links
                    .extend(content.links.iter().cloned().map(|mut link| {
                        link.range = cell_start + link.range.start..cell_start + link.range.end;
                        link
                    }));
                output
                    .anchors
                    .extend(content.anchors.iter().cloned().map(|mut anchor| {
                        anchor.offset += cell_start;
                        anchor
                    }));
                #[cfg(feature = "syntax-highlighting")]
                output.highlights.extend(content.highlights);
                let mut projection = Projection {
                    inline_math: content.inline_math,
                    cells: content.cells,
                    anchors: content.anchors,
                    text: content.text.into(),
                    spans: content.spans,
                    links: content.links,
                    blocks: content.blocks,
                    parts: Vec::new(),
                };
                projection.prepare_parts();
                output.cells.insert(cell_id, Arc::new(projection));
                cells.push(crate::TableCellSpan {
                    node: cell_id,
                    range: cell_start..output.text.len(),
                });
            }
            rows.push(crate::TableRowSpan {
                node: row_id,
                header,
                cells,
            });
        }
        output.blocks.push(BlockSpan {
            footnote: output.footnote,
            indent: output.indent,
            quote_depth: output.quote_depth,
            quote_path: output.quote_path.clone(),
            node: id,
            range: start..output.text.len(),
            kind: BlockKind::Table(crate::TableBlock { columns, rows }),
        });
        Ok(())
    }
}

/// A source node's UTF-8 byte range in projected text. List markers belong to
/// their item node; inter-block separators have no source ID.
#[derive(Clone, Debug)]
pub struct TextSpan {
    pub node: NodeId,
    pub range: Range<usize>,
    pub style: HighlightStyle,
    pub font_family: Option<gpui::FontFamily>,
}

/// Immutable text and source mapping, compiled outside the paint path.
#[derive(Default, Debug)]
pub struct Projection {
    pub(crate) inline_math: Vec<InlineMathSpan>,
    cells: HashMap<NodeId, Arc<Projection>>,
    anchors: Vec<crate::AnchorSpan>,
    text: SharedString,
    spans: Vec<TextSpan>,
    links: Vec<LinkSpan>,
    blocks: Vec<BlockSpan>,
    pub(crate) parts: Vec<RenderPart>,
}

#[derive(Clone, Debug)]
pub(crate) struct InlineMathSpan {
    pub node: NodeId,
    pub range: Range<usize>,
    pub formula: crate::MathInline,
}

#[derive(Debug)]
pub(crate) struct CellProjection {
    pub node: NodeId,
    pub offset: usize,
    pub projection: Arc<Projection>,
}

#[derive(Debug)]
pub(crate) struct RowProjection {
    pub node: NodeId,
    pub header: bool,
    pub cells: Vec<CellProjection>,
}

#[derive(Debug)]
pub(crate) enum RenderPart {
    Text(CellProjection),
    Embed(BlockSpan),
    Table {
        footnote: bool,
        indent: usize,
        quote_depth: usize,
        node: NodeId,
        columns: Vec<crate::TableColumn>,
        rows: Vec<RowProjection>,
    },
}

/// A leaf block's range, excluding separators inserted between blocks.
#[derive(Clone, Debug)]
pub struct BlockSpan {
    pub footnote: bool,
    pub indent: usize,
    pub quote_depth: usize,
    pub(crate) quote_path: Vec<NodeId>,
    pub node: NodeId,
    pub range: Range<usize>,
    pub kind: BlockKind,
}

#[derive(Clone, Debug)]
pub enum BlockKind {
    Graphic {
        graphic: crate::Graphic,
        /// A code header and surface are present only for a rendered code block.
        code_language: Option<SharedString>,
    },
    Details {
        details: crate::Details,
        body: Arc<Projection>,
    },
    Alert {
        alert: crate::Alert,
        body: Arc<Projection>,
    },
    DefinitionTerm,
    Rule,
    Image(crate::Image),
    Table(crate::TableBlock),
    Paragraph,
    Heading {
        level: HeadingLevel,
    },
    Quote,
    ListItem {
        marker: ListMarker,
    },
    Code {
        language: Option<SharedString>,
    },
}

/// An actionable range in the projected text; the destination is opaque to GPUI.
#[derive(Clone, Debug)]
pub struct LinkSpan {
    pub kind: crate::LinkKind,
    pub node: NodeId,
    pub target: SharedString,
    /// Optional destination description supplied by the document.
    pub title: Option<SharedString>,
    pub range: Range<usize>,
}

impl Projection {
    fn slice(&self, range: Range<usize>) -> Arc<Self> {
        let first_span = self
            .spans
            .partition_point(|span| span.range.end < range.start);
        let first_link = self
            .links
            .partition_point(|link| link.range.end <= range.start);
        let first_block = self
            .blocks
            .partition_point(|block| block.range.end < range.start);
        let intersects = |candidate: &Range<usize>| {
            candidate.start < range.end && candidate.end > range.start
                || candidate.is_empty()
                    && candidate.start >= range.start
                    && candidate.start <= range.end
        };
        Arc::new(Self {
            inline_math: self
                .inline_math
                .iter()
                .filter(|span| span.range.start >= range.start && span.range.end <= range.end)
                .cloned()
                .map(|mut span| {
                    span.range = span.range.start - range.start..span.range.end - range.start;
                    span
                })
                .collect(),
            cells: HashMap::new(),
            anchors: self
                .anchors
                .iter()
                .filter(|anchor| anchor.offset >= range.start && anchor.offset < range.end)
                .cloned()
                .map(|mut anchor| {
                    anchor.offset -= range.start;
                    anchor
                })
                .collect(),
            text: self.text[range.clone()].to_owned().into(),
            spans: self.spans[first_span..]
                .iter()
                .take_while(|span| span.range.start <= range.end)
                .filter(|span| intersects(&span.range))
                .map(|span| {
                    let mut span = span.clone();
                    span.range = span.range.start.max(range.start) - range.start
                        ..span.range.end.min(range.end) - range.start;
                    span
                })
                .collect(),
            links: self.links[first_link..]
                .iter()
                .take_while(|span| span.range.start < range.end)
                .filter(|span| intersects(&span.range))
                .map(|span| {
                    let mut span = span.clone();
                    span.range = span.range.start.max(range.start) - range.start
                        ..span.range.end.min(range.end) - range.start;
                    span
                })
                .collect(),
            blocks: self.blocks[first_block..]
                .iter()
                .take_while(|block| block.range.start <= range.end)
                .filter(|block| {
                    !matches!(
                        block.kind,
                        BlockKind::Table(_)
                            | BlockKind::Graphic { .. }
                            | BlockKind::Rule
                            | BlockKind::Image(_)
                            | BlockKind::Alert { .. }
                            | BlockKind::Details { .. }
                    ) && block.range.start >= range.start
                        && block.range.end <= range.end
                })
                .map(|block| {
                    let mut block = block.clone();
                    block.range = block.range.start - range.start..block.range.end - range.start;
                    block
                })
                .collect(),
            parts: Vec::new(),
        })
    }

    fn prepare_parts(&mut self) {
        if !self.blocks.iter().any(|block| {
            matches!(
                block.kind,
                BlockKind::Table(_)
                    | BlockKind::Graphic { .. }
                    | BlockKind::Rule
                    | BlockKind::Image(_)
                    | BlockKind::Alert { .. }
                    | BlockKind::Details { .. }
            )
        }) {
            return;
        }
        let mut parts = Vec::new();
        let mut first = 0;
        for (index, block) in self.blocks.iter().enumerate() {
            if !matches!(
                block.kind,
                BlockKind::Table(_)
                    | BlockKind::Graphic { .. }
                    | BlockKind::Rule
                    | BlockKind::Image(_)
                    | BlockKind::Alert { .. }
                    | BlockKind::Details { .. }
            ) {
                continue;
            }
            if first < index {
                let start = &self.blocks[first];
                parts.push(RenderPart::Text(CellProjection {
                    node: start.node,
                    offset: start.range.start,
                    projection: self.slice(start.range.start..self.blocks[index - 1].range.end),
                }));
            }
            if let BlockKind::Table(table) = &block.kind {
                parts.push(RenderPart::Table {
                    footnote: block.footnote,
                    indent: block.indent,
                    quote_depth: block.quote_depth,
                    node: block.node,
                    columns: table.columns.clone(),
                    rows: table
                        .rows
                        .iter()
                        .map(|row| RowProjection {
                            node: row.node,
                            header: row.header,
                            cells: row
                                .cells
                                .iter()
                                .map(|cell| CellProjection {
                                    node: cell.node,
                                    offset: cell.range.start,
                                    projection: self.cells[&cell.node].clone(),
                                })
                                .collect(),
                        })
                        .collect(),
                });
            } else {
                parts.push(RenderPart::Embed(block.clone()));
            }
            first = index + 1;
        }
        if first < self.blocks.len() {
            let start = &self.blocks[first];
            parts.push(RenderPart::Text(CellProjection {
                node: start.node,
                offset: start.range.start,
                projection: self.slice(start.range.start..self.blocks.last().unwrap().range.end),
            }));
        }
        self.parts = parts;
    }

    pub fn text(&self) -> &SharedString {
        &self.text
    }

    pub fn anchors(&self) -> &[crate::AnchorSpan] {
        &self.anchors
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

    /// Map a source-relative UTF-8 position to the shared selection stream.
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
        let marker_nodes: std::collections::HashSet<_> = self
            .blocks
            .iter()
            .filter_map(|block| {
                matches!(block.kind, BlockKind::ListItem { .. }).then_some(block.node)
            })
            .collect();
        StyledText::new(self.text.clone())
            .with_highlights(
                self.spans
                    .iter()
                    .filter(|span| !span.range.is_empty())
                    .map(|span| {
                        let style = if marker_nodes.contains(&span.node) {
                            HighlightStyle {
                                fade_out: Some(1.),
                                ..Default::default()
                            }
                        } else {
                            span.style
                        };
                        (span.range.clone(), style)
                    }),
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
