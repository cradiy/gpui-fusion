#![cfg(feature = "markdown")]
use gpui_richtext::{BlockKind, ListMarker, Registry, markdown};

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
