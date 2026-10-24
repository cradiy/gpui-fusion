use gpui::{Context, Image, ImageFormat, SharedString, Task, Window, div, prelude::*};
use std::sync::Arc;

/// A Mermaid diagram with its original source preserved, without Markdown fences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mermaid(pub SharedString);

/// Raw Mermaid source displayed with a code-block surface, header, and copy action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidCodeBlock(pub SharedString);

/// A standalone LaTeX mathematical expression, without delimiter characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathBlock(pub SharedString);

/// An atomic LaTeX expression inside a text paragraph, without dollar delimiters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathInline(pub SharedString);

/// Formula overrides. Unspecified values inherit the surrounding text style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MathStyle {
    pub font_size: Option<gpui::Pixels>,
    pub color: Option<gpui::Hsla>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MermaidTheme {
    #[default]
    Light,
    Dark,
    Forest,
    Neutral,
}

/// Resolved appearance used by the native graphic renderers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphicRenderOptions {
    pub font_size: gpui::Pixels,
    pub color: gpui::Hsla,
    pub mermaid_theme: MermaidTheme,
}

impl Default for GraphicRenderOptions {
    fn default() -> Self {
        Self {
            font_size: gpui::px(18.),
            color: gpui::rgb(0x273444).into(),
            mermaid_theme: MermaidTheme::Light,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RenderedGraphic {
    pub image: Arc<Image>,
    pub size: Option<gpui::Size<gpui::Pixels>>,
    pub baseline: gpui::Pixels,
}

/// A source-backed block rendered by an optional native graphics engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Graphic {
    Mermaid(Mermaid),
    Math(MathBlock),
    InlineMath(MathInline),
}

impl Graphic {
    pub fn source(&self) -> &SharedString {
        match self {
            Self::Mermaid(value) => &value.0,
            Self::Math(value) => &value.0,
            Self::InlineMath(value) => &value.0,
        }
    }

    /// Render a self-contained SVG. Run this operation on a background executor.
    pub fn render_svg(&self, options: &GraphicRenderOptions) -> anyhow::Result<String> {
        Ok(self.render_data(options)?.0)
    }

    fn render_data(
        &self,
        options: &GraphicRenderOptions,
    ) -> anyhow::Result<(String, Option<gpui::Size<gpui::Pixels>>, gpui::Pixels)> {
        anyhow::ensure!(
            self.source().len() <= 256 * 1024,
            "graphic source exceeds 256 KiB"
        );
        match self {
            #[cfg(feature = "mermaid")]
            Self::Mermaid(value) => {
                let theme = match options.mermaid_theme {
                    MermaidTheme::Light => mermaid_svg::Theme::default_theme(),
                    MermaidTheme::Dark => mermaid_svg::Theme::dark(),
                    MermaidTheme::Forest => mermaid_svg::Theme::forest(),
                    MermaidTheme::Neutral => mermaid_svg::Theme::neutral(),
                };
                Ok((
                    mermaid_svg::render_with(&value.0, &theme)?,
                    None,
                    gpui::px(0.),
                ))
            }
            #[cfg(feature = "math")]
            Self::Math(_) | Self::InlineMath(_) => {
                let font_size = f32::from(options.font_size);
                anyhow::ensure!(
                    font_size.is_finite() && font_size > 0.,
                    "invalid formula font size"
                );
                let font = latex_rust::MathFont::stix_two_math()?;
                let color = options.color.to_rgb();
                let svg_options = latex_rust::SvgOptions {
                    font_size_pt: latex_rust::Dim::from_i64(12),
                    color: latex_rust::Color::rgb(
                        (color.r * 255.).round() as u8,
                        (color.g * 255.).round() as u8,
                        (color.b * 255.).round() as u8,
                    ),
                    display: !matches!(self, Self::InlineMath(_)),
                };
                let ast = latex_rust::parse(self.source()).map_err(latex_rust::Error::Parse)?;
                // Scale the complete layout, including fixed-point matrix spacing.
                let layout = latex_rust::layout(
                    &ast,
                    &font,
                    if svg_options.display {
                        latex_rust::MathStyle::Display
                    } else {
                        latex_rust::MathStyle::Text
                    },
                )?;
                let width: f32 = layout.width.to_svg_string().parse()?;
                let ascent: f32 = layout.height.to_svg_string().parse()?;
                let descent: f32 = layout.depth.to_svg_string().parse()?;
                let svg = latex_rust::render_svg(&layout, &font, &svg_options)?;
                let intrinsic = format!(
                    "width=\"{}pt\" height=\"{}pt\"",
                    (&layout.width * &svg_options.font_size_pt).to_svg_string(),
                    ((&layout.height + &layout.depth) * &svg_options.font_size_pt).to_svg_string(),
                );
                let svg = svg.replacen(
                    &intrinsic,
                    &format!(
                        "width=\"{}px\" height=\"{}px\"",
                        width * font_size,
                        (ascent + descent) * font_size
                    ),
                    1,
                );
                let svg = svg.replacen("<g fill=", &format!("<g opacity=\"{}\" fill=", color.a), 1);
                Ok((
                    svg,
                    Some(gpui::size(
                        gpui::px(width * font_size),
                        gpui::px((ascent + descent) * font_size),
                    )),
                    gpui::px(ascent * font_size),
                ))
            }
            #[allow(unreachable_patterns)]
            _ => {
                let _ = options;
                anyhow::bail!("graphic renderer feature is disabled")
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct GraphicView {
    requested: Option<(Graphic, GraphicRenderOptions)>,
    displayed: Option<RenderedGraphic>,
    task: Option<Task<()>>,
    running: bool,
    error: Option<SharedString>,
    cache: std::collections::VecDeque<((Graphic, GraphicRenderOptions), RenderedGraphic)>,
}

impl GraphicView {
    pub(crate) fn ready(&self) -> Option<RenderedGraphic> {
        self.cache
            .iter()
            .find(|(key, _)| Some(key) == self.requested.as_ref())
            .or_else(|| {
                self.cache.iter().rev().find(|(key, _)| {
                    self.requested
                        .as_ref()
                        .is_some_and(|request| key.0 == request.0)
                })
            })
            .map(|(_, image)| image.clone())
    }

    pub(crate) fn request(
        &mut self,
        graphic: Graphic,
        options: GraphicRenderOptions,
        cx: &mut Context<Self>,
    ) {
        let request = (graphic, options);
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
                .render_data(&job.1)
                .map(|(svg, size, baseline)| RenderedGraphic {
                    image: Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())),
                    size,
                    baseline,
                })
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

    #[test]
    fn formula_metrics_scale_with_svg_and_color_overrides() {
        let formula = Graphic::InlineMath(MathInline(r"\frac{j}{j+2}".into()));
        let small = GraphicRenderOptions {
            font_size: gpui::px(15.),
            color: gpui::rgba(0x39a76b80).into(),
            ..Default::default()
        };
        let large = GraphicRenderOptions {
            font_size: gpui::px(30.),
            ..small
        };
        let (small_svg, small_size, small_baseline) = formula.render_data(&small).unwrap();
        let (large_svg, large_size, large_baseline) = formula.render_data(&large).unwrap();
        assert_eq!(large_size.unwrap().width, small_size.unwrap().width * 2.);
        assert_eq!(large_size.unwrap().height, small_size.unwrap().height * 2.);
        assert_eq!(large_baseline, small_baseline * 2.);
        let renderer = gpui::SvgRenderer::new(Arc::new(()));
        let small_image = renderer
            .render_single_frame(small_svg.as_bytes(), 1.)
            .unwrap();
        let large_image = renderer
            .render_single_frame(large_svg.as_bytes(), 1.)
            .unwrap();
        assert!(large_image.size(0).width > small_image.size(0).width);
        assert!(small_svg.contains("#39a76b"));
        assert!(small_svg.contains("opacity=\"0.50"));
    }

    #[gpui::test]
    fn newest_request_wins_and_invalid_stream_retains_last_image(cx: &mut gpui::TestAppContext) {
        let view = cx.new(|_| GraphicView::default());
        view.update(cx, |view, cx| {
            view.request(
                Graphic::Math(MathBlock("u^3".into())),
                GraphicRenderOptions {
                    font_size: gpui::px(24.),
                    ..Default::default()
                },
                cx,
            );
            view.request(
                Graphic::Math(MathBlock("v_7".into())),
                GraphicRenderOptions::default(),
                cx,
            );
        });
        cx.run_until_parked();
        let displayed = view.update(cx, |view, cx| {
            assert!(!view.running);
            assert!(view.error.is_none());
            let image = view.displayed.clone().unwrap().image;
            let expected = Graphic::Math(MathBlock("v_7".into()))
                .render_svg(&GraphicRenderOptions::default())
                .unwrap();
            assert_eq!(image.bytes, expected.as_bytes());
            view.request(
                Graphic::Math(MathBlock("\\frac{".into())),
                GraphicRenderOptions::default(),
                cx,
            );
            image
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert!(view.error.is_some());
            assert!(Arc::ptr_eq(
                &view.displayed.as_ref().unwrap().image,
                &displayed
            ));
            view.request(
                Graphic::Math(MathBlock("v_7".into())),
                GraphicRenderOptions::default(),
                cx,
            );
            assert!(!view.running);
            assert!(view.error.is_none());
            assert!(Arc::ptr_eq(
                &view.displayed.as_ref().unwrap().image,
                &displayed
            ));
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
                view.child(
                    gpui::img(image.image)
                        .when_some(image.size, |view, size| view.w(size.width).h(size.height))
                        .max_w_full(),
                )
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
