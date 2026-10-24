#[path = "support/graphics_preview.rs"]
mod preview;

use gpui_richtext::{Document, HeadingLevel, MathBlock, MathInline, compose::*};

fn formula(source: &str) -> gpui_richtext::NodeTree {
    node(MathInline(source.to_owned().into()))
}

fn main() {
    let document = Document::from_roots([
        heading(HeadingLevel::H1).child(text("Mathematical expressions")),
        paragraph()
            .child(text("The variance is "))
            .child(formula(r"\sigma^2 = E[X^2] - E[X]^2"))
            .child(text(", and the standard deviation is "))
            .child(formula(r"\sigma"))
            .child(text(". For a normalized vector, "))
            .child(formula(r"\sum_j w_j^2 = 1"))
            .child(text(" holds throughout the calculation.")),
        quote()
            .child(text("The ratio "))
            .child(formula(r"\frac{a+b}{c+d}"))
            .child(text(
                " remains inline and follows the surrounding paragraph.",
            )),
        table()
            .child(
                table_header()
                    .child(table_cell().child(text("Quantity")))
                    .child(table_cell().child(text("Expression"))),
            )
            .child(
                table_row()
                    .child(table_cell().child(text("Magnitude")))
                    .child(table_cell().child(formula(r"\sqrt{h^2+k^2}"))),
            )
            .child(
                table_row()
                    .child(table_cell().child(text("Phase")))
                    .child(table_cell().child(formula(r"\theta + \phi"))),
            ),
        node(MathBlock(
            r"\int_{-\infty}^{\infty} e^{-t^2}\,dt = \sqrt{\pi}".into(),
        )),
        node(MathBlock(
            r"\begin{pmatrix} p & q \\ r & s \end{pmatrix}^{-1}
= \frac{1}{ps-qr}\begin{pmatrix} s & -q \\ -r & p \end{pmatrix}"
                .into(),
        )),
        node(MathBlock(
            r"\sum_{k=0}^{m} \binom{m}{k} z^k = (1+z)^m".into(),
        )),
    ])
    .unwrap();
    preview::run(document);
}
