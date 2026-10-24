//! Markdown conversion into editable-by-the-producer rich-text node snapshots.
//! Images occupy their own blocks. Standalone details/summary tags form collapsible
//! blocks. Inline kbd, mark, u, and br tags are supported; other HTML is literal text.
use crate::{compose::*, *};
use anyhow::{Result, bail};
use pulldown_cmark::{Event, Options, Parser, Tag};
mod details;
mod footnotes;
mod front_matter;
pub use front_matter::{FrontMatter, FrontMatterFormat, ParsedMarkdown, parse_with_metadata};
mod headings;
mod inline_html;
mod stream;
pub use stream::MarkdownStream;

struct Item {
    inline_kind: Option<inline_html::Kind>,
    details: Option<Details>,
    reference: Option<(usize, usize)>,
    tag: Option<Tag<'static>>,
    event: Option<Event<'static>>,
    children: Vec<Item>,
}

/// Parse paragraphs, headings, lists, quotations, alerts, definition lists,
/// tables, images, fenced code and numbered footnotes with return links.
/// Parsing performs no asset loading or URL navigation.
/// Headings accept `{#id}`; otherwise IDs use lowercase letters and digits with
/// whitespace replaced by hyphens. Duplicate generated IDs receive numeric suffixes.
/// Fragment links resolve to heading IDs within this document.
/// Disclosure tags occupy their own lines; summaries use one line. An optional
/// `open` attribute on `<details>` sets the initial expanded state.
/// Attribute-free inline `kbd`, `mark`, and `u` pairs stay within a Markdown
/// container. Unmatched tags remain literal. Inline `br` inserts a newline.
/// Leading YAML or TOML front matter is excluded from the rendered body.
/// Use `parse_with_metadata` to retain its raw contents and completion state.
pub fn parse(source: &str) -> Result<Document> {
    Ok(parse_with_metadata(source)?.document)
}

fn parse_body(source: &str) -> Result<Document> {
    let mut items = inline_html::normalize(parse_items(source, 0)?, 0)?;
    headings::assign(&mut items)?;
    Document::from_roots(footnotes::convert(items)?)
}

fn parse_items(source: &str, depth: usize) -> Result<Vec<Item>> {
    if depth > 256 {
        bail!("Markdown exceeds nesting limits");
    }
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_HEADING_ATTRIBUTES;
    #[cfg(feature = "math")]
    let options = options | Options::ENABLE_MATH;
    let mut stack = vec![Item {
        inline_kind: None,
        details: None,
        reference: None,
        tag: None,
        event: None,
        children: Vec::new(),
    }];
    for (count, event) in Parser::new_ext(source, options).enumerate() {
        if count >= 100_000 || stack.len() > 256 {
            bail!("Markdown exceeds document limits");
        }
        match event {
            Event::Start(tag) => stack.push(Item {
                inline_kind: None,
                details: None,
                reference: None,
                tag: Some(tag.into_static()),
                event: None,
                children: Vec::new(),
            }),
            Event::End(_) => {
                let item = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(item);
            }
            event => stack.last_mut().unwrap().children.push(Item {
                inline_kind: None,
                details: None,
                reference: None,
                tag: None,
                event: Some(event.into_static()),
                children: Vec::new(),
            }),
        }
    }
    details::normalize(stack.pop().unwrap().children, depth)
}

fn plain(items: &[Item]) -> String {
    let mut result = String::new();
    for item in items {
        match &item.event {
            Some(Event::Text(value) | Event::Code(value)) => result.push_str(value),
            Some(Event::InlineMath(value) | Event::DisplayMath(value)) => result.push_str(value),
            Some(Event::SoftBreak | Event::HardBreak) => result.push('\n'),
            _ => result.push_str(&plain(&item.children)),
        }
    }
    result
}

fn task(items: &[Item]) -> Option<bool> {
    items.iter().find_map(|item| match item.event {
        Some(Event::TaskListMarker(checked)) => Some(checked),
        _ if matches!(item.tag, Some(Tag::List(_))) => None,
        _ => task(&item.children),
    })
}

fn inline(item: &Item) -> Result<Option<NodeTree>> {
    if let Some(kind) = item.inline_kind {
        return Ok(Some(kind.node().children(inlines(&item.children)?)));
    }
    if let Some((number, occurrence)) = item.reference {
        return Ok(Some(
            anchor(format!("footnote-ref-{number}-{occurrence}")).child(
                anchor_link(format!("footnote-{number}")).child(text(format!("[{number}]"))),
            ),
        ));
    }
    let container = match &item.tag {
        Some(Tag::Emphasis) => emphasis(),
        Some(Tag::Strong) => strong(),
        Some(Tag::Strikethrough) => strikethrough(),
        Some(Tag::Link {
            dest_url, title, ..
        }) => markdown_link(dest_url, title),
        Some(_) => bail!("unsupported block inside inline Markdown content"),
        None => {
            return Ok(match &item.event {
                Some(Event::Text(value) | Event::Html(value) | Event::InlineHtml(value)) => {
                    Some(text(value.to_string()))
                }
                Some(Event::Code(value)) => Some(code().child(text(value.to_string()))),
                Some(Event::InlineMath(value)) => Some(text(format!("${value}$"))),
                Some(Event::DisplayMath(value)) => Some(text(format!("$${value}$$"))),
                Some(Event::SoftBreak) => Some(text(" ")),
                Some(Event::HardBreak) => Some(hard_break()),
                Some(Event::TaskListMarker(_)) => None,
                Some(Event::FootnoteReference(label)) => Some(text(format!("[^{label}]"))),
                _ => bail!("unsupported Markdown event"),
            });
        }
    };
    Ok(Some(container.children(inlines(&item.children)?)))
}

fn inlines(items: &[Item]) -> Result<Vec<NodeTree>> {
    items
        .iter()
        .filter_map(|item| inline(item).transpose())
        .collect()
}

fn blocks(items: &[Item]) -> Result<Vec<NodeTree>> {
    let mut result = Vec::new();
    let mut pending = Vec::new();
    for item in items {
        if let Some(details) = &item.details {
            flush(&mut result, &mut pending);
            result.push(node(details.clone()).children(blocks(&item.children)?));
            continue;
        }
        if (item.inline_kind.is_some()
            || matches!(
                item.tag,
                Some(Tag::Link { .. } | Tag::Strong | Tag::Emphasis | Tag::Strikethrough)
            ))
            && contains_image(item)
        {
            for part in split_images(item, None)? {
                match part {
                    ImagePart::Inline(inline) => pending.push(inline),
                    ImagePart::Image(image) => {
                        flush(&mut result, &mut pending);
                        result.push(node(image));
                    }
                }
            }
            continue;
        }
        let block = match &item.tag {
            Some(Tag::Paragraph) => {
                flush(&mut result, &mut pending);
                result.extend(blocks(&item.children)?);
                continue;
            }
            Some(Tag::Heading { level, id, .. }) => Some(
                heading(match level {
                    pulldown_cmark::HeadingLevel::H1 => HeadingLevel::H1,
                    pulldown_cmark::HeadingLevel::H2 => HeadingLevel::H2,
                    pulldown_cmark::HeadingLevel::H3 => HeadingLevel::H3,
                    pulldown_cmark::HeadingLevel::H4 => HeadingLevel::H4,
                    pulldown_cmark::HeadingLevel::H5 => HeadingLevel::H5,
                    pulldown_cmark::HeadingLevel::H6 => HeadingLevel::H6,
                })
                .child(anchor(format!(
                    "heading:{}",
                    id.as_deref().unwrap_or_default()
                )))
                .children(inlines(&item.children)?),
            ),
            Some(Tag::BlockQuote(kind)) => Some(match kind {
                Some(kind) => node(Alert::new(match kind {
                    pulldown_cmark::BlockQuoteKind::Note => AlertKind::Note,
                    pulldown_cmark::BlockQuoteKind::Tip => AlertKind::Tip,
                    pulldown_cmark::BlockQuoteKind::Important => AlertKind::Important,
                    pulldown_cmark::BlockQuoteKind::Warning => AlertKind::Warning,
                    pulldown_cmark::BlockQuoteKind::Caution => AlertKind::Caution,
                }))
                .children(blocks(&item.children)?),
                None => node(BlockQuote).children(blocks(&item.children)?),
            }),
            Some(Tag::DefinitionList) => Some(definition_list().children(blocks(&item.children)?)),
            Some(Tag::DefinitionListTitle) => {
                Some(definition_term().children(inlines(&item.children)?))
            }
            Some(Tag::DefinitionListDefinition) => {
                Some(definition_description().children(blocks(&item.children)?))
            }
            Some(Tag::List(start)) => {
                let mut entries = Vec::new();
                for (index, entry) in item.children.iter().enumerate() {
                    let marker = if let Some(checked) = task(&entry.children) {
                        ListMarker::Task { checked }
                    } else if let Some(start) = start {
                        ListMarker::Number(u32::try_from(start.saturating_add(index as u64))?)
                    } else {
                        ListMarker::Bullet
                    };
                    entries.push(node(ListEntry(marker)).children(blocks(&entry.children)?));
                }
                Some(group().children(entries))
            }
            Some(Tag::CodeBlock(kind)) => {
                #[cfg(feature = "mermaid")]
                if let pulldown_cmark::CodeBlockKind::Fenced(language) = kind {
                    if language.split_whitespace().next() == Some("mermaid") {
                        flush(&mut result, &mut pending);
                        result.push(node(crate::Mermaid(plain(&item.children).into())));
                        continue;
                    }
                }
                let mut block = CodeBlock::new(plain(&item.children));
                if let pulldown_cmark::CodeBlockKind::Fenced(language) = kind {
                    if let Some(language) = language.split_whitespace().next() {
                        block = block.language(language.to_owned());
                    }
                }
                Some(node(block))
            }
            Some(Tag::Image {
                dest_url, title, ..
            }) => {
                let mut image = Image::new(dest_url.to_string(), plain(&item.children));
                if !title.is_empty() {
                    image.title = Some(title.to_string().into());
                }
                Some(node(image))
            }
            Some(Tag::Table(alignments)) => {
                let columns = alignments.iter().map(|align| {
                    TableColumn::flex(1.).align(match align {
                        pulldown_cmark::Alignment::Center => gpui::TextAlign::Center,
                        pulldown_cmark::Alignment::Right => gpui::TextAlign::Right,
                        _ => gpui::TextAlign::Left,
                    })
                });
                let mut rows = Vec::new();
                for row in &item.children {
                    let mut cells = Vec::new();
                    for cell in &row.children {
                        let content = if cell.children.iter().any(contains_image) {
                            blocks(&cell.children)?
                        } else {
                            inlines(&cell.children)?
                        };
                        cells.push(table_cell().children(content));
                    }
                    rows.push(
                        node(TableRow {
                            header: matches!(row.tag, Some(Tag::TableHead)),
                        })
                        .children(cells),
                    );
                }
                Some(node(Table::new().columns(columns)).children(rows))
            }
            Some(Tag::HtmlBlock) => Some(paragraph().children(inlines(&item.children)?)),
            _ if matches!(item.event, Some(Event::Rule)) => Some(node(Rule)),
            #[cfg(feature = "math")]
            _ if matches!(item.event, Some(Event::DisplayMath(_))) => {
                let Some(Event::DisplayMath(source)) = &item.event else {
                    unreachable!()
                };
                Some(node(crate::MathBlock(source.to_string().into())))
            }
            _ => None,
        };
        if let Some(block) = block {
            flush(&mut result, &mut pending);
            result.push(block);
        } else if let Some(inline) = inline(item)? {
            pending.push(inline);
        }
    }
    flush(&mut result, &mut pending);
    Ok(result)
}

fn flush(result: &mut Vec<NodeTree>, pending: &mut Vec<NodeTree>) {
    if !pending.is_empty() {
        result.push(paragraph().children(std::mem::take(pending)));
    }
}

enum ImagePart {
    Inline(NodeTree),
    Image(Image),
}

fn contains_image(item: &Item) -> bool {
    matches!(item.tag, Some(Tag::Image { .. })) || item.children.iter().any(contains_image)
}

fn split_images(item: &Item, target: Option<(&str, &str)>) -> Result<Vec<ImagePart>> {
    if let Some(Tag::Image {
        dest_url, title, ..
    }) = &item.tag
    {
        let mut image = Image::new(dest_url.to_string(), plain(&item.children));
        image.link = target.map(|(target, _)| target.to_owned().into());
        image.link_title =
            target.and_then(|(_, title)| (!title.is_empty()).then(|| title.to_owned().into()));
        if let Some(fragment) = target.and_then(|(target, _)| target.strip_prefix('#')) {
            image.link = Some(headings::target(fragment).into());
            image.link_kind = LinkKind::Anchor;
        }
        if !title.is_empty() {
            image.title = Some(title.to_string().into());
        }
        return Ok(vec![ImagePart::Image(image)]);
    }
    if !contains_image(item) {
        return Ok(inline(item)?.into_iter().map(ImagePart::Inline).collect());
    }
    let target = if let Some(Tag::Link {
        dest_url, title, ..
    }) = &item.tag
    {
        Some((dest_url.as_ref(), title.as_ref()))
    } else {
        target
    };
    let mut parts = Vec::new();
    for child in &item.children {
        for part in split_images(child, target)? {
            parts.push(match part {
                ImagePart::Inline(child) => {
                    let wrapper = if let Some(kind) = item.inline_kind {
                        kind.node()
                    } else {
                        match &item.tag {
                            Some(Tag::Link {
                                dest_url, title, ..
                            }) => markdown_link(dest_url, title),
                            Some(Tag::Strong) => strong(),
                            Some(Tag::Emphasis) => emphasis(),
                            Some(Tag::Strikethrough) => strikethrough(),
                            _ => bail!("unsupported image container"),
                        }
                    };
                    ImagePart::Inline(wrapper.child(child))
                }
                image => image,
            });
        }
    }
    Ok(parts)
}

fn markdown_link(target: &str, title: &str) -> NodeTree {
    let title = (!title.is_empty()).then(|| title.to_owned().into());
    if let Some(fragment) = target.strip_prefix('#') {
        node(AnchorLink {
            target: headings::target(fragment).into(),
            title,
        })
    } else {
        node(Link {
            link: target.to_owned().into(),
            text: None,
            title,
        })
    }
}
