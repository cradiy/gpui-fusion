use super::*;

enum Piece {
    Item(Item),
    Open(bool),
    Summary { label: String, raw: String },
    Close,
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
            let mut fence: Option<(u8, usize)> = None;
            let mut parts = Vec::new();
            for line in raw.split_inclusive('\n') {
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
                        "<details>" => Some(Piece::Open(false)),
                        "<details open>" => Some(Piece::Open(true)),
                        "</details>" => Some(Piece::Close),
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
                                    label,
                                    raw: line.to_owned(),
                                }
                            }),
                    }
                };
                if let Some(token) = token {
                    if !buffer.trim().is_empty() {
                        parts.push(Piece::Item(raw_item(std::mem::take(&mut buffer))));
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
                    parts.push(Piece::Item(raw_item(buffer)));
                }
                for part in parts {
                    if let Piece::Item(raw) = part {
                        let Some(Event::Html(text)) = raw.event else {
                            unreachable!()
                        };
                        pieces.extend(parse_items(&text, depth + 1)?.into_iter().map(Piece::Item));
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
            Piece::Open(open) => {
                if stack.len() + depth >= 256 {
                    anyhow::bail!("Markdown exceeds nesting limits");
                }
                stack.push(Item {
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
            Piece::Close if !stack.is_empty() => {
                let item = stack.pop().unwrap();
                push(&mut result, &mut stack, item);
            }
            Piece::Item(item) => push(&mut result, &mut stack, item),
            Piece::Close => push(&mut result, &mut stack, raw_item("</details>".into())),
            Piece::Summary { raw, .. } => push(&mut result, &mut stack, raw_item(raw)),
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

fn raw_item(text: String) -> Item {
    Item {
        details: None,
        reference: None,
        tag: None,
        event: Some(Event::Html(text.into())),
        children: Vec::new(),
    }
}
