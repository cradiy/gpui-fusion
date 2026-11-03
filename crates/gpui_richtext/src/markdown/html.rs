use super::*;
use html5ever::tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer};
use std::cell::RefCell;

fn raw(item: &Item) -> Option<&str> {
    match &item.event {
        Some(Event::Html(value) | Event::InlineHtml(value)) => Some(value),
        _ => None,
    }
}

// Keep block source positions intact for disclosure reparsing and streaming IDs.
pub(super) fn comments(items: Vec<Item>, depth: usize) -> Result<Vec<Item>> {
    if depth > 256 {
        bail!("Markdown exceeds nesting limits");
    }
    let mut result = Vec::new();
    let mut in_comment = false;
    for mut item in items {
        if matches!(item.tag, Some(Tag::HtmlBlock)) {
            let value = item.children.iter().filter_map(raw).collect::<String>();
            item.children = vec![leaf(Event::Html(value.into()), item.offset)];
        }
        item.children = comments(item.children, depth + 1)?;
        if let Some(value) = raw(&item) {
            let mut bytes = value.as_bytes().to_vec();
            let mut cursor = 0;
            while cursor < bytes.len() {
                if in_comment || value[cursor..].starts_with("<!--") {
                    let end = value[cursor..].find("-->").map(|end| cursor + end + 3);
                    in_comment = end.is_none();
                    let end = end.unwrap_or(bytes.len());
                    for byte in &mut bytes[cursor..end] {
                        if *byte != b'\n' && *byte != b'\r' {
                            *byte = b' ';
                        }
                    }
                    cursor = end;
                } else if bytes[cursor] == b'<' {
                    cursor = tag_end(value, cursor).unwrap_or(bytes.len());
                } else {
                    cursor += value[cursor..].chars().next().unwrap().len_utf8();
                }
            }
            let cleaned = String::from_utf8(bytes).unwrap();
            if cleaned.trim().is_empty() && matches!(item.event, Some(Event::InlineHtml(_))) {
                continue;
            }
            item.event = Some(if matches!(item.event, Some(Event::InlineHtml(_))) {
                Event::InlineHtml(cleaned.into())
            } else {
                Event::Html(cleaned.into())
            });
        }
        result.push(item);
    }
    Ok(result)
}

fn tag_end(value: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (index, byte) in value.bytes().enumerate().skip(start + 1) {
        match (quote, byte) {
            (Some(open), close) if open == close => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Some(index + 1),
            _ => {}
        }
    }
    None
}

#[derive(Default)]
struct ImageTag(RefCell<Option<(String, String, String)>>);
impl TokenSink for ImageTag {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        if let Token::TagToken(tag) = token {
            if tag.kind == TagKind::StartTag && &*tag.name == "img" {
                let attr = |name: &str| {
                    tag.attrs
                        .iter()
                        .find(|attr| &*attr.name.local == name)
                        .map(|attr| attr.value.to_string())
                        .unwrap_or_default()
                };
                let source = attr("src");
                if !source.trim().is_empty() {
                    *self.0.borrow_mut() = Some((source, attr("alt"), attr("title")));
                }
            }
        }
        TokenSinkResult::Continue
    }
}

fn image_tag(value: &str, offset: usize) -> Option<Item> {
    if !value.get(..4)?.eq_ignore_ascii_case("<img")
        || !value
            .as_bytes()
            .get(4)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'>' || *byte == b'/')
    {
        return None;
    }
    let tokenizer = Tokenizer::new(ImageTag::default(), Default::default());
    let input = BufferQueue::default();
    input.push_back(value.into());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let (source, alt, title) = tokenizer.sink.0.into_inner()?;
    Some(Item {
        event: None,
        tag: Some(Tag::Image {
            link_type: pulldown_cmark::LinkType::Inline,
            dest_url: source.into(),
            title: title.into(),
            id: "".into(),
        }),
        children: vec![leaf(Event::Text(alt.into()), offset)],
        ..leaf(Event::Text("".into()), offset)
    })
}

fn leaf(event: Event<'static>, offset: usize) -> Item {
    Item {
        offset,
        inline_kind: None,
        details: None,
        reference: None,
        tag: None,
        event: Some(event),
        children: Vec::new(),
    }
}

pub(super) fn images(items: Vec<Item>, depth: usize) -> Result<Vec<Item>> {
    if depth > 256 {
        bail!("Markdown exceeds nesting limits");
    }
    let mut result = Vec::new();
    for mut item in items {
        // HTML block events may split a tag across multiple source lines.
        if matches!(item.tag, Some(Tag::HtmlBlock)) {
            let value = item.children.iter().filter_map(raw).collect::<String>();
            item.children = vec![leaf(Event::Html(value.into()), item.offset)];
        }
        item.children = images(item.children, depth + 1)?;
        let Some(value) = raw(&item) else {
            result.push(item);
            continue;
        };
        let mut cursor = 0;
        let mut pending = 0;
        while let Some(relative) = value[cursor..].find('<') {
            let start = cursor + relative;
            let Some(end) = tag_end(value, start) else {
                break;
            };
            if let Some(image) = image_tag(&value[start..end], item.offset + start) {
                if !value[pending..start].trim().is_empty() {
                    result.push(leaf(
                        Event::Html(value[pending..start].to_owned().into()),
                        item.offset + pending,
                    ));
                }
                result.push(image);
                pending = end;
            }
            cursor = end;
        }
        if !value[pending..].trim().is_empty() {
            let event = if matches!(item.event, Some(Event::InlineHtml(_))) {
                Event::InlineHtml(value[pending..].to_owned().into())
            } else {
                Event::Html(value[pending..].to_owned().into())
            };
            result.push(leaf(event, item.offset + pending));
        }
    }
    Ok(result)
}
