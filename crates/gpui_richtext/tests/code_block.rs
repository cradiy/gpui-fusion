use gpui::{HighlightStyle, rgba};
use gpui_richtext::{BlockKind, CodeBlock, Document, Registry, compose::*};

#[test]
fn code_highlighting_keeps_source_offsets_indentation_and_block_metadata() {
    let source = "let 名称 = 7;\n\n\t名称 + 1\n";
    let document = Document::from_roots([
        paragraph().child(text("Source")),
        node(CodeBlock::new(source).language("rust").highlights([
            (0..3, HighlightStyle::color(rgba(0x8844ccff).into())),
            (4..10, HighlightStyle::color(rgba(0x226688ff).into())),
        ])),
        paragraph().child(text("Result")),
    ])
    .unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    let block = &projection.blocks()[1];
    assert_eq!(&projection.text()[block.range.clone()], source);
    assert!(
        matches!(&block.kind, BlockKind::Code { language: Some(language) } if language.as_ref() == "rust")
    );
    assert_eq!(
        projection.offset(block.node, source.len()),
        Some(block.range.end)
    );
    assert_eq!(projection.offset(block.node, 5), None);
    let spans: Vec<_> = projection
        .spans()
        .iter()
        .filter(|span| span.node == block.node)
        .collect();
    assert!(spans.len() > 2);
    assert!(
        spans
            .windows(2)
            .all(|pair| pair[0].range.end == pair[1].range.start)
    );
    assert_eq!(spans.first().unwrap().range.start, block.range.start);
    assert_eq!(spans.last().unwrap().range.end, block.range.end);
    assert_eq!(
        projection.text().as_ref(),
        format!("Source\n\n{source}\n\nResult")
    );
}

#[test]
fn invalid_code_ranges_and_inline_placement_are_rejected() {
    let registry = Registry::with_standard_nodes();
    for ranges in [vec![0..4, 3..6], vec![1..2], vec![0..100]] {
        let block = CodeBlock::new("λ = 2").highlights(
            ranges
                .into_iter()
                .map(|range| (range, HighlightStyle::default())),
        );
        assert!(
            registry
                .compile(&Document::from_roots([node(block)]).unwrap())
                .is_err()
        );
    }
    for root in [
        paragraph().child(code_block("source")),
        code_block("source").child(text("child")),
    ] {
        assert!(
            registry
                .compile(&Document::from_roots([root]).unwrap())
                .is_err()
        );
    }
}
