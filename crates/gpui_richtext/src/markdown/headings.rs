use super::*;
use std::collections::{HashMap, HashSet};

pub(super) fn blocks(
    item: &Item,
    level: pulldown_cmark::HeadingLevel,
    id: &str,
) -> Result<Vec<NodeTree>> {
    let level = match level {
        pulldown_cmark::HeadingLevel::H1 => HeadingLevel::H1,
        pulldown_cmark::HeadingLevel::H2 => HeadingLevel::H2,
        pulldown_cmark::HeadingLevel::H3 => HeadingLevel::H3,
        pulldown_cmark::HeadingLevel::H4 => HeadingLevel::H4,
        pulldown_cmark::HeadingLevel::H5 => HeadingLevel::H5,
        pulldown_cmark::HeadingLevel::H6 => HeadingLevel::H6,
    };
    let mut result = Vec::new();
    let mut content = vec![anchor(format!("heading:{id}"))];
    for child in &item.children {
        for part in split_images(child, None)? {
            match part {
                ImagePart::Inline(inline) => content.push(source_node(inline, child.offset, 0)),
                ImagePart::Image(image) => {
                    if !content.is_empty() {
                        result.push(source_node(
                            heading(level).children(std::mem::take(&mut content)),
                            item.offset,
                            6,
                        ));
                    }
                    result.push(source_node(node(image), child.offset, 6));
                }
            }
        }
    }
    if !content.is_empty() {
        result.push(source_node(
            heading(level).children(content),
            item.offset,
            6,
        ));
    }
    Ok(result)
}

pub(super) fn assign(items: &mut [Item]) -> Result<()> {
    let mut used = HashSet::new();
    reserve(items, &mut used)?;
    generate(items, &mut used, &mut HashMap::new());
    Ok(())
}

fn reserve(items: &[Item], used: &mut HashSet<String>) -> Result<()> {
    for item in items {
        if let Some(Tag::Heading { id: Some(id), .. }) = &item.tag {
            if id.is_empty() || !used.insert(id.to_string()) {
                bail!("empty or duplicate Markdown heading ID");
            }
        }
        reserve(&item.children, used)?;
    }
    Ok(())
}

fn generate(items: &mut [Item], used: &mut HashSet<String>, suffixes: &mut HashMap<String, usize>) {
    for item in items {
        if matches!(item.tag, Some(Tag::Heading { id: None, .. })) {
            let mut slug = String::new();
            for character in plain(&item.children).chars().flat_map(char::to_lowercase) {
                if character.is_alphanumeric() || character == '_' || character == '-' {
                    slug.push(character);
                } else if character.is_whitespace() && !slug.is_empty() && !slug.ends_with('-') {
                    slug.push('-');
                }
            }
            let mut base = slug.trim_matches('-').to_owned();
            if base.is_empty() {
                base = "section".into();
            }
            let mut candidate = base.clone();
            let suffix = suffixes.entry(base.clone()).or_default();
            while !used.insert(candidate.clone()) {
                *suffix += 1;
                candidate = format!("{base}-{suffix}");
            }
            if let Some(Tag::Heading { id, .. }) = &mut item.tag {
                *id = Some(candidate.into());
            }
        }
        generate(&mut item.children, used, suffixes);
    }
}

pub(super) fn target(fragment: &str) -> String {
    format!(
        "heading:{}",
        percent_encoding::percent_decode_str(fragment).decode_utf8_lossy()
    )
}
