use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Keyboard,
    Mark,
    Underline,
}

impl Kind {
    pub(super) fn node(self) -> NodeTree {
        match self {
            Self::Keyboard => keyboard(),
            Self::Mark => mark(),
            Self::Underline => underline(),
        }
    }
}

// Pair tags within one Markdown container; unmatched tags remain literal.
pub(super) fn normalize(items: Vec<Item>, depth: usize) -> Result<Vec<Item>> {
    if depth > 256 {
        bail!("Markdown exceeds nesting limits");
    }
    let mut result = Vec::new();
    let mut stack: Vec<(Kind, Item)> = Vec::new();
    for mut item in items {
        item.children = normalize(item.children, depth + 1)?;
        if let Some(Event::InlineHtml(raw)) = &item.event {
            let token = match raw.as_ref() {
                "<kbd>" => Some((Kind::Keyboard, true)),
                "</kbd>" => Some((Kind::Keyboard, false)),
                "<mark>" => Some((Kind::Mark, true)),
                "</mark>" => Some((Kind::Mark, false)),
                "<u>" => Some((Kind::Underline, true)),
                "</u>" => Some((Kind::Underline, false)),
                "<br>" | "<br/>" | "<br />" => {
                    item.event = Some(Event::HardBreak);
                    None
                }
                _ => None,
            };
            if let Some((kind, open)) = token {
                if open {
                    if depth + stack.len() >= 256 {
                        bail!("Markdown exceeds nesting limits");
                    }
                    stack.push((kind, item));
                    continue;
                }
                if stack.last().is_some_and(|(last, _)| *last == kind) {
                    let (_, mut container) = stack.pop().unwrap();
                    container.event = None;
                    container.inline_kind = Some(kind);
                    append(&mut result, &mut stack, container);
                    continue;
                }
            }
        }
        append(&mut result, &mut stack, item);
    }
    while let Some((_, mut unclosed)) = stack.pop() {
        let children = std::mem::take(&mut unclosed.children);
        append(&mut result, &mut stack, unclosed);
        for child in children {
            append(&mut result, &mut stack, child);
        }
    }
    Ok(result)
}

fn append(result: &mut Vec<Item>, stack: &mut [(Kind, Item)], item: Item) {
    if let Some((_, parent)) = stack.last_mut() {
        parent.children.push(item);
    } else {
        result.push(item);
    }
}
