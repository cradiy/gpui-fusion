use gpui::{FontStyle, FontWeight, HighlightStyle, rgba};
use gpui_richtext::{Document, InlineStyle, Registry, TextTheme, compose::*};

#[test]
fn composed_styles_inherit_without_leaking_into_siblings_or_changing_text() {
    let document = Document::from_roots([paragraph()
        .child(
            strong().child(
                emphasis().child(
                    code()
                        .child(text("nested"))
                        .child(
                            node(InlineStyle {
                                highlight: HighlightStyle::default(),
                                font_family: Some("alternate".into()),
                            })
                            .child(text("override")),
                        )
                        .child(text("resumed")),
                ),
            ),
        )
        .child(hard_break())
        .child(mark().child(underline().child(strikethrough().child(text("marked")))))
        .child(text("plain"))])
    .unwrap();
    let mut theme = TextTheme::default();
    theme.code.font_family = Some("code-family".into());
    theme.mark.highlight.background_color = Some(rgba(0x12345678).into());
    let projection = Registry::with_theme(theme.clone())
        .compile(&document)
        .unwrap();
    assert_eq!(
        projection.text().as_ref(),
        "nestedoverrideresumed\nmarkedplain"
    );
    let spans = projection.spans();
    for span in &spans[..3] {
        assert_eq!(span.style.font_weight, Some(FontWeight::BOLD));
        assert_eq!(span.style.font_style, Some(FontStyle::Italic));
    }
    assert_eq!(spans[0].font_family.as_deref(), Some("code-family"));
    assert_eq!(spans[1].font_family.as_deref(), Some("alternate"));
    assert_eq!(spans[2].font_family.as_deref(), Some("code-family"));
    assert_eq!(
        spans[4].style.background_color,
        theme.mark.highlight.background_color
    );
    assert!(spans[4].style.underline.is_some());
    assert!(spans[4].style.strikethrough.is_some());
    assert_eq!(spans[5].style, HighlightStyle::default());
    assert!(spans[5].font_family.is_none());
}
