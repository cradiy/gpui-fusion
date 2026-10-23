use gpui::{
    Context, FontWeight, Render, TestAppContext, TextAlign, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};
use gpui_richtext::{
    BlockKind, Document, LinkSpan, Projection, Registry, RichText, Table, TableColumn, compose::*,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[test]
fn table_compilation_preserves_cell_ranges_and_inherited_header_styles() {
    let document = Document::from_roots([
        paragraph().child(text("Inventory")),
        table()
            .child(
                table_header()
                    .child(table_cell().child(text("名称")))
                    .child(table_cell().child(text("Amount"))),
            )
            .child(
                table_row()
                    .child(table_cell().child(link_with_text("item:ceramic", "陶器")))
                    .child(table_cell()),
            )
            .child(
                table_row()
                    .child(table_cell().child(text("Spare")))
                    .child(table_cell().child(text("2"))),
            ),
        paragraph().child(text("End")),
    ])
    .unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    let BlockKind::Table(table) = &projection.blocks()[1].kind else {
        panic!("missing table metadata")
    };
    assert_eq!(
        projection.text().as_ref(),
        "Inventory\n\n名称\tAmount\n陶器\t\nSpare\t2\n\nEnd"
    );
    let header = &table.rows[0].cells[0];
    let header_span = projection
        .spans()
        .iter()
        .find(|span| span.range == header.range)
        .unwrap();
    assert_eq!(header_span.style.font_weight, Some(FontWeight::SEMIBOLD));
    let link = &projection.links()[0];
    assert_eq!(link.range, table.rows[1].cells[0].range);
    assert_eq!(
        projection.offset(link.node, "陶".len()),
        Some(link.range.start + "陶".len())
    );
    assert_eq!(projection.offset(link.node, 1), None);
    let empty = &table.rows[1].cells[1];
    assert_eq!(projection.offset(empty.node, 0), Some(empty.range.start));
}

#[test]
fn invalid_table_structure_and_column_dimensions_are_rejected() {
    for root in [
        table(),
        table().child(table_row()),
        table().child(paragraph().child(text("invalid"))),
        table().child(table_row().child(text("invalid"))),
        table()
            .child(table_row().child(table_cell()))
            .child(table_row().child(table_cell()).child(table_cell())),
        table().child(table_row().child(table_cell().child(paragraph()))),
        paragraph().child(table_row().child(table_cell())),
        table_cell().child(text("orphan")),
        node(Table::new().columns([TableColumn::flex(f32::NAN)]))
            .child(table_row().child(table_cell())),
        node(Table::new().columns([TableColumn::fixed(px(-4.))]))
            .child(table_row().child(table_cell())),
        node(Table::new().columns([TableColumn::flex(1.), TableColumn::flex(2.)]))
            .child(table_row().child(table_cell())),
    ] {
        assert!(
            Registry::with_standard_nodes()
                .compile(&Document::from_roots([root]).unwrap())
                .is_err()
        );
    }
}

struct TablePage {
    projection: Arc<Projection>,
    links: Rc<RefCell<Vec<LinkSpan>>>,
}

impl Render for TablePage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let links = self.links.clone();
        div().size_full().child(
            RichText::new("table", self.projection.clone())
                .w_full()
                .text_size(px(16.))
                .line_height(px(24.))
                .on_link_click(move |link, _, _| links.borrow_mut().push(link.clone())),
        )
    }
}

#[gpui::test]
fn cells_select_independently_and_keep_row_height_and_link_offsets_after_resize(
    cx: &mut TestAppContext,
) {
    let document = Document::from_roots([node(Table::new().columns([
        TableColumn::fixed(px(100.)),
        TableColumn::flex(1.).align(TextAlign::Right),
    ]))
    .child(
        table_header()
            .child(table_cell().child(text("Key")))
            .child(table_cell().child(text("Value"))),
    )
    .child(
        table_row()
            .child(table_cell().child(text("a\nb\nc")))
            .child(table_cell().child(link_with_text("cell:echo", "écho"))),
    )
    .child(
        table_row()
            .child(table_cell().child(text("omega")))
            .child(table_cell().child(text("tail"))),
    )])
    .unwrap();
    let projection = Arc::new(Registry::with_standard_nodes().compile(&document).unwrap());
    let expected_link = projection.links()[0].clone();
    let links = Rc::new(RefCell::new(Vec::new()));
    let events = links.clone();
    let window = cx.open_window(size(px(400.), px(400.)), move |_, _| TablePage {
        projection,
        links: events,
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    for width in [400., 260.] {
        visual.simulate_resize(size(px(width), px(400.)));
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        for (position, expected) in [
            (point(px(20.), px(60.)), "a\nb\nc"),
            (point(px(width - 30.), px(60.)), "écho"),
            (point(px(20.), px(150.)), "omega"),
        ] {
            visual.simulate_click(position, Default::default());
            visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
            visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
            assert_eq!(
                visual
                    .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
                    .as_deref(),
                Some(expected)
            );
        }
    }
    visual.simulate_resize(size(px(180.), px(400.)));
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: point(px(90.), px(60.)),
        delta: gpui::ScrollDelta::Pixels(point(px(-300.), px(0.))),
        ..Default::default()
    });
    visual.update(|window, cx| {
        window.draw(cx).clear();
    });
    visual.simulate_click(point(px(150.), px(60.)), Default::default());
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-a"));
    visual.simulate_keystrokes(&gpui::platform_shortcut("ctrl-c"));
    assert_eq!(
        visual
            .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
            .as_deref(),
        Some("écho")
    );
    assert_eq!(links.borrow().len(), 3);
    for link in links.borrow().iter() {
        assert_eq!(link.node, expected_link.node);
        assert_eq!(link.range, expected_link.range);
        assert_eq!(link.target, expected_link.target);
    }
}
