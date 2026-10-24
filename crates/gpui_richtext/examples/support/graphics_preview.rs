use gpui::{App, Context, Render, Window, WindowOptions, div, prelude::*, px};
use gpui_richtext::{CodeBlockLabels, Document, Projection, RichText, RichTextTheme};
use std::sync::Arc;

struct Preview {
    projection: Arc<Projection>,
    theme: RichTextTheme,
    document: Document,
    dark: bool,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(self.theme.background)
            .text_color(self.theme.foreground)
            .p_6()
            .gap_4()
            .child(
                div()
                    .id("theme")
                    .cursor_pointer()
                    .child(if self.dark {
                        "Light theme"
                    } else {
                        "Dark theme"
                    })
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.dark = !view.dark;
                        view.theme = if view.dark {
                            RichTextTheme::dark()
                        } else {
                            RichTextTheme::light()
                        };
                        view.projection =
                            Arc::new(view.theme.registry().compile(&view.document).unwrap());
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("document")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        RichText::new("graphics", self.projection.clone())
                            .code_block_labels(CodeBlockLabels {
                                copy: "Copy source".into(),
                                copied: "Copied".into(),
                            })
                            .theme(&self.theme)
                            .text_size(px(18.))
                            .w_full(),
                    ),
            )
    }
}

pub fn run(document: Document) {
    gpui_platform::application()
        .with_assets(gpui_icons::IconAssets)
        .run(move |cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| {
                    let theme = RichTextTheme::dark();
                    let projection = Arc::new(theme.registry().compile(&document).unwrap());
                    Preview {
                        projection,
                        theme,
                        document: document.clone(),
                        dark: true,
                    }
                })
            })
            .unwrap();
            cx.activate(true);
        });
}
