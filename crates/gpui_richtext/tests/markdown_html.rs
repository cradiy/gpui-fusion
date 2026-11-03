#![cfg(feature = "markdown")]
use gpui_richtext::{BlockKind, Registry, markdown};

#[test]
fn comments_are_hidden_without_consuming_code_or_disclosures() {
    let source = "First<!-- private -->last.\n\n<!--\n<details>\n<summary>Hidden</summary>\nsecret\n</details>\n-->\n\nVisible.\n\n`<!-- literal -->`\n\n```html\n<img src=literal.png>\n```\n";
    let registry = Registry::with_standard_nodes();
    let projection = registry.compile(&markdown::parse(source).unwrap()).unwrap();
    assert!(projection.text().starts_with("Firstlast."));
    assert!(!projection.text().contains("private"));
    assert!(!projection.text().contains("secret"));
    assert!(
        !projection
            .blocks()
            .iter()
            .any(|b| matches!(b.kind, BlockKind::Details { .. } | BlockKind::Image(_)))
    );
    assert!(projection.text().contains("<!-- literal -->"));
    assert!(projection.text().contains("<img src=literal.png>"));
    assert!(
        registry
            .compile(&markdown::parse("<!-- hidden").unwrap())
            .unwrap()
            .text()
            .is_empty()
    );
}

#[test]
fn html_images_decode_attributes_and_preserve_markdown_links_and_headings() {
    let source = "# Gallery <IMG SRC='art.png?a=1&amp;b=2' ALT='Rain &#x2602;' TITLE='A > B'>\n\n[<img src=badge.svg alt=Status>](app:status)\n\n<img\n src=portrait.png\n alt=Portrait />\n\nTail.\n";
    let registry = Registry::with_standard_nodes();
    let projection = registry.compile(&markdown::parse(source).unwrap()).unwrap();
    let images: Vec<_> = projection
        .blocks()
        .iter()
        .filter_map(|b| match &b.kind {
            BlockKind::Image(image) => Some(image),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 3);
    assert_eq!(images[0].source.as_ref(), "art.png?a=1&b=2");
    assert_eq!(images[0].alt.as_ref(), "Rain ☂");
    assert_eq!(images[0].title.as_deref(), Some("A > B"));
    assert_eq!(images[1].link.as_deref(), Some("app:status"));
    assert_eq!(images[2].source.as_ref(), "portrait.png");
    assert!(projection.text().ends_with("Tail."));
    let mut stream = markdown::MarkdownStream::new(Registry::with_standard_nodes());
    for character in source.chars() {
        stream.push_str(&character.to_string()).unwrap();
        stream.flush().unwrap();
    }
    let final_view = stream.finish().unwrap();
    assert_eq!(final_view.text(), projection.text());
    assert_eq!(
        final_view
            .blocks()
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Image(_)))
            .count(),
        3
    );
}

#[test]
fn image_attributes_do_not_turn_into_markup_and_missing_sources_stay_literal() {
    let source = "Text <img src='icon.png' alt='<!-- label -->' onerror='ignored()'> end.\n\n<img alt='No source'>\n\n<!-- removed -->\n\n<div>\n<img src='long.png' alt='first\n<!-- attribute -->\nlast'>\n</div>\n";
    let projection = Registry::with_standard_nodes()
        .compile(&markdown::parse(source).unwrap())
        .unwrap();
    let images: Vec<_> = projection
        .blocks()
        .iter()
        .filter_map(|b| match &b.kind {
            BlockKind::Image(image) => Some(image),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].alt.as_ref(), "<!-- label -->");
    assert_eq!(images[1].alt.as_ref(), "first\n<!-- attribute -->\nlast");
    assert!(projection.text().contains("<img alt='No source'>"));
    assert!(!projection.text().contains("ignored()"));
}
