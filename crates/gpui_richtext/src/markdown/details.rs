use super::*;

enum Piece {
    Item(Item),
    Open(bool, usize),
    Summary {
        label: String,
        raw: String,
        offset: usize,
    },
    Close(usize),
}

/// Recognize block-level disclosure tags without enabling general HTML rendering.
pub(super) fn normalize(items: Vec<Item>, depth: usize) -> Result<Vec<Item>> {
    if depth > 256 {
        bail!("Markdown exceeds nesting limits");
    }
    let mut pieces = Vec::new();
    for mut item in items {
        if matches!(item.tag, Some(Tag::HtmlBlock)) {
            let raw: String = item
                .children
                .iter()
                .filter_map(|child| match &child.event {
                    Some(Event::Html(text)) => Some(text.as_ref()),
                    _ => None,
                })
                .collect();
            let mut buffer = String::new();
            let mut offset = item.offset;
            let mut buffer_offset = offset;
            let mut fence: Option<(u8, usize)> = None;
            let mut parts = Vec::new();
            for line in raw.split_inclusive('\n') {
                let line_offset = offset;
                offset += line.len();
                if buffer.is_empty() {
                    buffer_offset = line_offset;
                }
                let trimmed = line.trim();
                let bytes = trimmed.as_bytes();
                let marker = bytes.first().copied().filter(|b| matches!(b, b'`' | b'~'));
                if let Some(marker) = marker {
                    let count = bytes.iter().take_while(|byte| **byte == marker).count();
                    if let Some((open, length)) = fence {
                        if marker == open && count >= length && trimmed[count..].trim().is_empty() {
                            fence = None;
                        }
                    } else if count >= 3 {
                        fence = Some((marker, count));
                    }
                    buffer.push_str(line);
                    continue;
                }
                let token = if fence.is_some() {
                    None
                } else {
                    match trimmed {
                        "<details>" => Some(Piece::Open(false, line_offset)),
                        "<details open>" => Some(Piece::Open(true, line_offset)),
                        "</details>" => Some(Piece::Close(line_offset)),
                        _ => trimmed
                            .strip_prefix("<summary>")
                            .and_then(|value| value.strip_suffix("</summary>"))
                            .map(|summary| {
                                let label = Parser::new(summary)
                                    .filter_map(|event| match event {
                                        Event::Text(text) | Event::Code(text) => {
                                            Some(text.to_string())
                                        }
                                        Event::SoftBreak | Event::HardBreak => Some(" ".into()),
                                        _ => None,
                                    })
                                    .collect();
                                Piece::Summary {
                                    offset: line_offset,
                                    label,
                                    raw: line.to_owned(),
                                }
                            }),
                    }
                };
                if let Some(token) = token {
                    if !buffer.trim().is_empty() {
                        parts.push(Piece::Item(raw_item(
                            std::mem::take(&mut buffer),
                            buffer_offset,
                        )));
                    }
                    buffer.clear();
                    parts.push(token);
                } else {
                    buffer.push_str(line);
                }
            }
            if parts.is_empty() {
                pieces.push(Piece::Item(item));
            } else {
                if !buffer.trim().is_empty() {
                    parts.push(Piece::Item(raw_item(buffer, buffer_offset)));
                }
                for part in parts {
                    if let Piece::Item(raw) = part {
                        let Some(Event::Html(text)) = raw.event else {
                            unreachable!()
                        };
                        pieces.extend(
                            parse_items(&text, depth + 1, raw.offset)?
                                .into_iter()
                                .map(Piece::Item),
                        );
                    } else {
                        pieces.push(part);
                    }
                }
            }
        } else {
            item.children = normalize(item.children, depth + 1)?;
            pieces.push(Piece::Item(item));
        }
    }
    let mut result = Vec::new();
    let mut stack: Vec<Item> = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Open(open, offset) => {
                if stack.len() + depth >= 256 {
                    anyhow::bail!("Markdown exceeds nesting limits");
                }
                stack.push(Item {
                    offset,
                    inline_kind: None,
                    details: Some(Details::new("").open(open)),
                    reference: None,
                    tag: None,
                    event: None,
                    children: Vec::new(),
                });
            }
            Piece::Summary { label: summary, .. } if !stack.is_empty() => {
                stack.last_mut().unwrap().details.as_mut().unwrap().summary = summary.into();
            }
            Piece::Close(_) if !stack.is_empty() => {
                let item = stack.pop().unwrap();
                push(&mut result, &mut stack, item);
            }
            Piece::Item(item) => push(&mut result, &mut stack, item),
            Piece::Close(offset) => push(
                &mut result,
                &mut stack,
                raw_item("</details>".into(), offset),
            ),
            Piece::Summary { raw, offset, .. } => {
                push(&mut result, &mut stack, raw_item(raw, offset))
            }
        }
    }
    while let Some(item) = stack.pop() {
        push(&mut result, &mut stack, item);
    }
    Ok(result)
}

fn push(result: &mut Vec<Item>, stack: &mut [Item], item: Item) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(item);
    } else {
        result.push(item);
    }
}

fn raw_item(text: String, offset: usize) -> Item {
    Item {
        offset,
        inline_kind: None,
        details: None,
        reference: None,
        tag: None,
        event: Some(Event::Html(text.into())),
        children: Vec::new(),
    }
}
