#![cfg(feature = "syntax-highlighting")]
use gpui_richtext::{CodeBlock, Document, Registry, compose::node, syntax::SyntaxHighlighter};

#[test]
fn highlighting_tracks_multiline_state_and_utf8_without_changing_source() {
    let source = "/* α\nstill comment */\nlet count = 42;\n";
    let highlighter = SyntaxHighlighter::default();
    let ranges = highlighter.highlight(source, "rust").unwrap();
    assert_eq!(ranges.first().unwrap().0.start, 0);
    assert_eq!(ranges.last().unwrap().0.end, source.len());
    assert!(
        ranges
            .windows(2)
            .all(|pair| pair[0].0.end == pair[1].0.start)
    );
    let color_at = |index| {
        ranges
            .iter()
            .find(|(range, _)| range.contains(&index))
            .unwrap()
            .1
            .color
    };
    assert_eq!(
        color_at(source.find('α').unwrap()),
        color_at(source.find("still").unwrap())
    );
    assert_ne!(color_at(3), color_at(source.find("42").unwrap()));
    assert!(
        highlighter
            .highlight(source, "unlisted-language")
            .unwrap()
            .is_empty()
    );
    let document = Document::from_roots([node(CodeBlock::new(source).language("rust"))]).unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    assert_eq!(projection.text().as_ref(), source);
    assert!(
        projection
            .spans()
            .iter()
            .any(|span| span.style.color.is_some())
    );
    let uncolored = Registry::with_standard_nodes()
        .syntax_highlighter(None)
        .compile(&document)
        .unwrap();
    assert!(
        uncolored
            .spans()
            .iter()
            .all(|span| span.style.color.is_none())
    );
}
