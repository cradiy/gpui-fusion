use super::{InteractiveTextClickEvent, TextLayout};
use crate::{
    App, Bounds, ClipboardItem, CursorStyle, DispatchPhase, FocusHandle, Hitbox, Hsla,
    KeyDownEvent, LongPressEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PaintQuad, Pixels, Point, SharedString, TextAlign, Window, fill, point, px, size,
};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};
use unicode_segmentation::UnicodeSegmentation;

mod platform;

pub(super) struct TextSelection {
    pub focus: FocusHandle,
    text: SharedString,
    anchor: usize,
    head: usize,
    dragging: bool,
    dragged: bool,
}

impl TextSelection {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus: cx.focus_handle(),
            text: "".into(),
            anchor: 0,
            head: 0,
            dragging: false,
            dragged: false,
        }
    }

    pub fn sync_text(&mut self, text: SharedString) -> bool {
        if self.text != text {
            self.text = text;
            self.anchor = 0;
            self.head = 0;
            self.dragging = false;
            self.dragged = false;
            return true;
        }
        false
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

struct Row {
    range: Range<usize>,
    line_start: usize,
    layout: std::sync::Arc<crate::LineLayout>,
    origin: Point<Pixels>,
    base_x: Pixels,
    width: Pixels,
    height: Pixels,
    newline: bool,
}

fn rows(layout: &TextLayout, align: TextAlign) -> Vec<Row> {
    let inner = layout.0.borrow();
    let inner = inner.as_ref().expect("text is measured before selection");
    let bounds = inner.bounds.expect("text is prepainted before selection");
    let mut result = Vec::new();
    let mut line_start = 0;
    let mut y = bounds.top();
    for (line_index, line) in inner.lines.iter().enumerate() {
        let line_height = inner.height_for_line(line_index);
        let indent = inner
            .line_indents
            .get(line_index)
            .copied()
            .unwrap_or_default();
        let available_width = (bounds.size.width - indent).max(px(0.));
        y += inner
            .line_top_spacing
            .get(line_index)
            .copied()
            .unwrap_or_default();
        for index in 0..=line.wrap_boundaries().len() {
            let range = line.row_range(index).unwrap();
            let extent = line.row_extent(index).unwrap();
            let unwrapped = &line.layout.unwrapped_layout;
            let base_x = extent.start;
            let width = extent.end - base_x;
            let offset = match align {
                TextAlign::Left => px(0.),
                TextAlign::Center => (available_width - width) / 2.,
                TextAlign::Right => available_width - width,
            };
            result.push(Row {
                range: line_start + range.start..line_start + range.end,
                line_start,
                layout: unwrapped.clone(),
                origin: point(bounds.left() + indent + offset, y),
                base_x,
                width,
                height: line_height,
                newline: index == line.wrap_boundaries().len()
                    && line_start + line.len() < inner.len,
            });
            y += line_height;
        }
        line_start += line.len() + 1;
    }
    result
}

fn index_at(rows: &[Row], position: Point<Pixels>) -> usize {
    let Some(first) = rows.first() else {
        return 0;
    };
    if position.y < first.origin.y {
        return 0;
    }
    for row in rows {
        if position.y < row.origin.y + row.height {
            return (row.line_start
                + row
                    .layout
                    .closest_index_for_x((position.x - row.origin.x + row.base_x).clamp(
                        row.base_x,
                        (row.base_x + row.width - px(0.001)).max(row.base_x),
                    )))
            .clamp(row.range.start, row.range.end);
        }
    }
    let last = rows.last().unwrap();
    last.line_start + last.layout.len
}

fn character_at(rows: &[Row], position: Point<Pixels>) -> Option<usize> {
    let row = rows.iter().find(|row| {
        position.y >= row.origin.y
            && position.y < row.origin.y + row.height
            && position.x >= row.origin.x
            && position.x < row.origin.x + row.width
    })?;
    row.layout
        .index_for_x(position.x - row.origin.x + row.base_x)
        .map(|index| row.line_start + index)
}

pub(super) fn selection_quads(
    layout: &TextLayout,
    selected: Range<usize>,
    align: TextAlign,
    color: Hsla,
) -> Vec<PaintQuad> {
    if selected.is_empty() {
        return Vec::new();
    }
    let mut quads = Vec::new();
    for row in rows(layout, align) {
        if row.range.is_empty() {
            continue;
        }
        let end = row.line_start + row.layout.len;
        let mut newline = row.newline && selected.start <= end && selected.end > end;
        let local = selected.start.saturating_sub(row.line_start)
            ..selected
                .end
                .saturating_sub(row.line_start)
                .min(row.layout.len);
        for range in row.layout.selection_ranges(local) {
            let left = range.start.max(row.base_x);
            let mut right = range.end.min(row.base_x + row.width);
            if right > left {
                if newline && right == row.base_x + row.width {
                    right += px(3.);
                    newline = false;
                }
                quads.push(fill(
                    Bounds::new(
                        row.origin + point(left - row.base_x, px(0.)),
                        size(right - left, row.height),
                    ),
                    color,
                ));
            }
        }
        if newline {
            quads.push(fill(
                Bounds::new(
                    row.origin + point(row.width, px(0.)),
                    size(px(3.), row.height),
                ),
                color,
            ));
        }
    }
    quads
}

type ClickListener = Box<dyn Fn(&[Range<usize>], InteractiveTextClickEvent, &mut Window, &mut App)>;

pub(super) fn register_handlers(
    selection: Rc<RefCell<TextSelection>>,
    layout: TextLayout,
    hitbox: Hitbox,
    mouse_down: Rc<Cell<Option<usize>>>,
    click: Option<ClickListener>,
    ranges: Vec<Range<usize>>,
    window: &mut Window,
    cx: &App,
) {
    let rows = Rc::new(rows(&layout, window.text_style().text_align));
    let focus = selection.borrow().focus.clone();
    window.handle_input(
        &focus,
        platform::SelectionInput::new(selection.clone(), rows.clone(), hitbox.bounds),
        cx,
    );
    window.on_mouse_event({
        let state = selection.clone();
        let rows = rows.clone();
        let hitbox = hitbox.clone();
        move |event: &crate::TextInputFocusEvent, phase, window, cx| {
            if phase.bubble() && hitbox.is_hovered(window) {
                let index = index_at(&rows, event.position);
                let focus = {
                    let mut state = state.borrow_mut();
                    state.anchor = index;
                    state.head = index;
                    state.focus.clone()
                };
                window.focus(&focus, cx);
                window.refresh();
                window.prevent_default();
                cx.stop_propagation();
            }
        }
    });
    let hovered = character_at(&rows, window.mouse_position());
    window.set_cursor_style(
        if hovered.is_some_and(|index| ranges.iter().any(|range| range.contains(&index))) {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        },
        &hitbox,
    );
    window.on_mouse_event({
        let state = selection.clone();
        let rows = rows.clone();
        let hitbox = hitbox.clone();
        let down = mouse_down.clone();
        move |event: &LongPressEvent, phase, window, cx| {
            if !phase.bubble() || window.default_prevented() || !hitbox.is_hovered(window) {
                return;
            }
            let Some(index) = character_at(&rows, event.position) else {
                return;
            };
            let mut state = state.borrow_mut();
            let Some((start, word)) = state
                .text
                .split_word_bound_indices()
                .find(|(start, word)| (*start..*start + word.len()).contains(&index))
            else {
                return;
            };
            let end = start + word.len();
            state.anchor = start;
            state.head = end;
            state.dragging = false;
            state.dragged = false;
            let focus = state.focus.clone();
            drop(state);
            down.set(None);
            window.focus(&focus, cx);
            window.refresh();
            if !cfg!(target_os = "android") {
                window.prevent_default();
            }
            cx.stop_propagation();
        }
    });
    window.on_mouse_event({
        let state = selection.clone();
        let rows = rows.clone();
        let hitbox = hitbox.clone();
        let down = mouse_down.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            if event.button != MouseButton::Left {
                return;
            }
            if phase == DispatchPhase::Capture && !hitbox.is_hovered(window) {
                let mut state = state.borrow_mut();
                let changed = !state.range().is_empty() || state.dragging;
                state.anchor = 0;
                state.head = 0;
                state.dragging = false;
                state.dragged = false;
                if state.focus.is_focused(window) {
                    window.blur();
                }
                down.set(None);
                if changed {
                    window.refresh();
                }
                return;
            }
            if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                return;
            }
            let index = index_at(&rows, event.position);
            let focus = {
                let mut state = state.borrow_mut();
                if event.click_count >= 2 {
                    let range = state
                        .text
                        .split_word_bound_indices()
                        .find(|(start, word)| (*start..*start + word.len()).contains(&index))
                        .map(|(start, word)| start..start + word.len())
                        .unwrap_or(index..index);
                    state.anchor = range.start;
                    state.head = range.end;
                    state.dragging = false;
                    state.dragged = true;
                } else {
                    if !event.modifiers.shift {
                        state.anchor = index;
                    }
                    state.head = index;
                    state.dragging = true;
                    state.dragged = event.modifiers.shift;
                }
                state.focus.clone()
            };
            down.set(character_at(&rows, event.position));
            window.focus(&focus, cx);
            window.refresh();
            cx.stop_propagation();
        }
    });
    window.on_mouse_event({
        let state = selection.clone();
        let rows = rows.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            let mut state = state.borrow_mut();
            if !state.dragging {
                return;
            }
            if event.pressed_button != Some(MouseButton::Left) || !state.focus.is_focused(window) {
                state.dragging = false;
                return;
            }
            let head = index_at(&rows, event.position);
            state.dragged |= head != state.head;
            state.head = head;
            drop(state);
            window.refresh();
            cx.stop_propagation();
        }
    });
    window.on_mouse_event({
        let state = selection.clone();
        move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                return;
            }
            let should_click = {
                let mut state = state.borrow_mut();
                if !state.dragging {
                    return;
                }
                state.dragging = false;
                let head = index_at(&rows, event.position);
                state.dragged |= head != state.head;
                state.head = head;
                !state.dragged && state.range().is_empty() && hitbox.is_hovered(window)
            };
            let start = mouse_down.take();
            if should_click
                && let (Some(click), Some(start), Some(end)) =
                    (&click, start, character_at(&rows, event.position))
            {
                click(
                    &ranges,
                    InteractiveTextClickEvent {
                        mouse_down_index: start,
                        mouse_up_index: end,
                    },
                    window,
                    cx,
                );
            }
            window.refresh();
            cx.stop_propagation();
        }
    });
    window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let mut state = selection.borrow_mut();
        if !state.focus.is_focused(window) {
            return;
        }
        let modifiers = event.keystroke.modifiers;
        let command = if cfg!(target_family = "wasm") {
            modifiers.platform != modifiers.control
        } else if cfg!(target_os = "macos") {
            modifiers.platform && !modifiers.control
        } else {
            modifiers.control && !modifiers.platform
        };
        if !command || modifiers.alt || modifiers.shift {
            return;
        }
        match event.keystroke.key.as_str() {
            "a" => {
                state.anchor = 0;
                state.head = state.text.len();
            }
            "c" => {
                if state.range().is_empty() {
                    return;
                }
                cx.write_to_clipboard(ClipboardItem::new_string(
                    state.text[state.range()].to_owned(),
                ));
            }
            _ => return,
        }
        drop(state);
        window.refresh();
        cx.stop_propagation();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Context, FontWeight, InteractiveText, IntoElement, Render, StyledText, TestAppContext,
        VisualTestContext, WindowHandle, div, prelude::*,
    };

    struct Demo {
        text: SharedString,
        layout: TextLayout,
        clicks: usize,
        align: TextAlign,
        truncate: bool,
    }
    impl Render for Demo {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let text = StyledText::new(self.text.clone())
                .with_highlights([(0..4, FontWeight::BOLD.into())]);
            self.layout = text.layout().clone();
            let clicked = cx.listener(|this, _: &usize, _, _| this.clicks += 1);
            div().size_full().p_4().child(
                div()
                    .w(px(180.))
                    .text_align(self.align)
                    .when(self.truncate, |this| this.truncate())
                    .child(
                        InteractiveText::new("selectable", text)
                            .selectable(crate::rgba(0x4488ff55))
                            .on_click(vec![0..4], move |index, window, cx| {
                                clicked(&index, window, cx)
                            }),
                    ),
            )
        }
    }

    #[test]
    fn bidi_readonly_selection_uses_visual_spans_and_alignment() {
        use crate::{
            FontId, GlyphId, LineLayout, ShapedGlyph, ShapedRun, WrappedLine, WrappedLineLayout,
        };
        use std::sync::Arc;
        let text: SharedString = "aאב12".into();
        let glyphs = [
            (0, 1, false),
            (5, 6, false),
            (6, 7, false),
            (3, 5, true),
            (1, 3, true),
        ]
        .into_iter()
        .enumerate()
        .map(|(visual, (index, cluster_end, is_rtl))| ShapedGlyph {
            id: GlyphId(0),
            position: point(px(visual as f32 * 10.), px(0.)),
            index,
            cluster_end,
            advance: px(10.),
            is_rtl,
            is_emoji: false,
        })
        .collect();
        let line = WrappedLine {
            layout: Arc::new(WrappedLineLayout {
                unwrapped_layout: Arc::new(LineLayout {
                    font_size: px(16.),
                    width: px(50.),
                    ascent: px(12.),
                    descent: px(4.),
                    runs: vec![ShapedRun {
                        font_id: FontId(0),
                        glyphs,
                    }],
                    len: 7,
                }),
                ..Default::default()
            }),
            text: text.clone(),
            decoration_runs: Vec::new(),
        };
        let layout = TextLayout(Rc::new(RefCell::new(Some(super::super::TextLayoutInner {
            text,
            len: 7,
            lines: smallvec::smallvec![line],
            line_height: px(20.),
            line_top_spacing: vec![px(0.)],
            line_indents: Vec::new(),
            line_heights: Vec::new(),
            wrap_width: None,
            size: Some(size(px(200.), px(20.))),
            bounds: Some(Bounds::new(
                point(px(100.), px(50.)),
                size(px(200.), px(20.)),
            )),
        }))));
        let quads = selection_quads(&layout, 0..3, TextAlign::Right, crate::black());
        assert_eq!(quads.len(), 2);
        assert_eq!(
            quads[0].bounds,
            Bounds::new(point(px(250.), px(50.)), size(px(10.), px(20.)))
        );
        assert_eq!(
            quads[1].bounds,
            Bounds::new(point(px(290.), px(50.)), size(px(10.), px(20.)))
        );
        let rows = rows(&layout, TextAlign::Right);
        assert_eq!(index_at(&rows, point(px(299.), px(55.))), 1);
        assert_eq!(character_at(&rows, point(px(285.), px(55.))), Some(3));
    }

    #[crate::test]
    fn line_typography_preserves_wrapped_hit_testing_and_selection(cx: &mut TestAppContext) {
        let (_, mut visual) = setup(cx, TextAlign::Left);
        let source = "Aperture\n\n光谱 measurements across bands\nNotes";
        let large_start = source.find('光').unwrap();
        for width in [300., 110.] {
            let text = StyledText::new(source).with_line_typography(vec![
                crate::TextLineTypography {
                    line_start: 0,
                    font_size: px(28.),
                    line_height: px(38.),
                },
                crate::TextLineTypography {
                    line_start: large_start,
                    font_size: px(22.),
                    line_height: px(32.),
                },
            ]);
            let layout = text.layout().clone();
            visual.draw(
                point(px(0.), px(0.)),
                size(
                    crate::AvailableSpace::Definite(px(width)),
                    crate::AvailableSpace::MinContent,
                ),
                move |_, _| {
                    div()
                        .w(px(width))
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .whitespace_normal()
                        .child(text)
                },
            );
            let visual_rows = rows(&layout, TextAlign::Left);
            assert!(visual_rows.len() > 4);
            for row in &visual_rows {
                let (font_size, height) = if row.line_start == 0 {
                    (28., 38.)
                } else if row.line_start == large_start {
                    (22., 32.)
                } else {
                    (16., 24.)
                };
                assert_eq!(row.layout.font_size, px(font_size));
                assert_eq!(row.height, px(height));
                if !row.range.is_empty() {
                    assert_eq!(
                        index_at(&visual_rows, row.origin + point(px(0.1), row.height / 2.)),
                        row.range.start
                    );
                    let quads = selection_quads(
                        &layout,
                        row.range.clone(),
                        TextAlign::Left,
                        crate::black(),
                    );
                    assert!(!quads.is_empty());
                    assert!(
                        quads
                            .iter()
                            .all(|quad| quad.bounds.size.height == row.height)
                    );
                }
            }
            for pair in visual_rows.windows(2) {
                assert_eq!(pair[1].origin.y, pair[0].origin.y + pair[0].height);
            }
        }
    }

    #[crate::test]
    fn line_spacing_preserves_wrapped_selection_and_source_coordinates(cx: &mut TestAppContext) {
        let (_, mut visual) = setup(cx, TextAlign::Left);
        let source = "Title\nSeveral words wrap onto following rows.\nEnd";
        for width in [180., 90.] {
            let mut layouts = Vec::new();
            for spacing in [Vec::new(), vec![(6, px(32.))]] {
                let text = StyledText::new(source).with_line_top_spacing(spacing);
                layouts.push(text.layout().clone());
                visual.draw(
                    point(px(0.), px(0.)),
                    size(
                        crate::AvailableSpace::Definite(px(width)),
                        crate::AvailableSpace::MinContent,
                    ),
                    move |_, _| {
                        div()
                            .w(px(width))
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .whitespace_normal()
                            .child(text)
                    },
                );
            }
            let original = rows(&layouts[0], TextAlign::Left);
            let spaced = rows(&layouts[1], TextAlign::Left);
            assert_eq!(original.len(), spaced.len());
            for (before, after) in original.iter().zip(&spaced) {
                assert_eq!(before.range, after.range);
                let expected_gap = if after.line_start >= 6 {
                    px(32.)
                } else {
                    px(0.)
                };
                assert_eq!(after.origin.y, before.origin.y + expected_gap);
            }
            let start = layouts[1].position_for_index(6).unwrap();
            assert_eq!(
                layouts[1].index_for_position(start + point(px(0.), px(1.))),
                Ok(6)
            );
            assert_eq!(character_at(&spaced, start - point(px(0.), px(8.))), None);
            let quads = selection_quads(
                &layouts[1],
                0..source.len(),
                TextAlign::Left,
                crate::black(),
            );
            assert!(
                quads
                    .iter()
                    .all(|quad| quad.bounds.bottom() <= start.y - px(32.)
                        || quad.bounds.top() >= start.y)
            );
            assert_eq!(layouts[1].text(), source);
        }
    }

    #[crate::test]
    fn indented_prefixes_share_wrapping_hit_testing_and_selection_geometry(
        cx: &mut TestAppContext,
    ) {
        let (_, mut visual) = setup(cx, TextAlign::Left);
        for width in [150., 72.] {
            let mut reference_rows = None;
            for prefix in ["[ ] ", "[x] ", "87. ", "509. "] {
                let source = format!("{prefix}A boat returns before sunset.");
                let len = source.len();
                let text = StyledText::new(source.clone()).with_line_indents(vec![
                    crate::TextLineIndent {
                        line_start: 0,
                        prefix_len: prefix.len(),
                        width: px(32.),
                    },
                ]);
                let layout = text.layout().clone();
                visual.draw(
                    point(px(0.), px(0.)),
                    size(
                        crate::AvailableSpace::Definite(px(width)),
                        crate::AvailableSpace::MinContent,
                    ),
                    move |_, _| {
                        div()
                            .w(px(width))
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .whitespace_normal()
                            .child(text)
                    },
                );
                let shaped_rows = rows(&layout, TextAlign::Left);
                assert!(shaped_rows.len() > 1);
                let content_rows: Vec<_> = shaped_rows
                    .iter()
                    .map(|row| {
                        assert_eq!(row.origin.x, px(32.));
                        (
                            row.range.start.saturating_sub(prefix.len()),
                            row.range.end - prefix.len(),
                        )
                    })
                    .collect();
                if let Some(reference) = &reference_rows {
                    assert_eq!(&content_rows, reference);
                } else {
                    reference_rows = Some(content_rows);
                }
                for row in &shaped_rows {
                    let index = row.range.start.max(prefix.len());
                    let position = layout.position_for_index(index).unwrap();
                    assert_eq!(
                        position.x,
                        px(32.),
                        "prefix={prefix:?} row={:?} position={position:?}",
                        row.range
                    );
                    assert_eq!(
                        character_at(&shaped_rows, position + point(px(0.5), px(1.))),
                        Some(index)
                    );
                }
                assert!(
                    selection_quads(&layout, 0..len, TextAlign::Left, crate::black())
                        .iter()
                        .all(|quad| quad.bounds.left() >= px(32.))
                );
                assert_eq!(layout.text(), source);
            }
        }
    }

    #[crate::test]
    fn compact_empty_lines_keep_source_and_following_hit_geometry(cx: &mut TestAppContext) {
        let (_, mut visual) = setup(cx, TextAlign::Left);
        let source = "Lime\n\nIris";
        let text = StyledText::new(source).with_empty_line_heights(vec![(5, px(8.)), (6, px(2.))]);
        let layout = text.layout().clone();
        visual.draw(
            point(px(0.), px(0.)),
            size(
                crate::AvailableSpace::Definite(px(180.)),
                crate::AvailableSpace::MinContent,
            ),
            move |_, _| {
                div()
                    .w(px(180.))
                    .text_size(px(16.))
                    .line_height(px(24.))
                    .child(text)
            },
        );
        let shaped_rows = rows(&layout, TextAlign::Left);
        assert_eq!(
            shaped_rows.iter().map(|row| row.height).collect::<Vec<_>>(),
            vec![px(24.), px(8.), px(24.)]
        );
        let start = layout.position_for_index(6).unwrap();
        assert_eq!(start.y, px(32.));
        assert_eq!(
            character_at(&shaped_rows, start + point(px(1.), px(1.))),
            Some(6)
        );
        assert_eq!(
            layout.index_for_position(start + point(px(1.), px(1.))),
            Ok(6)
        );
        let quads = selection_quads(&layout, 6..source.len(), TextAlign::Left, crate::black());
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].bounds.top(), start.y);
        assert_eq!(quads[0].bounds.size.height, px(24.));
        assert_eq!(layout.text(), source);
    }

    fn setup(cx: &mut TestAppContext, align: TextAlign) -> (WindowHandle<Demo>, VisualTestContext) {
        let window = cx.open_window(size(px(400.), px(400.)), move |_, _| Demo {
            text: "link 中文😀\nA long styled line that wraps across several rows.\n\nLast".into(),
            layout: TextLayout::default(),
            clicks: 0,
            align,
            truncate: false,
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        (window, visual)
    }

    fn position(
        window: WindowHandle<Demo>,
        visual: &mut VisualTestContext,
        index: usize,
    ) -> Point<Pixels> {
        window
            .update(&mut visual.cx, |view, _, _| {
                let rows = rows(&view.layout, view.align);
                let row = rows
                    .iter()
                    .find(|row| row.range.start <= index && index <= row.range.end)
                    .unwrap();
                row.origin
                    + point(
                        row.layout.x_for_index(index - row.line_start) - row.base_x,
                        row.height / 2.,
                    )
            })
            .unwrap()
    }

    fn copy(visual: &mut VisualTestContext) -> Option<String> {
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-c"));
        visual.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
    }

    #[crate::test]
    fn selectable_text_drag_copies_unicode_across_wrapped_rows(cx: &mut TestAppContext) {
        let (window, mut visual) = setup(cx, TextAlign::Center);
        let start = position(window, &mut visual, 5);
        let end = position(window, &mut visual, 35);
        visual.simulate_mouse_down(end, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(start, MouseButton::Left, Default::default());
        visual.simulate_mouse_up(start, MouseButton::Left, Default::default());
        let expected = window
            .update(&mut visual.cx, |view, _, _| view.text[5..35].to_owned())
            .unwrap();
        assert_eq!(copy(&mut visual), Some(expected));
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
        let text = window
            .update(&mut visual.cx, |view, _, _| view.text.to_string())
            .unwrap();
        assert_eq!(copy(&mut visual), Some(text));
    }

    #[crate::test]
    fn system_selection_is_utf16_and_cannot_modify_read_only_text(cx: &mut TestAppContext) {
        use crate::PlatformWindow;
        let (window, mut visual) = setup(cx, TextAlign::Left);
        let at = position(window, &mut visual, 5);
        visual.simulate_click(at, Default::default());
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let mut platform = cx.test_window(window.into());
        let mut input = platform
            .take_input_handler()
            .expect("focused selection registers a system handler");
        assert!(!input.query_accepts_text_input());
        input.set_selected_text_range(Range { start: 9, end: 5 });
        let selection = input.selected_text_range(false).unwrap();
        assert_eq!(selection.range, 5..9);
        assert!(selection.reversed);
        assert_eq!(
            input.text_for_range(selection.range, &mut None).as_deref(),
            Some("中文😀")
        );
        assert!(input.bounds_for_range(5..5).is_some());
        assert!(input.bounds_for_range(9..9).is_some());
        input.replace_text_in_range(None, "replacement");
        input.replace_and_mark_text_in_range(None, "composition", None);
        assert_eq!(
            input.text_for_range(5..9, &mut None).as_deref(),
            Some("中文😀")
        );
        platform.set_input_handler(input);
        assert_eq!(copy(&mut visual).as_deref(), Some("中文😀"));
    }

    #[crate::test]
    fn selectable_text_drag_does_not_activate_links_and_resets_on_content_change(
        cx: &mut TestAppContext,
    ) {
        let (window, mut visual) = setup(cx, TextAlign::Left);
        let start = position(window, &mut visual, 0);
        let end = position(window, &mut visual, 3);
        visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
        visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
        visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
        assert_eq!(copy(&mut visual).as_deref(), Some("lin"));
        window
            .update(&mut visual.cx, |view, _, _| assert_eq!(view.clicks, 0))
            .unwrap();
        let link_end = position(window, &mut visual, 4);
        visual.simulate_click(
            point(end.x + (link_end.x - end.x) * 0.8, end.y),
            Default::default(),
        );
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
        window
            .update(&mut visual.cx, |view, _, cx| {
                assert_eq!(view.clicks, 1);
                view.text = "link".into();
                cx.notify();
            })
            .unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert_eq!(copy(&mut visual).as_deref(), Some("lin"));
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
        assert_eq!(copy(&mut visual).as_deref(), Some("link"));
    }

    #[crate::test]
    fn selectable_text_skips_empty_row_highlights_but_copies_newlines(cx: &mut TestAppContext) {
        let (window, mut visual) = setup(cx, TextAlign::Right);
        let expected = window
            .update(&mut visual.cx, |view, _, _| {
                let rows = rows(&view.layout, view.align);
                assert!(rows.len() > 4);
                assert!(rows.iter().any(|row| row.range.is_empty()));
                let quads =
                    selection_quads(&view.layout, 0..view.text.len(), view.align, crate::blue());
                let visible_rows: Vec<_> =
                    rows.iter().filter(|row| !row.range.is_empty()).collect();
                assert_eq!(quads.len(), visible_rows.len());
                for (quad, row) in quads.iter().zip(visible_rows) {
                    assert_eq!(quad.bounds.origin, row.origin);
                    assert!(quad.bounds.size.width > px(0.));
                    assert_eq!(quad.bounds.size.height, row.height);
                }
                view.text.to_string()
            })
            .unwrap();
        let start = position(window, &mut visual, 0);
        visual.simulate_click(start, Default::default());
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
        assert_eq!(copy(&mut visual), Some(expected));
    }

    #[crate::test]
    fn selecting_another_text_clears_previous_highlights(cx: &mut TestAppContext) {
        struct Pair;
        impl Render for Pair {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(16.))
                    .line_height(px(24.))
                    .child(
                        InteractiveText::new("first", StyledText::new("Copper"))
                            .selectable(crate::black()),
                    )
                    .child(
                        InteractiveText::new("second", StyledText::new("Silver"))
                            .selectable(crate::black()),
                    )
            }
        }
        let window = cx.open_window(size(px(300.), px(200.)), |_, _| Pair);
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        for y in [12., 36., 12.] {
            visual.simulate_click(point(px(4.), px(y)), Default::default());
            visual.update(|window, cx| {
                window.draw(cx).clear();
                assert!(window.rendered_frame.scene.quads.is_empty());
            });
            visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
            visual.update(|window, cx| {
                window.draw(cx).clear();
                assert_eq!(window.rendered_frame.scene.quads.len(), 1);
            });
        }
        visual.simulate_click(point(px(250.), px(150.)), Default::default());
        visual.update(|window, cx| {
            window.draw(cx).clear();
            assert!(window.rendered_frame.scene.quads.is_empty());
        });
    }

    #[crate::test]
    fn selectable_text_copies_displayed_ellipsis_and_clears_on_outside_click(
        cx: &mut TestAppContext,
    ) {
        let (window, mut visual) = setup(cx, TextAlign::Left);
        window
            .update(&mut visual.cx, |view, _, cx| {
                view.text = "link 中文😀 repeated text that is wider than the block".into();
                view.truncate = true;
                cx.notify();
            })
            .unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear();
        });
        let (displayed, pos) = window
            .update(&mut visual.cx, |view, _, _| {
                let layout = view.layout.0.borrow();
                let layout = layout.as_ref().unwrap();
                assert_ne!(layout.text, view.text);
                (layout.text.to_string(), layout.bounds.unwrap().center())
            })
            .unwrap();
        visual.simulate_click(pos, Default::default());
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
        assert_eq!(copy(&mut visual), Some(displayed));
        visual.simulate_click(point(px(380.), px(380.)), Default::default());
        visual.simulate_keystrokes(&crate::platform_shortcut("ctrl-a"));
        visual.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("unchanged".into())));
        assert_eq!(copy(&mut visual).as_deref(), Some("unchanged"));
    }
}
