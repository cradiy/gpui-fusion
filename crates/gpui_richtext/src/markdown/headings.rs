use super::*;
use std::collections::{HashMap, HashSet};

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
