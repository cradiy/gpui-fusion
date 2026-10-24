//! Markdown conversion into editable-by-the-producer rich-text node snapshots.
//! Images occupy their own blocks. HTML is displayed as literal text.
use crate::{compose::*, *};
use anyhow::{Result, bail};
use pulldown_cmark::{Event, Options, Parser, Tag};
mod footnotes;
mod stream;
pub use stream::MarkdownStream;

struct Item {
    reference: Option<(usize, usize)>,
    tag: Option<Tag<'static>>,
    event: Option<Event<'static>>,
    children: Vec<Item>,
}

/// Parse paragraphs, headings, lists, quotations, alerts, definition lists,
/// tables, images, fenced code and numbered footnotes with return links.
/// Parsing performs no asset loading or URL navigation.
pub fn parse(source: &str) -> Result<Document> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_FOOTNOTES;
    let mut stack = vec![Item {
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
                reference: None,
                tag: None,
                event: Some(event.into_static()),
                children: Vec::new(),
            }),
        }
    }
    Document::from_roots(footnotes::convert(stack.pop().unwrap().children)?)
}

fn plain(items: &[Item]) -> String {
    let mut result = String::new();
    for item in items {
        match &item.event {
            Some(Event::Text(value) | Event::Code(value)) => result.push_str(value),
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
        Some(Tag::Link { dest_url, .. }) => link(dest_url.to_string()),
        Some(_) => bail!("unsupported block inside inline Markdown content"),
        None => {
            return Ok(match &item.event {
                Some(Event::Text(value) | Event::Html(value) | Event::InlineHtml(value)) => {
                    Some(text(value.to_string()))
                }
                Some(Event::Code(value)) => Some(code().child(text(value.to_string()))),
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
        let block = match &item.tag {
            Some(Tag::Paragraph) => {
                flush(&mut result, &mut pending);
                result.extend(blocks(&item.children)?);
                continue;
            }
            Some(Tag::Heading { level, .. }) => Some(
                heading(match level {
                    pulldown_cmark::HeadingLevel::H1 => HeadingLevel::H1,
                    pulldown_cmark::HeadingLevel::H2 => HeadingLevel::H2,
                    pulldown_cmark::HeadingLevel::H3 => HeadingLevel::H3,
                    pulldown_cmark::HeadingLevel::H4 => HeadingLevel::H4,
                    pulldown_cmark::HeadingLevel::H5 => HeadingLevel::H5,
                    pulldown_cmark::HeadingLevel::H6 => HeadingLevel::H6,
                })
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
                let mut block = CodeBlock::new(plain(&item.children));
                if let pulldown_cmark::CodeBlockKind::Fenced(language) = kind {
                    if let Some(language) = language.split_whitespace().next() {
                        block = block.language(language.to_owned());
                    }
                }
                Some(node(block))
            }
            Some(Tag::Image { dest_url, .. }) => Some(node(Image {
                source: dest_url.to_string().into(),
                alt: plain(&item.children).into(),
            })),
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
                        cells.push(table_cell().children(inlines(&cell.children)?));
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
