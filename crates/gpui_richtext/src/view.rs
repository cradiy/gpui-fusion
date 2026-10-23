use crate::{BlockKind, CodeBlockLabels, CodeBlockStyle, LinkSpan, Projection};
use gpui::prelude::*;
use gpui::{
    AnimationExt, ElementId, Hsla, InteractiveText, IntoElement, Refineable, RenderOnce,
    StyleRefinement, Styled, div,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct CopyFeedback {
    copied: bool,
    generation: usize,
    reset: Option<gpui::Task<()>>,
}

impl CopyFeedback {
    fn copied(&mut self, cx: &mut gpui::Context<Self>) {
        self.copied = true;
        self.generation = self.generation.wrapping_add(1);
        self.reset = Some(cx.spawn(async move |state, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = state.update(cx, |state, cx| {
                state.copied = false;
                cx.notify();
            });
        }));
        cx.notify();
    }
}

type LinkListener = Box<dyn Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App)>;
type LinkTooltipBuilder =
    Box<dyn Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App) -> Option<gpui::AnyView>>;

/// A uniform-size selectable text flow over a compiled node document.
///
/// Use a stable ID and reuse the projection between frames. Surface and inherited
/// typography use `Styled`. Selection uses GPUI's read-only system integration.
#[derive(IntoElement)]
pub struct RichText {
    id: ElementId,
    projection: Arc<Projection>,
    style: StyleRefinement,
    selection: Hsla,
    selectable: bool,
    on_link_click: Option<LinkListener>,
    link_tooltips: bool,
    link_tooltip: Option<LinkTooltipBuilder>,
    code_block_style: CodeBlockStyle,
    code_block_labels: Option<CodeBlockLabels>,
    quote_border_color: Hsla,
    task_marker_style: crate::TaskMarkerStyle,
}

impl RichText {
    pub fn new(id: impl Into<ElementId>, projection: Arc<Projection>) -> Self {
        Self {
            id: id.into(),
            projection,
            style: StyleRefinement::default(),
            selection: gpui::rgba(0x4488ff55).into(),
            selectable: true,
            on_link_click: None,
            link_tooltips: true,
            link_tooltip: None,
            code_block_style: CodeBlockStyle::default(),
            code_block_labels: None,
            quote_border_color: gpui::rgba(0x64748b66).into(),
            task_marker_style: crate::TaskMarkerStyle::default(),
        }
    }

    pub fn selection_color(mut self, color: impl Into<Hsla>) -> Self {
        self.selection = color.into();
        self
    }

    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    pub fn code_block_style(mut self, style: CodeBlockStyle) -> Self {
        self.code_block_style = style;
        self
    }

    pub fn quote_border_color(mut self, color: impl Into<Hsla>) -> Self {
        self.quote_border_color = color.into();
        self
    }

    pub fn task_marker_style(mut self, style: crate::TaskMarkerStyle) -> Self {
        self.task_marker_style = style;
        self
    }

    /// Supply localized tooltips and the copy button's accessible action name.
    pub fn code_block_labels(mut self, labels: CodeBlockLabels) -> Self {
        self.code_block_labels = Some(labels);
        self
    }

    /// Enable destination tooltips, including on links without activation handlers.
    pub fn link_tooltips(mut self, enabled: bool) -> Self {
        self.link_tooltips = enabled;
        self
    }

    /// Replace the destination tooltip with application content or suppress it
    /// for individual links by returning `None`.
    pub fn link_tooltip(
        mut self,
        builder: impl Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App) -> Option<gpui::AnyView>
        + 'static,
    ) -> Self {
        self.link_tooltip = Some(Box::new(builder));
        self
    }

    /// Handle activation without automatically navigating or opening a URL.
    /// Dragging a selection does not activate the link.
    pub fn on_link_click(
        mut self,
        listener: impl Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_link_click = Some(Box::new(listener));
        self
    }
}

impl Styled for RichText {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RichText {
    fn render(self, _: &mut gpui::Window, _: &mut gpui::App) -> impl IntoElement {
        let mut surface = div().whitespace_normal();
        surface.style().refine(&self.style);
        let header_height = gpui::px(36.);
        let header_spacing = header_height + gpui::px(8.);
        let quote_color = self.quote_border_color;
        let task_style = self.task_marker_style;
        let tasks: Vec<_> = self
            .projection
            .blocks()
            .iter()
            .filter_map(|block| {
                if let BlockKind::ListItem {
                    marker: crate::ListMarker::Task { checked },
                } = block.kind
                {
                    Some((block.range.start, checked))
                } else {
                    None
                }
            })
            .collect();
        let quote_ranges: Vec<_> = self
            .projection
            .blocks()
            .iter()
            .filter(|block| matches!(block.kind, BlockKind::Quote))
            .map(|block| block.range.clone())
            .collect();
        let code_ranges: Vec<_> = self
            .projection
            .blocks()
            .iter()
            .filter(|block| matches!(block.kind, BlockKind::Code { .. }))
            .map(|block| block.range.clone())
            .collect();
        let styled_text = self.projection.styled_text().with_line_top_spacing(
            code_ranges
                .iter()
                .map(|range| (range.start, header_spacing))
                .collect(),
        );
        let layout = styled_text.layout().clone();
        let header_layout = layout.clone();
        let header_projection = self.projection.clone();
        let id = self.id.clone();
        let mut text = InteractiveText::new(self.id, styled_text)
            .when(self.selectable, |text| text.selectable(self.selection));
        if self.link_tooltips && !self.projection.links().is_empty() {
            let projection = self.projection.clone();
            let builder = self.link_tooltip;
            text = text.tooltip(move |index, window, cx| {
                let link = projection
                    .links()
                    .iter()
                    .find(|link| link.range.contains(&index))?;
                if let Some(builder) = &builder {
                    builder(link, window, cx)
                } else {
                    Some(cx.new(|_| LinkTooltip(link.target.clone())).into())
                }
            });
        }
        if let Some(listener) = self.on_link_click {
            let projection = self.projection;
            text = text.on_click(
                projection
                    .links()
                    .iter()
                    .map(|link| link.range.clone())
                    .collect(),
                move |index, window, cx| listener(&projection.links()[index], window, cx),
            );
        }
        if code_ranges.is_empty() && quote_ranges.is_empty() && tasks.is_empty() {
            return surface.child(text);
        }
        let style = self.code_block_style;
        let labels = self.code_block_labels;
        let task_layout = layout.clone();
        let padding = if quote_ranges.is_empty() {
            style.padding.max(gpui::px(0.))
        } else {
            style.padding.max(gpui::px(12.))
        };
        surface.child(
            div()
                .id(id)
                .relative()
                .px(padding)
                .py(padding)
                .child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let vertical_padding = padding.min(layout.line_height() / 4.);
                            for range in code_ranges {
                                let Some(start) = layout.position_for_index(range.start) else {
                                    continue;
                                };
                                let Some(end) = layout.position_for_index(range.end) else {
                                    continue;
                                };
                                let block_bounds = gpui::Bounds::new(
                                    gpui::point(
                                        bounds.left(),
                                        start.y - vertical_padding - header_spacing,
                                    ),
                                    gpui::size(
                                        bounds.size.width,
                                        end.y - start.y
                                            + layout.line_height()
                                            + header_spacing
                                            + vertical_padding * 2.,
                                    ),
                                );
                                window.paint_quad(gpui::quad(
                                    block_bounds,
                                    style.corner_radius.max(gpui::px(0.)),
                                    style.background,
                                    gpui::px(1.),
                                    style.border_color,
                                    gpui::BorderStyle::default(),
                                ));
                            }
                            for range in quote_ranges {
                                let Some(start) = layout.position_for_index(range.start) else {
                                    continue;
                                };
                                let Some(end) = layout.position_for_index(range.end) else {
                                    continue;
                                };
                                window.paint_quad(gpui::fill(
                                    gpui::Bounds::new(
                                        gpui::point(bounds.left() + gpui::px(2.), start.y),
                                        gpui::size(
                                            gpui::px(3.),
                                            end.y - start.y + layout.line_height(),
                                        ),
                                    ),
                                    quote_color,
                                ));
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
                .child(text)
                .child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |_, _, window, _| {
                            let side = gpui::px(16.).min(task_layout.line_height());
                            for (offset, checked) in tasks {
                                let Some(start) = task_layout.position_for_index(offset) else {
                                    continue;
                                };
                                let origin = gpui::point(
                                    start.x,
                                    start.y + (task_layout.line_height() - side) / 2.,
                                );
                                let border = if checked {
                                    task_style.checked_background
                                } else {
                                    task_style.border
                                };
                                let background = if checked {
                                    task_style.checked_background
                                } else {
                                    gpui::transparent_black()
                                };
                                window.paint_quad(gpui::quad(
                                    gpui::Bounds::new(origin, gpui::size(side, side)),
                                    gpui::px(4.),
                                    background,
                                    gpui::px(1.),
                                    border,
                                    gpui::BorderStyle::default(),
                                ));
                                if checked {
                                    let mut path = gpui::PathBuilder::stroke(gpui::px(1.7));
                                    path.move_to(origin + gpui::point(side * 0.24, side * 0.5));
                                    path.line_to(origin + gpui::point(side * 0.43, side * 0.69));
                                    path.line_to(origin + gpui::point(side * 0.77, side * 0.32));
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, task_style.check);
                                    }
                                }
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
                .child(
                    gpui::canvas(
                        move |bounds, window, cx| {
                            let mut headers = Vec::new();
                            for block in header_projection.blocks() {
                                let BlockKind::Code { language } = &block.kind else {
                                    continue;
                                };
                                let Some(start) =
                                    header_layout.position_for_index(block.range.start)
                                else {
                                    continue;
                                };
                                let source =
                                    header_projection.text()[block.range.clone()].to_owned();
                                let feedback = window.use_keyed_state(
                                    ("code-copy-feedback", block.node.0 as usize),
                                    cx,
                                    |_, _| CopyFeedback::default(),
                                );
                                let copied = feedback.read(cx).copied;
                                let generation = feedback.read(cx).generation;
                                let icon = if copied {
                                    gpui::svg()
                                        .path(gpui_icons::GpuiIcons::LucideCheck)
                                        .text_color(style.copy_success_color)
                                        .size(gpui::px(16.))
                                        .with_animation(
                                            ("copy-success", generation),
                                            gpui::Animation::new(Duration::from_secs(2)),
                                            |icon, progress| {
                                                let enter = (progress / 0.08).min(1.);
                                                let opacity =
                                                    enter.min(((1. - progress) / 0.1).min(1.));
                                                icon.opacity(opacity).with_transformation(
                                                    gpui::Transformation::scale(gpui::size(
                                                        0.75 + 0.25 * enter,
                                                        0.75 + 0.25 * enter,
                                                    )),
                                                )
                                            },
                                        )
                                        .into_any_element()
                                } else {
                                    gpui::svg()
                                        .path(gpui_icons::GpuiIcons::LucideCopy)
                                        .text_color(style.header_color)
                                        .size(gpui::px(16.))
                                        .into_any_element()
                                };
                                let mut header = div()
                                    .w(bounds.size.width)
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .h(header_height)
                                    .px(padding)
                                    .gap_2()
                                    .border_b_1()
                                    .border_color(style.border_color)
                                    .text_color(style.header_color)
                                    .text_size(gpui::px(12.))
                                    .line_height(gpui::px(20.))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child(language.clone().unwrap_or_default()),
                                    )
                                    .child(
                                        div()
                                            .id(("copy-code", block.range.start))
                                            .flex_shrink_0()
                                            .size(gpui::px(28.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_md()
                                            .cursor_pointer()
                                            .when_some(labels.clone(), |button, labels| {
                                                button.aria_label(labels.copy.clone()).tooltip(
                                                    move |_, cx| {
                                                        let label = if copied {
                                                            labels.copied.clone()
                                                        } else {
                                                            labels.copy.clone()
                                                        };
                                                        cx.new(|_| LinkTooltip(label)).into()
                                                    },
                                                )
                                            })
                                            .hover(move |button| {
                                                button.bg(style.button_hover_background)
                                            })
                                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                cx.write_to_clipboard(
                                                    gpui::ClipboardItem::new_string(source.clone()),
                                                );
                                                feedback.update(cx, |state, cx| state.copied(cx));
                                            })
                                            .child(icon),
                                    )
                                    .into_any_element();
                                header.layout_as_root(
                                    gpui::size(
                                        gpui::AvailableSpace::Definite(bounds.size.width),
                                        gpui::AvailableSpace::Definite(header_height),
                                    ),
                                    window,
                                    cx,
                                );
                                header.prepaint_at(
                                    gpui::point(bounds.left(), start.y - header_spacing),
                                    window,
                                    cx,
                                );
                                headers.push(header);
                            }
                            headers
                        },
                        |_, mut headers, window, cx| {
                            for header in &mut headers {
                                header.paint(window, cx);
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
        )
    }
}

struct LinkTooltip(gpui::SharedString);

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn copy_feedback_restarts_timeout_on_repeated_copy(cx: &mut gpui::TestAppContext) {
        let feedback = cx.new(|_| CopyFeedback::default());
        feedback.update(cx, |state, cx| state.copied(cx));
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(1));
        feedback.update(cx, |state, cx| {
            assert!(state.copied);
            state.copied(cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1500));
        cx.run_until_parked();
        feedback.update(cx, |state, _| assert!(state.copied));
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        feedback.update(cx, |state, _| assert!(!state.copied));
    }
}

impl gpui::Render for LinkTooltip {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let width = (window.viewport_size().width - gpui::px(24.))
            .max(gpui::px(1.))
            .min(gpui::px(480.));
        div()
            .max_w(width)
            .px_3()
            .py_2()
            .rounded_md()
            .bg(gpui::rgb(0x20252d))
            .text_color(gpui::rgb(0xf5f6f8))
            .text_size(gpui::px(13.))
            .line_height(gpui::px(19.))
            .whitespace_normal()
            .child(self.0.clone())
    }
}
