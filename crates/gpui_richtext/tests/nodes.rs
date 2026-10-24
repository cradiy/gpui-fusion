use gpui_richtext::{Document, Registry, compose::*};

#[test]
fn link_labels_default_to_destination_without_replacing_activation_targets() {
    let document = Document::from_roots([paragraph()
        .child(link("app:catalog"))
        .child(text(" / "))
        .child(link_with_text("app:inventory", "Inventory"))])
    .unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    assert_eq!(projection.text().as_ref(), "app:catalog / Inventory");
    let links = projection.links();
    assert_eq!(links.len(), 2);
    assert_eq!(
        &projection.text()[links[0].range.clone()],
        links[0].target.as_ref()
    );
    assert_eq!(&projection.text()[links[1].range.clone()], "Inventory");
    assert_eq!(links[1].target.as_ref(), "app:inventory");
    for invalid in [
        link_with_text("   ", "Label"),
        link_with_text("app:entry", "Label").child(text("ambiguous")),
    ] {
        let document = Document::from_roots([paragraph().child(invalid)]).unwrap();
        assert!(Registry::with_standard_nodes().compile(&document).is_err());
    }
}

#[test]
fn groups_preserve_paragraph_order_and_links_cover_styled_descendants() {
    let document = Document::from_roots([
        group()
            .child(group())
            .child(paragraph().child(text("Preface")))
            .child(
                group()
                    .child(
                        paragraph().child(text("Read ")).child(
                            link("app:chapter/8")
                                .child(strong().child(text("章节")))
                                .child(code().child(text("八"))),
                        ),
                    )
                    .child(paragraph()),
            ),
        paragraph().child(text("End")),
    ])
    .unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    assert_eq!(
        projection.text().as_ref(),
        "Preface\n\nRead 章节八\n\n\n\nEnd"
    );
    let links = projection.links();
    assert_eq!(links.len(), 1);
    assert_eq!(&projection.text()[links[0].range.clone()], "章节八");
    assert_eq!(links[0].target.as_ref(), "app:chapter/8");
    assert!(
        projection
            .spans()
            .iter()
            .filter(|span| links[0].range.contains(&span.range.start))
            .all(|span| span.style.underline.is_some())
    );
}

#[test]
fn nested_links_and_block_inline_role_violations_are_rejected() {
    let registry = Registry::with_standard_nodes();
    for root in [
        paragraph().child(link("a").child(strong().child(link("b").child(text("label"))))),
        paragraph().child(group().child(paragraph().child(text("block inside inline")))),
        group().child(text("inline inside block group")),
        paragraph().child(link("").child(text("missing target"))),
    ] {
        assert!(
            registry
                .compile(&Document::from_roots([root]).unwrap())
                .is_err()
        );
    }
}
