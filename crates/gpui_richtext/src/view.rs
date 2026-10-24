use crate::{BlockKind, CodeBlockLabels, CodeBlockStyle, LinkSpan, Projection};
use gpui::prelude::*;
use gpui::{
    AnimationExt, ElementId, Hsla, InteractiveText, IntoElement, Refineable, RenderOnce,
    StyleRefinement, Styled, div,
};
use std::rc::Rc;
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

type LinkListener = Rc<dyn Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App)>;
type LinkTooltipBuilder =
    Rc<dyn Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App) -> Option<gpui::AnyView>>;
type ImageResolver = Rc<dyn Fn(&crate::Image) -> gpui::ImageSource>;
type AnchorPositions = std::collections::HashMap<gpui::SharedString, gpui::Point<gpui::Pixels>>;

/// Selectable rich text over a compiled node document.
///
/// Use a stable ID and reuse the projection between frames. Surface and inherited
/// typography use `Styled`. Selection uses GPUI's read-only system integration.
/// Table cells, alert bodies and surrounding text sections have independent selections.
#[derive(Clone, IntoElement)]
pub struct RichText {
    id: ElementId,
    projection: Arc<Projection>,
    style: StyleRefinement,
    selection: Hsla,
    selectable: bool,
    preserve_selection_on_update: bool,
    selection_revision: u64,
    on_link_click: Option<LinkListener>,
    link_tooltips: bool,
    link_tooltip: Option<LinkTooltipBuilder>,
    code_block_style: CodeBlockStyle,
    code_block_labels: Option<CodeBlockLabels>,
    quote_border_color: Hsla,
    task_marker_style: crate::TaskMarkerStyle,
    block_spacing: gpui::Pixels,
    table_style: crate::TableStyle,
    heading_styles: [crate::HeadingStyle; 6],
    image_source: Option<ImageResolver>,
    alert_styles: [crate::AlertStyle; 5],
    navigation: Option<Rc<std::cell::RefCell<AnchorPositions>>>,
    scroll_handle: Option<gpui::ScrollHandle>,
    footnote_style: crate::FootnoteStyle,
}

impl RichText {
    pub fn new(id: impl Into<ElementId>, projection: Arc<Projection>) -> Self {
        Self {
            id: id.into(),
            projection,
            style: StyleRefinement::default(),
            selection: gpui::rgba(0x4488ff55).into(),
            selectable: true,
            preserve_selection_on_update: false,
            selection_revision: 0,
            on_link_click: None,
            link_tooltips: true,
            link_tooltip: None,
            code_block_style: CodeBlockStyle::default(),
            code_block_labels: None,
            quote_border_color: gpui::rgba(0x64748b66).into(),
            task_marker_style: crate::TaskMarkerStyle::default(),
            block_spacing: gpui::px(10.),
            table_style: crate::TableStyle::default(),
            heading_styles: crate::HeadingStyle::defaults(),
            image_source: None,
            alert_styles: crate::RichTextTheme::light().alerts,
            navigation: None,
            scroll_handle: None,
            footnote_style: crate::FootnoteStyle::default(),
        }
    }

    /// Apply surface colors; compile with the same theme's registry for inline colors.
    /// Subsequent styling calls can override individual values.
    pub fn theme(mut self, theme: &crate::RichTextTheme) -> Self {
        self = self.footnote_style(theme.footnote);
        self.alert_styles = theme.alerts.clone();
        self.selection = theme.selection;
        self.quote_border_color = theme.quote_border;
        self.code_block_style = theme.code_block.clone();
        self.table_style = theme.table.clone();
        self.task_marker_style = theme.task_marker.clone();
        self.bg(theme.background).text_color(theme.foreground)
    }

    pub fn alert_style(mut self, kind: crate::AlertKind, style: crate::AlertStyle) -> Self {
        self.alert_styles[kind.index()] = style;
        self
    }

    /// Scroll this enclosing viewport when an internal document link is activated.
    pub fn scroll_handle(mut self, handle: &gpui::ScrollHandle) -> Self {
        self.scroll_handle = Some(handle.clone());
        self
    }

    /// Nonpositive or nonfinite metrics retain their existing values.
    pub fn footnote_style(mut self, style: crate::FootnoteStyle) -> Self {
        if style.font_scale.is_finite() && style.font_scale > 0. {
            self.footnote_style.font_scale = style.font_scale;
        }
        if style.line_height.is_finite() && style.line_height > 0. {
            self.footnote_style.line_height = style.line_height;
        }
        self
    }

    pub fn selection_color(mut self, color: impl Into<Hsla>) -> Self {
        self.selection = color.into();
        self
    }

    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// Keep selections on updates that leave their text prefix unchanged.
    pub fn preserve_selection_on_update(mut self, preserve: bool) -> Self {
        self.preserve_selection_on_update = preserve;
        self
    }

    /// Change this value when replacing a document to discard its previous selection.
    pub fn selection_revision(mut self, revision: u64) -> Self {
        self.selection_revision = revision;
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

    /// Height of automatically inserted blank separators between blocks.
    pub fn block_spacing(mut self, spacing: gpui::Pixels) -> Self {
        if f32::from(spacing).is_finite() {
            self.block_spacing = spacing.max(gpui::px(1.));
        }
        self
    }

    pub fn table_style(mut self, style: crate::TableStyle) -> Self {
        self.table_style = style;
        self
    }

    /// Metrics for H1 through H6. Nonpositive or nonfinite values retain body metrics.
    pub fn heading_styles(mut self, styles: [crate::HeadingStyle; 6]) -> Self {
        self.heading_styles = styles;
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
        self.link_tooltip = Some(Rc::new(builder));
        self
    }

    /// Handle activation without automatically navigating or opening a URL.
    /// Dragging a selection does not activate the link.
    pub fn on_link_click(
        mut self,
        listener: impl Fn(&LinkSpan, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_link_click = Some(Rc::new(listener));
        self
    }
}

impl Styled for RichText {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RichText {
    /// Resolve document image destinations to assets, file paths, URLs or decoded images.
    pub fn image_source(
        mut self,
        resolver: impl Fn(&crate::Image) -> gpui::ImageSource + 'static,
    ) -> Self {
        self.image_source = Some(Rc::new(resolver));
        self
    }
}

impl RenderOnce for RichText {
    fn render(mut self, window: &mut gpui::Window, _: &mut gpui::App) -> impl IntoElement {
        if self.navigation.is_none() {
            self.navigation = Some(Rc::default());
        }
        if !self.projection.parts.is_empty() {
            return self.render_parts(window);
        }
        let mut surface = div().relative().whitespace_normal();
        surface.style().refine(&self.style);
        let header_height = gpui::px(36.);
        let header_spacing = header_height + gpui::px(8.);
        let quote_color = self.quote_border_color;
        let task_style = self.task_marker_style;
        let padding = self.code_block_style.padding.max(gpui::px(0.));
        let mut marker_text_style = window.text_style();
        marker_text_style.refine(&self.style.text);
        let mut list_indent = gpui::px(24.);
        let bullet_inset = gpui::px(16.);
        let mut bullet_indent = bullet_inset + gpui::px(8.);
        let markers: Vec<_> = self
            .projection
            .blocks()
            .iter()
            .filter_map(|block| {
                if let BlockKind::ListItem { marker } = block.kind {
                    let mut marker_style = marker_text_style.clone();
                    if block.footnote {
                        marker_style.font_size =
                            (marker_style.font_size.to_pixels(window.rem_size())
                                * self.footnote_style.font_scale)
                                .into();
                    }
                    if let Some(span) = self
                        .projection
                        .spans()
                        .iter()
                        .find(|span| span.node == block.node)
                    {
                        marker_style = marker_style.highlight(span.style);
                        if let Some(family) = &span.font_family {
                            marker_style.font_family = family.clone();
                        }
                    }
                    let label: gpui::SharedString = marker.text().trim_end().to_owned().into();
                    let shaped = window.text_system().shape_line(
                        label.clone(),
                        marker_style.font_size.to_pixels(window.rem_size()),
                        &[marker_style.to_run(label.len())],
                        None,
                    );
                    if matches!(marker, crate::ListMarker::Bullet) {
                        bullet_indent =
                            bullet_indent.max(bullet_inset + shaped.width() + gpui::px(8.));
                    } else if !matches!(marker, crate::ListMarker::Task { .. }) {
                        list_indent = list_indent.max(shaped.width() + gpui::px(8.));
                    }
                    Some((block.range.start, marker, shaped))
                } else {
                    None
                }
            })
            .collect();
        let mut indents = Vec::new();
        for block in self.projection.blocks() {
            let (prefix_len, width) = match block.kind {
                BlockKind::ListItem {
                    marker: crate::ListMarker::Bullet,
                } => (crate::ListMarker::Bullet.text().len(), bullet_indent),
                BlockKind::ListItem { marker } => (marker.text().len(), list_indent),
                BlockKind::Quote => (0, gpui::px(16.)),
                BlockKind::Code { .. } => (0, padding),
                _ if block.indent > 0 || block.quote_depth > 0 => (0, gpui::px(0.)),
                _ => continue,
            };
            let width =
                width + gpui::px(block.indent as f32 * 24. + block.quote_depth as f32 * 16.);
            indents.push(gpui::TextLineIndent {
                line_start: block.range.start,
                prefix_len,
                width,
            });
            for (offset, _) in self.projection.text()[block.range.clone()].match_indices('\n') {
                indents.push(gpui::TextLineIndent {
                    line_start: block.range.start + offset + 1,
                    prefix_len: 0,
                    width,
                });
            }
        }
        let mut quote_ranges = Vec::new();
        let blocks = self.projection.blocks();
        let depth = |block: &crate::BlockSpan| {
            block.quote_depth + usize::from(matches!(block.kind, BlockKind::Quote))
        };
        for level in 0..blocks.iter().map(depth).max().unwrap_or(0) {
            let quote_id = |block: &crate::BlockSpan| {
                block.quote_path.get(level).copied().unwrap_or(block.node)
            };
            let mut index = 0;
            while index < blocks.len() {
                let first = &blocks[index];
                if depth(first) <= level {
                    index += 1;
                    continue;
                }
                let mut end = index;
                while end + 1 < blocks.len()
                    && depth(&blocks[end + 1]) > level
                    && blocks[end + 1].indent == first.indent
                    && quote_id(&blocks[end + 1]) == quote_id(first)
                {
                    end += 1;
                }
                quote_ranges.push((
                    first.range.start..blocks[end].range.end,
                    first.indent,
                    level,
                    matches!(first.kind, BlockKind::Code { .. }),
                    matches!(blocks[end].kind, BlockKind::Code { .. }),
                ));
                index = end + 1;
            }
        }
        let code_ranges: Vec<_> = self
            .projection
            .blocks()
            .iter()
            .filter(|block| matches!(block.kind, BlockKind::Code { .. }))
            .map(|block| block.range.clone())
            .collect();
        let styled_text = self
            .projection
            .styled_text()
            .with_line_typography({
                let body_size = marker_text_style.font_size.to_pixels(window.rem_size());
                let mut typography = Vec::new();
                for block in self.projection.blocks() {
                    let metrics = match block.kind {
                        BlockKind::Heading { level } => {
                            let style = self.heading_styles[level.index()];
                            let scale = if block.footnote {
                                self.footnote_style.font_scale
                            } else {
                                1.
                            };
                            Some((body_size * style.font_scale * scale, style.line_height))
                        }
                        _ if block.footnote => Some((
                            body_size * self.footnote_style.font_scale,
                            self.footnote_style.line_height,
                        )),
                        _ => None,
                    };
                    if let Some((font_size, line_ratio)) = metrics {
                        let line_height = font_size * line_ratio;
                        for start in std::iter::once(block.range.start).chain(
                            self.projection.text()[block.range.clone()]
                                .match_indices('\n')
                                .map(|(offset, _)| block.range.start + offset + 1),
                        ) {
                            typography.push(gpui::TextLineTypography {
                                line_start: start,
                                font_size,
                                line_height,
                            });
                        }
                    }
                }
                typography
            })
            .with_empty_line_heights(
                self.projection
                    .blocks()
                    .windows(2)
                    .filter_map(|blocks| {
                        let gap = blocks[0].range.end..blocks[1].range.start;
                        (self.projection.text()[gap.clone()] == *"\n\n")
                            .then_some((gap.start + 1, self.block_spacing))
                    })
                    .collect(),
            )
            .with_line_indents(indents)
            .with_line_top_spacing(
                code_ranges
                    .iter()
                    .map(|range| (range.start, header_spacing))
                    .collect(),
            );
        let layout = styled_text.layout().clone();
        let anchor_layout = layout.clone();
        let anchors = self.projection.anchors().to_vec();
        let navigation = self.navigation.clone().unwrap();
        let navigation_overlay = gpui::canvas(
            move |_, _, _| {
                let mut positions = navigation.borrow_mut();
                for anchor in &anchors {
                    if let Some(position) = anchor_layout.position_for_index(anchor.offset) {
                        positions.insert(anchor.name.clone(), position);
                    }
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let header_layout = layout.clone();
        let header_projection = self.projection.clone();
        let id = self.id.clone();
        let mut text = InteractiveText::new(self.id, styled_text)
            .preserve_selection_on_update(self.preserve_selection_on_update)
            .selection_revision(self.selection_revision)
            .when(self.selectable, |text| text.selectable(self.selection));
        if self.link_tooltips && !self.projection.links().is_empty() {
            let projection = self.projection.clone();
            let builder = self.link_tooltip;
            text = text.tooltip(move |index, window, cx| {
                let link = projection
                    .links()
                    .iter()
                    .find(|link| link.range.contains(&index))?;
                if link.kind == crate::LinkKind::Anchor {
                    return None;
                }
                if let Some(builder) = &builder {
                    builder(link, window, cx)
                } else {
                    Some(cx.new(|_| LinkTooltip(link.target.clone())).into())
                }
            });
        }
        if self.on_link_click.is_some() || self.scroll_handle.is_some() {
            let links: Vec<_> = self
                .projection
                .links()
                .iter()
                .filter(|link| match link.kind {
                    crate::LinkKind::External => self.on_link_click.is_some(),
                    crate::LinkKind::Anchor => self.scroll_handle.is_some(),
                })
                .cloned()
                .collect();
            let listener = self.on_link_click;
            let scroll = self.scroll_handle;
            let navigation = self.navigation.unwrap();
            text = text.on_click(
                links.iter().map(|link| link.range.clone()).collect(),
                move |index, window, cx| {
                    let link = &links[index];
                    if link.kind == crate::LinkKind::Anchor {
                        if let Some(position) = navigation.borrow().get(&link.target)
                            && let Some(scroll) = &scroll
                        {
                            let mut offset = scroll.offset();
                            offset.y += scroll.bounds().top() - position.y;
                            scroll.set_offset(offset);
                            window.refresh();
                        }
                    } else if let Some(listener) = &listener {
                        listener(link, window, cx);
                    }
                },
            );
        }
        if code_ranges.is_empty() && quote_ranges.is_empty() && markers.is_empty() {
            return surface
                .child(text)
                .child(navigation_overlay)
                .into_any_element();
        }
        let style = self.code_block_style;
        let labels = self.code_block_labels;
        let task_layout = layout.clone();
        let content = surface.child(
            div()
                .id(id)
                .relative()
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
                                        start.x - padding,
                                        start.y - vertical_padding - header_spacing,
                                    ),
                                    gpui::size(
                                        (bounds.right() - start.x + padding).max(gpui::px(0.)),
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
                                let radius = (style.corner_radius - gpui::px(1.)).max(gpui::px(0.));
                                window.paint_quad(
                                    gpui::fill(
                                        gpui::Bounds::new(
                                            block_bounds.origin
                                                + gpui::point(gpui::px(1.), gpui::px(1.)),
                                            gpui::size(
                                                (block_bounds.size.width - gpui::px(2.))
                                                    .max(gpui::px(0.)),
                                                header_height + vertical_padding - gpui::px(1.),
                                            ),
                                        ),
                                        style.header_background,
                                    )
                                    .corner_radii(
                                        gpui::Corners {
                                            top_left: radius,
                                            top_right: radius,
                                            bottom_left: gpui::px(0.),
                                            bottom_right: gpui::px(0.),
                                        },
                                    ),
                                );
                            }
                            for (range, indent, level, starts_code, ends_code) in quote_ranges {
                                let Some(start) = layout.position_for_index(range.start) else {
                                    continue;
                                };
                                let Some(end) = layout.position_for_index(range.end) else {
                                    continue;
                                };
                                let top = start.y
                                    - if starts_code {
                                        header_spacing + vertical_padding
                                    } else {
                                        gpui::px(0.)
                                    };
                                let bottom = end.y
                                    + layout.line_height()
                                    + if ends_code {
                                        vertical_padding
                                    } else {
                                        gpui::px(0.)
                                    };
                                window.paint_quad(gpui::fill(
                                    gpui::Bounds::new(
                                        gpui::point(
                                            bounds.left()
                                                + gpui::px(
                                                    indent as f32 * 24. + level as f32 * 16.,
                                                ),
                                            top,
                                        ),
                                        gpui::size(gpui::px(3.), bottom - top),
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
                        move |_, _, window, cx| {
                            let side = gpui::px(16.).min(task_layout.line_height());
                            for (offset, marker, shaped) in markers {
                                let Some(start) = task_layout.position_for_index(offset) else {
                                    continue;
                                };
                                let crate::ListMarker::Task { checked } = marker else {
                                    let marker_x = if matches!(marker, crate::ListMarker::Bullet) {
                                        start.x - bullet_indent + bullet_inset
                                    } else {
                                        start.x - list_indent
                                    };
                                    let origin = gpui::point(marker_x, start.y);
                                    let _ = shaped.paint(
                                        origin,
                                        task_layout.line_height(),
                                        gpui::TextAlign::Left,
                                        None,
                                        window,
                                        cx,
                                    );
                                    continue;
                                };
                                let origin = gpui::point(
                                    start.x - list_indent,
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
                                        .text_color(style.copy_icon_color)
                                        .size(gpui::px(16.))
                                        .into_any_element()
                                };
                                let mut header = div()
                                    .w((bounds.right() - start.x + padding).max(gpui::px(0.)))
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
                                            .active(move |button| {
                                                button.bg(style.button_active_background)
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
                                        gpui::AvailableSpace::Definite(
                                            (bounds.right() - start.x + padding).max(gpui::px(0.)),
                                        ),
                                        gpui::AvailableSpace::Definite(header_height),
                                    ),
                                    window,
                                    cx,
                                );
                                header.prepaint_at(
                                    gpui::point(start.x - padding, start.y - header_spacing),
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
        );
        content.child(navigation_overlay).into_any_element()
    }
}

impl RichText {
    fn fragment(&self, cell: &crate::projection::CellProjection) -> Self {
        let mut child = self.clone();
        child.id = ("richtext-fragment", cell.node.0 as usize).into();
        child.projection = cell.projection.clone();
        child.style = StyleRefinement::default();
        let offset = cell.offset;
        if let Some(listener) = self.on_link_click.clone() {
            child.on_link_click = Some(Rc::new(move |link, window, cx| {
                let mut link = link.clone();
                link.range = link.range.start + offset..link.range.end + offset;
                listener(&link, window, cx);
            }));
        }
        if let Some(builder) = self.link_tooltip.clone() {
            child.link_tooltip = Some(Rc::new(move |link, window, cx| {
                let mut link = link.clone();
                link.range = link.range.start + offset..link.range.end + offset;
                builder(&link, window, cx)
            }));
        }
        child
    }

    fn render_parts(&self, window: &gpui::Window) -> gpui::AnyElement {
        let mut text_style = window.text_style();
        text_style.refine(&self.style.text);
        let line_height = text_style.line_height_in_pixels(window.rem_size());
        let mut surface = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .gap(self.block_spacing);
        surface.style().refine(&self.style);
        let style = &self.table_style;
        for part in &self.projection.parts {
            match part {
                crate::projection::RenderPart::Text(cell) => {
                    surface = surface.child(self.fragment(cell).w_full());
                }
                crate::projection::RenderPart::Embed(block) => {
                    let content = match &block.kind {
                        BlockKind::Alert { alert, body } => {
                            let style = &self.alert_styles[alert.kind.index()];
                            let title = alert.title.clone().or_else(|| style.title.clone());
                            div()
                                .w_full()
                                .min_w_0()
                                .border_l(gpui::px(3.))
                                .border_color(style.accent)
                                .bg(style.background)
                                .rounded_r(gpui::px(6.))
                                .overflow_hidden()
                                .when_some(title, |view, title| {
                                    view.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .px(gpui::px(16.))
                                            .py(gpui::px(8.))
                                            .bg(style.header_background)
                                            .border_b_1()
                                            .border_color(style.header_border)
                                            .text_color(style.accent)
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .when_some(style.header_icon.clone(), |header, icon| {
                                                header.child(
                                                    gpui::svg()
                                                        .path(icon)
                                                        .size(gpui::px(16.))
                                                        .flex_shrink_0()
                                                        .text_color(style.accent),
                                                )
                                            })
                                            .child(div().min_w_0().child(title)),
                                    )
                                })
                                .child(
                                    div().px(gpui::px(16.)).py(gpui::px(8.)).child(
                                        self.fragment(&crate::projection::CellProjection {
                                            node: block.node,
                                            offset: block.range.start,
                                            projection: body.clone(),
                                        })
                                        .w_full(),
                                    ),
                                )
                                .into_any_element()
                        }
                        BlockKind::Rule => div()
                            .w_full()
                            .h(gpui::px(1.))
                            .bg(self.quote_border_color)
                            .into_any_element(),
                        BlockKind::Image(image) => {
                            let alt = image.alt.clone();
                            let link = self
                                .projection
                                .links()
                                .iter()
                                .find(|link| link.node == block.node)
                                .cloned();
                            let source = self.image_source.as_ref().map_or_else(
                                || image.source.clone().into(),
                                |resolve| resolve(image),
                            );
                            gpui::img(source)
                                .id(("richtext-image", block.node.0 as usize))
                                .max_w_full()
                                .with_fallback(move || div().child(alt.clone()).into_any_element())
                                .when_some(link, |view, link| {
                                    let tooltip = link.clone();
                                    view.when(self.link_tooltips, |view| {
                                        let builder = self.link_tooltip.clone();
                                        view.optional_tooltip(move |window, cx| {
                                            if let Some(builder) = &builder {
                                                builder(&tooltip, window, cx)
                                            } else {
                                                Some(
                                                    cx.new(|_| LinkTooltip(tooltip.target.clone()))
                                                        .into(),
                                                )
                                            }
                                        })
                                    })
                                    .when_some(
                                        self.on_link_click.clone(),
                                        |view, listener| {
                                            view.cursor_pointer().on_click(move |_, window, cx| {
                                                cx.stop_propagation();
                                                listener(&link, window, cx);
                                            })
                                        },
                                    )
                                })
                                .when(image.link.is_none(), |view| {
                                    view.when_some(image.title.clone(), |view, title| {
                                        view.tooltip(move |_, cx| {
                                            cx.new(|_| LinkTooltip(title.clone())).into()
                                        })
                                    })
                                })
                                .into_any_element()
                        }
                        _ => unreachable!(),
                    };
                    surface =
                        surface.child(self.block_surface(content, block.indent, block.quote_depth));
                }
                crate::projection::RenderPart::Table {
                    footnote,
                    indent,
                    quote_depth,
                    node,
                    columns,
                    rows,
                } => {
                    let padding = gpui::size(
                        style.cell_padding.width.max(gpui::px(0.)),
                        style.cell_padding.height.max(gpui::px(0.)),
                    );
                    let minimum = style
                        .min_column_width
                        .max(padding.width * 2. + gpui::px(1.));
                    let min_width: gpui::Pixels = columns
                        .iter()
                        .map(|column| match column.width {
                            crate::ColumnWidth::Fixed(width) => {
                                width.max(padding.width * 2. + gpui::px(1.))
                            }
                            crate::ColumnWidth::Flex(_) => minimum,
                        })
                        .sum();
                    let all_fixed = columns
                        .iter()
                        .all(|column| matches!(column.width, crate::ColumnWidth::Fixed(_)));
                    let min_width = min_width + gpui::px(2.);
                    let mut table = div()
                        .w_full()
                        .when(all_fixed, |table| table.w(min_width))
                        .min_w(min_width)
                        .flex()
                        .flex_col()
                        .border_1()
                        .border_color(style.border_color)
                        .rounded(style.corner_radius.max(gpui::px(0.)))
                        .overflow_hidden()
                        .bg(style.background);
                    let mut body_index = 0;
                    for (row_index, row) in rows.iter().enumerate() {
                        let mut row_view = div()
                            .id(("table-row", row.node.0 as usize))
                            .w_full()
                            .flex()
                            .items_stretch();
                        if row.header {
                            row_view = row_view.bg(style.header_background);
                        } else {
                            if body_index % 2 == 1 {
                                row_view = row_view
                                    .when_some(style.alternate_background, |row, color| {
                                        row.bg(color)
                                    });
                            }
                            body_index += 1;
                        }
                        if style.row_borders && row_index + 1 < rows.len() {
                            row_view = row_view.border_b_1().border_color(style.border_color);
                        }
                        for (index, (cell, column)) in row.cells.iter().zip(columns).enumerate() {
                            let mut cell_view = div()
                                .id(("table-cell", cell.node.0 as usize))
                                .flex()
                                .flex_col()
                                .px(padding.width)
                                .py(padding.height)
                                .flex_shrink_0();
                            cell_view = match column.vertical_align {
                                crate::CellVerticalAlign::Top => cell_view.justify_start(),
                                crate::CellVerticalAlign::Center => cell_view.justify_center(),
                                crate::CellVerticalAlign::Bottom => cell_view.justify_end(),
                            };
                            cell_view = match column.width {
                                crate::ColumnWidth::Fixed(width) => {
                                    cell_view.w(width.max(padding.width * 2. + gpui::px(1.)))
                                }
                                crate::ColumnWidth::Flex(weight) => cell_view
                                    .flex_basis(gpui::px(0.))
                                    .flex_grow(weight)
                                    .min_w(minimum),
                            };
                            if style.column_borders && index + 1 < columns.len() {
                                cell_view = cell_view.border_r_1().border_color(style.border_color);
                            }
                            row_view = row_view.child(
                                cell_view.child(
                                    self.fragment(cell)
                                        .w_full()
                                        .min_h(line_height)
                                        .when(*footnote, |view| {
                                            let size =
                                                text_style.font_size.to_pixels(window.rem_size())
                                                    * self.footnote_style.font_scale;
                                            view.text_size(size)
                                                .line_height(size * self.footnote_style.line_height)
                                        })
                                        .text_align(column.align),
                                ),
                            );
                        }
                        table = table.child(row_view);
                    }
                    surface = surface.child(
                        self.block_surface(
                            div()
                                .id(("table-scroll", node.0 as usize))
                                .w_full()
                                .min_w_0()
                                .overflow_x_scroll()
                                .child(table)
                                .into_any_element(),
                            *indent,
                            *quote_depth,
                        ),
                    );
                }
            }
        }
        surface.into_any_element()
    }

    fn block_surface(
        &self,
        mut content: gpui::AnyElement,
        indent: usize,
        quote_depth: usize,
    ) -> gpui::AnyElement {
        for _ in 0..quote_depth {
            content = div()
                .pl(gpui::px(13.))
                .border_l(gpui::px(3.))
                .border_color(self.quote_border_color)
                .child(content)
                .into_any_element();
        }
        div()
            .pl(gpui::px(indent as f32 * 24.))
            .child(content)
            .into_any_element()
    }
}

struct LinkTooltip(gpui::SharedString);

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn internal_links_scroll_across_fragments_and_back_after_reflow(cx: &mut gpui::TestAppContext) {
        use crate::compose::*;
        struct Page {
            projection: Arc<Projection>,
            scroll: gpui::ScrollHandle,
            positions: Rc<std::cell::RefCell<AnchorPositions>>,
        }
        impl gpui::Render for Page {
            fn render(
                &mut self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> impl IntoElement {
                let mut text = RichText::new("navigation", self.projection.clone())
                    .scroll_handle(&self.scroll)
                    .text_size(gpui::px(16.))
                    .line_height(gpui::px(24.));
                text.navigation = Some(self.positions.clone());
                div()
                    .id("viewport")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(text)
            }
        }
        let mut roots = vec![
            paragraph()
                .child(anchor("entry").child(anchor_link("destination").child(text("Continue")))),
        ];
        roots.extend((0..30).map(|index| {
            paragraph().child(text(format!(
                "Section {index}: a paragraph that wraps when the viewport becomes narrow."
            )))
        }));
        roots.push(rule());
        roots.push(
            alert(crate::AlertKind::Note)
                .child(paragraph().child(
                    anchor("destination").child(anchor_link("entry").child(text("Return"))),
                )),
        );
        let projection = Arc::new(
            crate::Registry::with_standard_nodes()
                .compile(&crate::Document::from_roots(roots).unwrap())
                .unwrap(),
        );
        let positions = Rc::new(std::cell::RefCell::new(AnchorPositions::default()));
        let scroll = gpui::ScrollHandle::new();
        let observed_positions = positions.clone();
        let observed_scroll = scroll.clone();
        let window = cx.open_window(gpui::size(gpui::px(400.), gpui::px(240.)), move |_, _| {
            Page {
                projection,
                scroll,
                positions,
            }
        });
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let entry = observed_positions.borrow()["entry"];
        visual.simulate_click(
            entry + gpui::point(gpui::px(2.), gpui::px(8.)),
            Default::default(),
        );
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert!(observed_scroll.offset().y < gpui::px(-240.));
        visual.simulate_resize(gpui::size(gpui::px(220.), gpui::px(240.)));
        // Return to the end after wrapping changes its vertical position.
        observed_scroll.scroll_to_bottom();
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let destination = observed_positions.borrow()["destination"];
        visual.simulate_click(
            destination + gpui::point(gpui::px(2.), gpui::px(8.)),
            Default::default(),
        );
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert_eq!(observed_scroll.offset().y, gpui::px(0.));
    }

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
