use gpui::{FontWeight, HighlightStyle};
use gpui_richtext::{Contribution, Document, Node, NodeId, Paragraph, Registry, Span, Text};

#[test]
fn projection_keeps_unicode_leaf_positions_and_inherited_styles() {
    struct Token(String);
    let mut registry = Registry::with_standard_nodes();
    registry.register::<Token>(|token| {
        Ok(Contribution::Text(
            token.0.clone().into(),
            HighlightStyle::default(),
        ))
    });
    let document = Document::new(
        [
            Node::new(NodeId(1), Paragraph, vec![NodeId(2), NodeId(4)]),
            Node::new(
                NodeId(2),
                Span(HighlightStyle::from(FontWeight::BOLD)),
                vec![NodeId(3)],
            ),
            Node::new(NodeId(3), Text("naïve e".into()), vec![]),
            Node::new(NodeId(4), Token("\u{301} 🪁".into()), vec![]),
            Node::new(NodeId(5), Paragraph, vec![]),
            Node::new(NodeId(6), Paragraph, vec![NodeId(7)]),
            Node::new(NodeId(7), Text("סיום".into()), vec![]),
        ],
        vec![NodeId(1), NodeId(5), NodeId(6)],
    )
    .unwrap();
    let projection = registry.compile(&document).unwrap();
    assert_eq!(projection.text().as_ref(), "naïve e\u{301} 🪁\n\n\n\nסיום");
    assert_eq!(projection.offset(NodeId(3), 3), None);
    assert_eq!(projection.offset(NodeId(4), 1), None);
    let join = projection.offset(NodeId(4), 0).unwrap();
    assert_eq!(join, projection.offset(NodeId(3), "naïve e".len()).unwrap());
    assert_eq!(
        &projection.text()[join..join + "\u{301} 🪁".len()],
        "\u{301} 🪁"
    );
    assert_eq!(
        projection.spans()[0].style.font_weight,
        Some(FontWeight::BOLD)
    );
    assert_eq!(projection.spans()[1].style.font_weight, None);
    assert_eq!(projection.offset(NodeId(7), 100), None);
}

#[test]
fn invalid_ownership_is_rejected_before_resolution() {
    assert!(
        Document::new(
            [
                Node::new(NodeId(1), Paragraph, vec![NodeId(2), NodeId(2)]),
                Node::new(NodeId(2), Text("duplicate ownership".into()), vec![]),
            ],
            vec![NodeId(1)]
        )
        .is_err()
    );
    assert!(
        Document::new(
            [
                Node::new(NodeId(1), Paragraph, vec![NodeId(2)]),
                Node::new(NodeId(2), Span(HighlightStyle::default()), vec![NodeId(1)]),
            ],
            vec![NodeId(1)]
        )
        .is_err()
    );
    assert!(
        Document::new(
            [Node::new(NodeId(1), Paragraph, vec![NodeId(2)])],
            vec![NodeId(1)]
        )
        .is_err()
    );
}

#[test]
fn replacing_resolution_does_not_change_existing_projection() {
    struct Badge;
    let document = Document::new(
        [
            Node::new(NodeId(1), Paragraph, vec![NodeId(2)]),
            Node::new(NodeId(2), Badge, vec![]),
        ],
        vec![NodeId(1)],
    )
    .unwrap();
    let mut registry = Registry::with_standard_nodes();
    assert!(registry.compile(&document).is_err());
    registry.register::<Badge>(|_| {
        Ok(Contribution::Text(
            "queued".into(),
            HighlightStyle::default(),
        ))
    });
    let first = registry.compile(&document).unwrap();
    registry.register::<Badge>(|_| {
        Ok(Contribution::Text(
            "ready".into(),
            HighlightStyle::default(),
        ))
    });
    let second = registry.compile(&document).unwrap();
    assert_eq!(first.text().as_ref(), "queued");
    assert_eq!(second.text().as_ref(), "ready");
    registry.register::<Badge>(|_| Ok(Contribution::Paragraph(HighlightStyle::default())));
    assert!(registry.compile(&document).is_err());
}
