#![cfg(feature = "markdown")]
use gpui_richtext::{BlockKind, ListMarker, Registry, markdown};

#[test]
fn linked_images_split_styled_paragraphs_without_losing_targets_or_text() {
    let source = "**Before [![地图](map.png \"Overview\") *catalog*](app:atlas) after**.\n\n[![](empty.png)](app:empty)\n";
    let projection = Registry::with_standard_nodes()
        .compile(&markdown::parse(source).unwrap())
        .unwrap();
    let images: Vec<_> = projection
        .blocks()
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Image(image) => Some((block, image)),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 2);
    let (block, image) = images[0];
    assert_eq!(image.link.as_deref(), Some("app:atlas"));
    assert_eq!(image.title.as_deref(), Some("Overview"));
    assert_eq!(&projection.text()[block.range.clone()], "地图");
    assert!(
        projection
            .links()
            .iter()
            .any(|link| link.node == images[1].0.node
                && link.range.is_empty()
                && link.target.as_ref() == "app:empty")
    );
    let catalog = projection
        .spans()
        .iter()
        .find(|span| &projection.text()[span.range.clone()] == "catalog")
        .unwrap();
    assert_eq!(catalog.style.font_weight, Some(gpui::FontWeight::BOLD));
    assert_eq!(catalog.style.font_style, Some(gpui::FontStyle::Italic));
    assert!(
        projection
            .links()
            .iter()
            .any(|link| link.range == catalog.range && link.target.as_ref() == "app:atlas")
    );
    assert!(projection.text().contains(" after"));
    let mut stream = markdown::MarkdownStream::new(Registry::with_standard_nodes());
    for character in source.chars() {
        stream.push_str(&character.to_string()).unwrap();
        stream.flush().unwrap();
    }
    assert_eq!(stream.finish().unwrap().text(), projection.text());
}

#[test]
fn streamed_footnotes_number_by_reference_and_keep_all_return_destinations() {
    let source = "Read[^beta] again[^beta], then[^alpha].\n\n[^alpha]: Alpha **内容**.\n\n[^beta]: Beta [source](https://example.org/b).\n\n    A second paragraph.\n\n[^unused]: Omitted.\n";
    let mut stream = markdown::MarkdownStream::new(Registry::with_standard_nodes());
    for character in source.chars() {
        stream.push_str(&character.to_string()).unwrap();
        stream.flush().unwrap();
    }
    let projection = stream.finish().unwrap();
    assert!(projection.text().starts_with("Read[1] again[1], then[2]."));
    assert!(projection.text().find("Beta").unwrap() < projection.text().find("Alpha").unwrap());
    assert!(!projection.text().contains("Omitted"));
    for link in projection
        .links()
        .iter()
        .filter(|link| link.kind == gpui_richtext::LinkKind::Anchor)
    {
        let anchor = projection
            .anchors()
            .iter()
            .find(|anchor| anchor.name == link.target)
            .unwrap();
        assert!(projection.text().is_char_boundary(anchor.offset));
    }
    assert_eq!(projection.anchors().len(), 5);
    assert_eq!(
        projection
            .links()
            .iter()
            .filter(|link| link.kind == gpui_richtext::LinkKind::External)
            .count(),
        1
    );
}

#[test]
fn nested_markdown_preserves_structure_targets_and_code_source() {
    let source = "# Data catalog\n\n> Overview\n>\n> 3. Primary **record**\n>    - [x] Validate [schema](app:schema)\n>\n>    ```rust\n>    let label = \"温度\";\n>    ```\n\n---\n\n![Diagram](assets/catalog.png)\n\n| Field | Count |\n| :--- | ---: |\n| Nodes | 12 |\n";
    let document = markdown::parse(source).unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    let items: Vec<_> = projection
        .blocks()
        .iter()
        .filter(|block| matches!(block.kind, BlockKind::ListItem { .. }))
        .collect();
    assert!(matches!(
        items[0].kind,
        BlockKind::ListItem {
            marker: ListMarker::Number(3)
        }
    ));
    assert!(matches!(
        items[1].kind,
        BlockKind::ListItem {
            marker: ListMarker::Task { checked: true }
        }
    ));
    assert_eq!(
        (items[0].indent, items[1].indent, items[1].quote_depth),
        (0, 1, 1)
    );
    let link = &projection.links()[0];
    assert_eq!(link.target.as_ref(), "app:schema");
    assert_eq!(&projection.text()[link.range.clone()], "schema");
    let code = projection
        .blocks()
        .iter()
        .find(|block| matches!(block.kind, BlockKind::Code { .. }))
        .unwrap();
    assert_eq!(
        &projection.text()[code.range.clone()],
        "let label = \"温度\";\n"
    );
    assert_eq!((code.indent, code.quote_depth), (1, 1));
    assert!(
        projection
            .blocks()
            .iter()
            .any(|block| matches!(block.kind, BlockKind::Rule))
    );
    assert!(projection.blocks().iter().any(|block| matches!(&block.kind, BlockKind::Image(image) if image.source.as_ref() == "assets/catalog.png" && image.alt.as_ref() == "Diagram")));
    let BlockKind::Table(table) = &projection.blocks().last().unwrap().kind else {
        panic!("table missing")
    };
    assert_eq!(table.columns[1].align, gpui::TextAlign::Right);
    assert_eq!(
        &projection.text()[table.rows[1].cells[1].range.clone()],
        "12"
    );
}

#[test]
fn image_splits_paragraph_without_losing_text_and_html_stays_literal() {
    let document =
        markdown::parse("Above ![Plot](plot.png) below.\n\n<span>literal</span>").unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    assert_eq!(
        projection.text().as_ref(),
        "Above \n\nPlot\n\n below.\n\n<span>literal</span>"
    );
}

#[test]
fn callout_children_keep_local_and_document_coordinates_during_streaming() {
    let source = "Preamble\n\n> [!WARNING]\n> Read [limits](app:limits).\n>\n> > [!TIP]\n> > Cache **温度**.\n>\n> ```text\n> stable\n> ```\n\nCodec\n: Converts bytes.\n\n: Accepts a [profile](app:profile).\n\nWindow\n: A bounded interval.\n";
    let mut stream = markdown::MarkdownStream::new(Registry::with_standard_nodes());
    for character in source.chars() {
        stream.push_str(&character.to_string()).unwrap();
        stream.flush().unwrap();
    }
    let projection = stream.finish().unwrap();
    let callout = &projection.blocks()[1];
    let BlockKind::Alert { alert, body } = &callout.kind else {
        panic!("callout missing")
    };
    assert_eq!(alert.kind, gpui_richtext::AlertKind::Warning);
    assert_eq!(
        &projection.text()[callout.range.clone()],
        body.text().as_ref()
    );
    let link = &body.links()[0];
    assert_eq!(&body.text()[link.range.clone()], "limits");
    assert_eq!(
        projection.links()[0].range.start,
        callout.range.start + link.range.start
    );
    let BlockKind::Alert { body: nested, .. } = &body.blocks()[1].kind else {
        panic!("nested callout missing")
    };
    let span = nested
        .spans()
        .iter()
        .find(|span| span.style.font_weight.is_some())
        .unwrap();
    assert_eq!(&nested.text()[span.range.clone()], "温度");
    assert_eq!(
        projection.offset(span.node, 0),
        Some(callout.range.start + body.blocks()[1].range.start + span.range.start)
    );
    let terms: Vec<_> = projection
        .blocks()
        .iter()
        .filter(|block| matches!(block.kind, BlockKind::DefinitionTerm))
        .collect();
    assert_eq!(terms.len(), 2);
    let descriptions: Vec<_> = projection
        .blocks()
        .iter()
        .filter(|block| block.indent == 1)
        .collect();
    assert_eq!(descriptions.len(), 3);
    assert_eq!(
        projection.links().last().unwrap().target.as_ref(),
        "app:profile"
    );
}
