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
        target: SharedString,
        text: Option<SharedString>,
        style: InlineStyle,
    },
}

#[derive(Default)]
struct Compilation {
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

/// View-specific typed node resolution. Unknown payloads are errors.
///
/// Resolving a document creates an immutable projection; replacing registrations
/// affects only subsequently compiled projections.
#[derive(Default)]
pub struct Registry {
    resolvers: HashMap<TypeId, Resolver>,
    #[cfg(feature = "syntax-highlighting")]
    highlighter: Option<Arc<crate::syntax::SyntaxHighlighter>>,
}

impl Registry {
    #[cfg(feature = "syntax-highlighting")]
    pub fn syntax_highlighter(
        mut self,
        highlighter: Option<Arc<crate::syntax::SyntaxHighlighter>>,
    ) -> Self {
        self.highlighter = highlighter;
        self
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
        registry.register::<crate::Image>(|image| Ok(Contribution::Image(image.clone())));
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
    /// Blocks are separated by two newlines; consecutive list items use one.
    /// Styling boundaries do not add text or interrupt shaping explicitly.
    pub fn compile(&self, document: &Document) -> Result<Projection> {
        let mut output = Compilation::default();
        for root in document.roots.iter() {
            self.append(document, *root, true, InlineStyle::default(), &mut output)?;
        }
        let mut projection = Projection {
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
        if root
            != matches!(
                contribution,
                Contribution::Paragraph(_)
                    | Contribution::Rule
                    | Contribution::Image(_)
                    | Contribution::BlockQuote(_)
                    | Contribution::ListEntry { .. }
                    | Contribution::Table(_)
                    | Contribution::Heading { .. }
                    | Contribution::Quote(_)
                    | Contribution::ListItem { .. }
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
            leaf @ (Contribution::Rule | Contribution::Image(_)) => {
                if !node.children.is_empty() {
                    bail!("leaf block {id:?} cannot have children");
                }
                let start = output.start_block();
                let kind = match leaf {
                    Contribution::Image(image) => {
                        output.text.push_str(&image.alt);
                        BlockKind::Image(image)
                    }
                    _ => BlockKind::Rule,
                };
                output.blocks.push(BlockSpan {
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
            Contribution::ListEntry { marker, style } => {
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
            }
            Contribution::Table(table) => {
                self.append_table(document, id, table, inherited, output)?
            }
            Contribution::TableRow { .. } | Contribution::TableCell(_) => {
                bail!("table row or cell {id:?} outside its table parent");
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
            | Contribution::Heading { .. }
            | Contribution::Quote(_)
            | Contribution::ListItem { .. }) => {
                let (kind, style, prefix) = match block {
                    Contribution::Paragraph(style) => (BlockKind::Paragraph, style.into(), None),
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
                        block.highlights = highlighter.highlight(&block.text, language)?;
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
                for child in &cell_node.children {
                    self.append(document, *child, false, style.clone(), output)?;
                }
                if cell_node.children.is_empty() {
                    output.spans.push(TextSpan {
                        node: cell_id,
                        range: cell_start..cell_start,
                        style: style.highlight,
                        font_family: style.font_family,
                    });
                }
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
    pub font_family: Option<SharedString>,
}

/// Immutable text and source mapping, compiled outside the paint path.
pub struct Projection {
    text: SharedString,
    spans: Vec<TextSpan>,
    links: Vec<LinkSpan>,
    blocks: Vec<BlockSpan>,
    pub(crate) parts: Vec<RenderPart>,
}

pub(crate) struct CellProjection {
    pub node: NodeId,
    pub offset: usize,
    pub projection: Arc<Projection>,
}

pub(crate) struct RowProjection {
    pub node: NodeId,
    pub header: bool,
    pub cells: Vec<CellProjection>,
}

pub(crate) enum RenderPart {
    Text(CellProjection),
    Embed(BlockSpan),
    Table {
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
    pub indent: usize,
    pub quote_depth: usize,
    pub(crate) quote_path: Vec<NodeId>,
    pub node: NodeId,
    pub range: Range<usize>,
    pub kind: BlockKind,
}

#[derive(Clone, Debug)]
pub enum BlockKind {
    Rule,
    Image(crate::Image),
    Table(crate::TableBlock),
    Paragraph,
    Heading { level: HeadingLevel },
    Quote,
    ListItem { marker: ListMarker },
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
                        BlockKind::Table(_) | BlockKind::Rule | BlockKind::Image(_)
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
                BlockKind::Table(_) | BlockKind::Rule | BlockKind::Image(_)
            )
        }) {
            return;
        }
        let mut parts = Vec::new();
        let mut first = 0;
        for (index, block) in self.blocks.iter().enumerate() {
            if !matches!(
                block.kind,
                BlockKind::Table(_) | BlockKind::Rule | BlockKind::Image(_)
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
                                    projection: self.slice(cell.range.clone()),
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
