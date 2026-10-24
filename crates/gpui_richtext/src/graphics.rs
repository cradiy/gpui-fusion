use gpui::{Context, Image, ImageFormat, SharedString, Task, Window, div, prelude::*};
use std::sync::Arc;

/// A Mermaid diagram with its original editable source preserved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mermaid(pub SharedString);

/// A standalone LaTeX mathematical expression, without delimiter characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathBlock(pub SharedString);

/// A source-backed block rendered by an optional native graphics engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Graphic {
    Mermaid(Mermaid),
    Math(MathBlock),
}

impl Graphic {
    pub fn source(&self) -> &SharedString {
        match self {
            Self::Mermaid(value) => &value.0,
            Self::Math(value) => &value.0,
        }
    }

    /// Render a self-contained SVG. Run this operation on a background executor.
    pub fn render_svg(&self, dark: bool) -> anyhow::Result<String> {
        anyhow::ensure!(
            self.source().len() <= 256 * 1024,
            "graphic source exceeds 256 KiB"
        );
        match self {
            #[cfg(feature = "mermaid")]
            Self::Mermaid(value) => {
                let theme = if dark {
                    mermaid_svg::Theme::dark()
                } else {
                    mermaid_svg::Theme::default_theme()
                };
                Ok(mermaid_svg::render_with(&value.0, &theme)?)
            }
            #[cfg(feature = "math")]
            Self::Math(value) => {
                let font = latex_rust::MathFont::stix_two_math()?;
                let options = latex_rust::SvgOptions {
                    font_size_pt: latex_rust::Dim::from_i64(18),
                    color: if dark {
                        latex_rust::Color::rgb(220, 228, 238)
                    } else {
                        latex_rust::Color::rgb(39, 52, 68)
                    },
                    display: true,
                };
                let ast = latex_rust::parse(&value.0).map_err(latex_rust::Error::Parse)?;
                // Scale the complete layout, including fixed-point matrix spacing.
                let layout = latex_rust::layout(&ast, &font, latex_rust::MathStyle::Display)?;
                Ok(latex_rust::render_svg(&layout, &font, &options)?)
            }
            #[allow(unreachable_patterns)]
            _ => {
                let _ = dark;
                anyhow::bail!("graphic renderer feature is disabled")
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct GraphicView {
    requested: Option<(Graphic, bool)>,
    displayed: Option<Arc<Image>>,
    task: Option<Task<()>>,
    running: bool,
    error: Option<SharedString>,
    cache: std::collections::VecDeque<((Graphic, bool), Arc<Image>)>,
}

impl GraphicView {
    pub(crate) fn request(&mut self, graphic: Graphic, dark: bool, cx: &mut Context<Self>) {
        let request = (graphic, dark);
        if self.requested.as_ref() == Some(&request) {
            return;
        }
        self.requested = Some(request);
        if let Some((_, image)) = self
            .cache
            .iter()
            .find(|(key, _)| Some(key) == self.requested.as_ref())
        {
            self.displayed = Some(image.clone());
            self.error = None;
            return;
        }
        if !self.running {
            self.start(cx);
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let request = self.requested.clone().unwrap();
        if let Some((_, image)) = self.cache.iter().find(|(key, _)| key == &request) {
            self.displayed = Some(image.clone());
            self.error = None;
            return;
        }
        self.running = true;
        let job = request.clone();
        let work = cx.background_executor().spawn(async move {
            job.0
                .render_svg(job.1)
                .map(|svg| Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())))
                .map_err(|error| SharedString::from(error.to_string()))
        });
        self.task = Some(cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |view, cx| {
                view.running = false;
                if let Ok(image) = &result {
                    view.cache.push_back((request.clone(), image.clone()));
                    if view.cache.len() > 4 {
                        view.cache.pop_front();
                    }
                }
                if view.requested.as_ref() != Some(&request) {
                    view.start(cx);
                } else {
                    match result {
                        Ok(image) => {
                            view.displayed = Some(image);
                            view.error = None;
                        }
                        Err(error) => view.error = Some(error),
                    }
                }
                cx.notify();
            });
        }));
    }
}

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::*;

    #[gpui::test]
    fn newest_request_wins_and_invalid_stream_retains_last_image(cx: &mut gpui::TestAppContext) {
        let view = cx.new(|_| GraphicView::default());
        view.update(cx, |view, cx| {
            view.request(Graphic::Math(MathBlock("u^3".into())), true, cx);
            view.request(Graphic::Math(MathBlock("v_7".into())), false, cx);
        });
        cx.run_until_parked();
        let displayed = view.update(cx, |view, cx| {
            assert!(!view.running);
            assert!(view.error.is_none());
            let image = view.displayed.clone().unwrap();
            let expected = Graphic::Math(MathBlock("v_7".into()))
                .render_svg(false)
                .unwrap();
            assert_eq!(image.bytes, expected.as_bytes());
            view.request(Graphic::Math(MathBlock("\\frac{".into())), false, cx);
            image
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert!(view.error.is_some());
            assert!(Arc::ptr_eq(view.displayed.as_ref().unwrap(), &displayed));
            view.request(Graphic::Math(MathBlock("v_7".into())), false, cx);
            assert!(!view.running);
            assert!(view.error.is_none());
            assert!(Arc::ptr_eq(view.displayed.as_ref().unwrap(), &displayed));
        });
    }
}

impl gpui::Render for GraphicView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w_full()
            .when_some(self.displayed.clone(), |view, image| {
                view.child(gpui::img(image).max_w_full())
            })
            .when(self.displayed.is_none(), |view| {
                view.child(
                    self.requested
                        .as_ref()
                        .map(|request| request.0.source().clone())
                        .unwrap_or_default(),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().child(error))
            })
    }
}
