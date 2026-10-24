use super::*;
use std::collections::HashMap;

pub(super) fn convert(mut items: Vec<Item>) -> Result<Vec<NodeTree>> {
    let mut definitions = HashMap::new();
    extract(&mut items, &mut definitions);
    let mut order = Vec::new();
    let mut counts = HashMap::new();
    annotate(&mut items, &definitions, &mut order, &mut counts);
    let mut bodies = Vec::new();
    let mut index = 0;
    while index < order.len() {
        let label = order[index].clone();
        let mut definition = definitions.remove(&label).unwrap();
        // Keep known labels available while visiting references inside definitions.
        annotate(
            &mut definition.children,
            &definitions,
            &mut order,
            &mut counts,
        );
        bodies.push(definition);
        index += 1;
    }
    let mut result = blocks(&items)?;
    if !bodies.is_empty() {
        result.push(rule());
    }
    for (index, definition) in bodies.into_iter().enumerate() {
        let number = index + 1;
        let mut body = blocks(&definition.children)?;
        if body
            .first()
            .is_none_or(|node| !node.payload.is::<Paragraph>())
        {
            body.insert(0, paragraph());
        }
        body[0]
            .children
            .insert(0, anchor(format!("footnote-{number}")));
        let returns = (1..=counts[&order[index]].1).map(|occurrence| {
            node(FootnoteBacklink {
                target: format!("footnote-ref-{number}-{occurrence}").into(),
            })
            .child(text(if counts[&order[index]].1 == 1 {
                "↩".to_owned()
            } else {
                format!("↩{occurrence}")
            }))
        });
        if body
            .last()
            .is_none_or(|node| !node.payload.is::<Paragraph>())
        {
            body.push(paragraph());
        }
        for back in returns {
            body.last_mut().unwrap().children.extend([text(" "), back]);
        }
        result.push(source_node(
            node(Footnote {
                number: number as u32,
            })
            .children(body),
            definition.offset,
            11,
        ));
    }
    Ok(result)
}

fn extract(items: &mut Vec<Item>, definitions: &mut HashMap<String, Item>) {
    for mut item in std::mem::take(items) {
        if let Some(Tag::FootnoteDefinition(label)) = &item.tag {
            definitions.entry(label.to_string()).or_insert(item);
        } else {
            extract(&mut item.children, definitions);
            items.push(item);
        }
    }
}

fn annotate(
    items: &mut [Item],
    definitions: &HashMap<String, Item>,
    order: &mut Vec<String>,
    counts: &mut HashMap<String, (usize, usize)>,
) {
    for item in items {
        if let Some(Event::FootnoteReference(label)) = &item.event {
            if definitions.contains_key(label.as_ref()) || counts.contains_key(label.as_ref()) {
                let (number, count) = counts.entry(label.to_string()).or_insert_with(|| {
                    order.push(label.to_string());
                    (order.len(), 0)
                });
                *count += 1;
                item.reference = Some((*number, *count));
            }
        }
        annotate(&mut item.children, definitions, order, counts);
    }
}
