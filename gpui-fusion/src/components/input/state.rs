use std::{borrow::Cow, ops::Range, rc::Rc, time::Duration};

use gpui::{
    App, Bounds, CaretAffinity, ClipboardItem, Context, CursorStyle, DispatchPhase,
    EntityInputHandler, FocusHandle, Focusable, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, PreeditSelection, Render, ScrollHandle, SharedString, Task,
    TextCaret, UTF16Selection, Window, WrappedLine, div, point, prelude::*, px,
};
use unicode_segmentation::UnicodeSegmentation;

use super::scrollbar::ScrollbarState;
use super::{InputActionEvent, InputEvent, InputMode, actions::*, element::TextElement};

#[derive(Clone)]
pub(super) struct TextLayout {
    pub(super) lines: Rc<Vec<Rc<WrappedLine>>>,
    pub(super) line_starts: Rc<Vec<usize>>,
    pub(super) row_starts: Rc<Vec<usize>>,
    pub(super) overhang: Pixels,
    pub(super) line_height: Pixels,
    pub(super) shaping_run: Option<gpui::TextRun>,
    shape_key: Option<TextShapeKey>,
    tab_stop: Pixels,
}

#[derive(Clone, Copy)]
pub(super) struct TextLayoutSettings {
    pub font_size: Pixels,
    pub wrap_width: Option<Pixels>,
    pub line_height: Pixels,
    pub tab_size: usize,
}

#[derive(Clone, PartialEq)]
struct TextShapeKey {
    text: SharedString,
    font_size: Pixels,
    runs: Vec<gpui::TextRun>,
    wrap_width: Option<Pixels>,
    tab_size: usize,
}

fn preedit_offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units + ch.len_utf16() > offset {
            return byte;
        }
        units += ch.len_utf16();
    }
    text.len()
}

impl TextLayout {
    #[cfg(test)]
    pub(super) fn new(
        lines: Vec<WrappedLine>,
        line_starts: Vec<usize>,
        line_height: Pixels,
    ) -> Self {
        let lines: Vec<_> = lines.into_iter().map(Rc::new).collect();
        let (row_starts, overhang) = Self::measure_rows(&lines);
        Self {
            lines: Rc::new(lines),
            line_starts: Rc::new(line_starts),
            row_starts,
            overhang,
            line_height,
            shaping_run: None,
            shape_key: None,
            tab_stop: px(0.),
        }
    }

    pub(super) fn shape(
        text: SharedString,
        runs: Vec<gpui::TextRun>,
        settings: TextLayoutSettings,
        shaper: &gpui::WindowTextSystem,
        previous: Option<&Self>,
    ) -> Self {
        let TextLayoutSettings {
            font_size,
            wrap_width,
            line_height,
            tab_size,
        } = settings;
        let key = TextShapeKey {
            text,
            font_size,
            runs,
            wrap_width,
            tab_size,
        };
        if let Some(previous) = previous.filter(|layout| layout.shape_key.as_ref() == Some(&key)) {
            let mut layout = previous.clone();
            layout.line_height = line_height;
            return layout;
        }
        let tab_stop = key.runs.first().map_or(px(0.), |run| {
            shaper.layout_width(shaper.resolve_font(&run.font), font_size, ' ')
                * tab_size.max(1) as f32
        });
        let starts: Vec<_> = std::iter::once(0)
            .chain(key.text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect();
        let reusable = previous.filter(|layout| {
            let Some(old) = &layout.shape_key else {
                return false;
            };
            if old.font_size != font_size
                || old.wrap_width != wrap_width
                || old.tab_size != tab_size
            {
                return false;
            }
            match (old.runs.as_slice(), key.runs.as_slice()) {
                ([old], [new]) => {
                    let mut old = old.clone();
                    old.len = new.len;
                    old == *new
                }
                _ => false,
            }
        });
        let lines = if let Some(previous) = reusable {
            let paragraphs: Vec<_> = key.text.split('\n').collect();
            let prefix = previous
                .lines
                .iter()
                .zip(&paragraphs)
                .take_while(|(line, text)| line.text.as_ref() == **text)
                .count();
            let suffix = previous.lines[prefix..]
                .iter()
                .rev()
                .zip(paragraphs[prefix..].iter().rev())
                .take_while(|(line, text)| line.text.as_ref() == **text)
                .count();
            let mut lines = Vec::with_capacity(paragraphs.len());
            lines.extend(previous.lines[..prefix].iter().cloned());
            let changed_end = paragraphs.len() - suffix;
            if prefix < changed_end {
                let start = starts[prefix];
                let end = starts[changed_end - 1] + paragraphs[changed_end - 1].len();
                let mut run = key.runs[0].clone();
                run.len = end - start;
                lines.extend(
                    shaper
                        .shape_text(
                            key.text[start..end].to_owned().into(),
                            font_size,
                            &[run],
                            wrap_width,
                            None,
                        )
                        .expect("failed to shape input text")
                        .into_iter()
                        .map(|line| Rc::new(line.with_tab_stops(tab_stop))),
                );
            }
            lines.extend(
                previous.lines[previous.lines.len() - suffix..]
                    .iter()
                    .cloned(),
            );
            Rc::new(lines)
        } else {
            Rc::new(
                shaper
                    .shape_text(key.text.clone(), font_size, &key.runs, wrap_width, None)
                    .expect("failed to shape input text")
                    .into_iter()
                    .map(|line| Rc::new(line.with_tab_stops(tab_stop)))
                    .collect(),
            )
        };
        let (row_starts, overhang) = Self::measure_rows(&lines);
        Self {
            lines,
            line_starts: Rc::new(starts),
            row_starts,
            overhang,
            line_height,
            shaping_run: None,
            shape_key: Some(key),
            tab_stop,
        }
    }

    fn measure_rows(lines: &[Rc<WrappedLine>]) -> (Rc<Vec<usize>>, Pixels) {
        let mut starts = Vec::with_capacity(lines.len() + 1);
        let mut rows = 0;
        let mut overhang = px(0.);
        starts.push(rows);
        for line in lines {
            rows += line.wrap_boundaries().len() + 1;
            starts.push(rows);
            overhang = overhang
                .max(line.font_size())
                .max(line.ascent() + line.descent());
        }
        (Rc::new(starts), overhang)
    }

    pub(super) fn visual_row_count(&self) -> usize {
        self.row_starts.last().copied().unwrap_or(0).max(1)
    }

    pub(super) fn line_for_row(&self, row: usize) -> usize {
        self.row_starts
            .partition_point(|start| *start <= row)
            .saturating_sub(1)
            .min(self.lines.len().saturating_sub(1))
    }

    pub(super) fn visible_lines(&self, top: Pixels, bottom: Pixels) -> Range<usize> {
        let first = ((top - self.overhang) / self.line_height).floor().max(0.) as usize;
        let last = ((bottom + self.overhang) / self.line_height).ceil().max(0.) as usize;
        self.line_for_row(first)..(self.line_for_row(last) + 1).min(self.lines.len())
    }

    fn line_for_offset(&self, offset: usize) -> (usize, usize) {
        let line_ix = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
            .min(self.lines.len().saturating_sub(1));
        let line_start = self.line_starts.get(line_ix).copied().unwrap_or(0);
        let local_offset = offset
            .saturating_sub(line_start)
            .min(self.lines.get(line_ix).map_or(0, |line| line.len()));
        (line_ix, local_offset)
    }

    pub(super) fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        self.position_for_caret(offset, CaretAffinity::Downstream)
    }

    pub(super) fn position_for_caret(
        &self,
        offset: usize,
        affinity: CaretAffinity,
    ) -> Point<Pixels> {
        let (line_ix, local_offset) = self.line_for_offset(offset);
        let rows_before = self.row_starts[line_ix];
        let local = self.lines[line_ix]
            .position_for_caret(
                TextCaret {
                    index: local_offset,
                    affinity,
                },
                self.line_height,
            )
            .unwrap_or_default();
        point(local.x, local.y + self.line_height * rows_before as f32)
    }

    pub(super) fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        self.caret_for_position(position).index
    }

    fn caret_for_position(&self, position: Point<Pixels>) -> TextCaret {
        if position.y < px(0.) {
            return 0.into();
        }

        let target_row = (position.y / self.line_height).floor() as usize;
        if target_row < self.visual_row_count() && !self.lines.is_empty() {
            let line_ix = self.line_for_row(target_row);
            let local_y = self.line_height * (target_row - self.row_starts[line_ix]) as f32;
            let mut caret = self.lines[line_ix]
                .closest_caret_for_position(point(position.x, local_y), self.line_height);
            caret.index += self.line_starts[line_ix];
            return caret;
        }

        (self.line_starts.last().copied().unwrap_or(0)
            + self.lines.last().map_or(0, |line| line.len()))
        .into()
    }

    fn row_range_for_offset(&self, offset: usize) -> Range<usize> {
        let (line_ix, local) = self.line_for_offset(offset);
        let line = &self.lines[line_ix];
        let position = line
            .position_for_index(local, self.line_height)
            .unwrap_or_default();
        let row = (position.y / self.line_height) as usize;
        let range = line.row_range(row).unwrap_or(0..0);
        let start = self.line_starts[line_ix];
        start + range.start..start + range.end
    }

    fn matches_content(&self, content: &str) -> bool {
        self.line_starts.last().copied().unwrap_or(0)
            + self.lines.last().map_or(0, |line| line.len())
            == content.len()
            && self
                .lines
                .iter()
                .zip(self.line_starts.iter())
                .all(|(line, start)| {
                    content.get(*start..start + line.len()) == Some(line.text.as_ref())
                })
    }

    fn refresh_for_edit(&mut self, content: &SharedString, shaper: &gpui::WindowTextSystem) {
        if self.matches_content(content) {
            return;
        }
        let Some(mut run) = self.shaping_run.clone() else {
            return;
        };
        let Some(first) = self.lines.first() else {
            return;
        };
        run.len = content.len();
        if let Ok(lines) = shaper.shape_text(
            content.clone(),
            first.font_size(),
            &[run],
            first.wrap_width,
            None,
        ) {
            self.lines = Rc::new(
                lines
                    .into_iter()
                    .map(|line| Rc::new(line.with_tab_stops(self.tab_stop)))
                    .collect(),
            );
            self.shape_key = None;
            self.line_starts = Rc::new(
                std::iter::once(0)
                    .chain(content.match_indices('\n').map(|(i, _)| i + 1))
                    .collect(),
            );
            (self.row_starts, self.overhang) = Self::measure_rows(&self.lines);
        }
    }

    fn bidi_horizontal_target(
        &self,
        content: &str,
        caret: TextCaret,
        right: bool,
        collapse: Option<Range<usize>>,
    ) -> Option<TextCaret> {
        if !self.lines.iter().any(|line| {
            line.runs()
                .iter()
                .any(|run| run.glyphs.iter().any(|glyph| glyph.is_rtl))
        }) || !self.matches_content(content)
        {
            return None;
        }

        let boundaries: Vec<_> = content
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(content.len()))
            .collect();
        let mut stops = Vec::new();
        let mut selection_edges = Vec::new();
        let mut y = px(0.);
        for (line, start) in self.lines.iter().zip(self.line_starts.iter()) {
            for (mut stop, mut position) in line.visual_carets(self.line_height) {
                stop.index += start;
                position.y += y;
                if boundaries.binary_search(&stop.index).is_ok() {
                    stops.push((stop, position));
                }
            }
            if let Some(range) = &collapse {
                let local = range.start.saturating_sub(*start).min(line.len())
                    ..range.end.saturating_sub(*start).min(line.len());
                for bounds in line.selection_bounds(local, self.line_height) {
                    selection_edges.push(point(
                        if right { bounds.right() } else { bounds.left() },
                        bounds.top() + y,
                    ));
                }
            }
            y += self.line_height * (line.wrap_boundaries().len() + 1);
        }
        let key = |position: Point<Pixels>| (position.y, position.x);
        let current = key(self.position_for_caret(caret.index, caret.affinity));
        let target_position = if collapse.is_some() && !selection_edges.is_empty() {
            selection_edges.into_iter().map(key).reduce(
                |a, b| {
                    if right { a.max(b) } else { a.min(b) }
                },
            )
        } else {
            stops
                .iter()
                .map(|(_, position)| key(*position))
                .filter(|position| {
                    if right {
                        *position > current
                    } else {
                        *position < current
                    }
                })
                .reduce(|a, b| if right { a.min(b) } else { a.max(b) })
        };
        Some(
            target_position
                .and_then(|position| {
                    stops
                        .into_iter()
                        .filter(|(_, p)| key(*p) == position)
                        .min_by_key(|(stop, _)| {
                            (
                                collapse.as_ref().is_some_and(|range| {
                                    stop.index < range.start || stop.index > range.end
                                }),
                                stop.index.abs_diff(caret.index),
                            )
                        })
                        .map(|(stop, _)| stop)
                })
                .unwrap_or(caret),
        )
    }

    fn bidi_deletion_target(
        &self,
        content: &str,
        caret: TextCaret,
        right: bool,
    ) -> Option<(Range<usize>, TextCaret)> {
        let (line_ix, index) = self.line_for_offset(caret.index);
        let line = self.lines.get(line_ix)?;
        let start = self.line_starts[line_ix];
        if !self.matches_content(content)
            || !line
                .runs()
                .iter()
                .any(|run| run.glyphs.iter().any(|glyph| glyph.is_rtl))
        {
            return None;
        }
        let key = |p: Point<Pixels>| (p.y, p.x);
        let position = line.position_for_caret(TextCaret { index, ..caret }, self.line_height)?;
        let edges = line.visual_carets(self.line_height);
        let cluster = edges
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|pair| {
                let left = key(pair[0].1).min(key(pair[1].1));
                let end = key(pair[0].1).max(key(pair[1].1));
                if right {
                    end > key(position)
                } else {
                    left < key(position)
                }
            })
            .reduce(|a, b| {
                let edge = |pair: &[(TextCaret, Point<Pixels>)]| {
                    if right {
                        key(pair[0].1).min(key(pair[1].1))
                    } else {
                        key(pair[0].1).max(key(pair[1].1))
                    }
                };
                if (right && edge(b) < edge(a)) || (!right && edge(b) > edge(a)) {
                    b
                } else {
                    a
                }
            });
        let Some(cluster) = cluster else {
            // Hard line breaks remain editable at the displayed line edges.
            let range = if !right && start > 0 && content.as_bytes().get(start - 1) == Some(&b'\n')
            {
                start - 1..start
            } else if right && content.as_bytes().get(start + line.len()) == Some(&b'\n') {
                start + line.len()..start + line.len() + 1
            } else {
                caret.index..caret.index
            };
            return Some((range.clone(), range.start.into()));
        };
        let rtl = cluster[0].1.x > cluster[1].1.x;
        let cluster_range = start + cluster[0].0.index..start + cluster[1].0.index;
        let mut graphemes = content.grapheme_indices(true).filter_map(|(index, text)| {
            let range = index..index + text.len();
            (range.start < cluster_range.end && cluster_range.start < range.end).then_some(range)
        });
        let range = if right != rtl {
            graphemes.next()
        } else {
            graphemes.next_back()
        }?;

        // Anchor to a surviving cluster, so deletion at a direction boundary
        // does not move the caret to the other visual representation of an index.
        let anchor = edges
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|pair| {
                let cluster = start + pair[0].0.index..start + pair[1].0.index;
                cluster.end <= range.start || cluster.start >= range.end
            })
            .flat_map(|pair| pair.iter())
            .min_by_key(|(stop, p)| {
                (
                    (p.y - position.y).abs(),
                    (p.x - position.x).abs(),
                    *stop != TextCaret { index, ..caret },
                )
            })
            .map(|(stop, _)| TextCaret {
                index: start + stop.index,
                ..*stop
            });
        let mut anchor = anchor.unwrap_or_else(|| range.start.into());
        if anchor.index >= range.end {
            anchor.index -= range.len();
        } else if anchor.index > range.start {
            anchor.index = range.start;
        }
        Some((range, anchor))
    }
}

pub struct InputState {
    pub(super) focus_handle: FocusHandle,
    pub(super) content: SharedString,
    pub(super) committed_content: SharedString,
    pub(super) placeholder: SharedString,
    pub(super) accessible_label: Option<SharedString>,
    pub(super) selected_range: Range<usize>,
    pub(super) selection_reversed: bool,
    pub(super) caret_affinity: CaretAffinity,
    horizontal_selection_anchor: Option<TextCaret>,
    pub(super) marked_range: Option<Range<usize>>,
    pub(super) preedit_cursor_hidden: bool,
    pub(super) last_layout: Option<TextLayout>,
    pub(super) last_bounds: Option<Bounds<Pixels>>,
    pub(super) last_viewport_bounds: Option<Bounds<Pixels>>,
    pub(super) is_selecting: bool,
    selection_pointer: Option<Point<Pixels>>,
    selection_line_anchor: Option<Range<usize>>,
    selection_scroll_task: Option<Task<()>>,
    pub(super) disabled: bool,
    pub(super) read_only: bool,
    history: super::history::History,
    pending_edit: Option<super::history::Snapshot>,
    pub(super) mode: InputMode,
    password_visible: bool,
    password_ime: bool,
    pub(super) tab_size: usize,
    input_purpose: gpui::TextInputPurpose,
    pub(super) autofill: Option<gpui::AutofillOptions>,
    input_action: Option<gpui::TextInputAction>,
    pub(super) placeholder_style: crate::style::ComputedStyle,
    pub(super) caret_style: crate::style::ComputedStyle,
    pub(super) selection_style: crate::style::ComputedStyle,
    pub(super) preferred_x: Option<Pixels>,
    pub(super) scroll_handle: ScrollHandle,
    pub(super) scrollbar_state: ScrollbarState,
    pub(super) scroll_cursor_pending: bool,
    pub(super) last_cursor_scroll: Option<super::element::CursorScrollSnapshot>,
    pub(super) single_line_scroll_offset: Pixels,
}

impl gpui::EventEmitter<InputEvent> for InputState {}
impl gpui::EventEmitter<InputActionEvent> for InputState {}

impl InputState {
    /// Offers this field to the system autofill service. Disabled by default.
    /// The name must be nonempty, stable, and unique within the window.
    /// Password hints expose the value only to the user's chosen autofill service.
    pub fn autofill(mut self, name: impl Into<SharedString>, hint: gpui::AutofillHint) -> Self {
        self.autofill = Some(gpui::AutofillOptions::new(name, hint));
        self
    }

    /// Changes or disables system autofill for this field.
    pub fn set_autofill(&mut self, options: Option<gpui::AutofillOptions>, cx: &mut Context<Self>) {
        self.autofill = options;
        cx.notify();
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            committed_content: "".into(),
            placeholder: "".into(),
            accessible_label: None,
            selected_range: 0..0,
            selection_reversed: false,
            caret_affinity: CaretAffinity::Downstream,
            horizontal_selection_anchor: None,
            marked_range: None,
            preedit_cursor_hidden: false,
            last_layout: None,
            last_bounds: None,
            last_viewport_bounds: None,
            is_selecting: false,
            selection_pointer: None,
            selection_line_anchor: None,
            selection_scroll_task: None,
            disabled: false,
            read_only: false,
            history: Default::default(),
            pending_edit: None,
            mode: InputMode::Text,
            password_visible: false,
            password_ime: false,
            tab_size: 4,
            input_purpose: gpui::TextInputPurpose::default(),
            autofill: None,
            input_action: None,
            placeholder_style: Default::default(),
            caret_style: Default::default(),
            selection_style: Default::default(),
            preferred_x: None,
            scroll_handle: ScrollHandle::new(),
            scrollbar_state: ScrollbarState::new(),
            scroll_cursor_pending: true,
            last_cursor_scroll: None,
            single_line_scroll_offset: px(0.),
        }
    }

    pub fn text(mut self) -> Self {
        self.mode = InputMode::Text;
        self
    }

    pub fn password(mut self) -> Self {
        self.mode = InputMode::Password;
        self
    }

    /// Enables multi-line editing with soft wrapping and newline insertion on Enter.
    pub fn multiline(mut self) -> Self {
        self.mode = InputMode::Multiline;
        self
    }

    /// Sets tab stops in space widths, measured using the input font. Defaults to four.
    /// Stops restart on each visual row; zero is treated as one. Text retains its tabs.
    pub fn tab_size(mut self, spaces: usize) -> Self {
        self.tab_size = spaces.max(1);
        self
    }

    /// Changes tab stops and refreshes wrapping and caret geometry.
    pub fn set_tab_size(&mut self, spaces: usize, cx: &mut Context<Self>) {
        self.tab_size = spaces.max(1);
        self.last_layout = None;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        cx.notify();
    }

    pub fn mode(mut self, mode: InputMode) -> Self {
        self.mode = mode;
        self
    }

    /// Requests a software keyboard layout without validating or filtering text.
    pub fn input_purpose(mut self, purpose: gpui::TextInputPurpose) -> Self {
        self.input_purpose = purpose;
        self
    }

    /// Configures the software keyboard action, emitted as [`InputActionEvent`].
    /// Physical Enter still submits single-line fields or inserts multiline newlines.
    pub fn input_action(mut self, action: gpui::TextInputAction) -> Self {
        self.input_action = Some(action);
        self
    }

    /// Changes the software keyboard action; `None` restores the mode's default.
    pub fn set_input_action(
        &mut self,
        action: Option<gpui::TextInputAction>,
        cx: &mut Context<Self>,
    ) {
        if self.input_action != action {
            self.input_action = action;
            cx.notify();
        }
    }

    /// Updates the keyboard hint while retaining the field's value and selection.
    pub fn set_input_purpose(&mut self, purpose: gpui::TextInputPurpose, cx: &mut Context<Self>) {
        if self.input_purpose != purpose {
            self.input_purpose = purpose;
            cx.notify();
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        self
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.stop_selection();
        if disabled {
            self.password_visible = false;
            self.last_layout = None;
            self.cancel_composition();
        }
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        cx.notify();
    }

    /// Prevents editing while retaining focus, selection, and copying.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        if self.read_only != read_only {
            self.read_only = read_only;
            if read_only {
                self.cancel_composition();
            }
            cx.notify();
        }
    }
    pub fn input_mode(&self) -> InputMode {
        self.mode
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Reveals the displayed password without changing its protected input mode.
    pub fn set_password_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        let visible = visible && self.mode == InputMode::Password && !self.disabled;
        if self.password_visible != visible {
            self.password_visible = visible;
            if self.is_password_masked() && !self.password_ime {
                self.cancel_composition();
            }
            self.last_layout = None;
            self.last_cursor_scroll = None;
            self.scroll_cursor_pending = true;
            cx.notify();
        }
    }
    /// Allows IME composition while a password is masked. Disabled by default.
    pub fn password_ime(mut self, enabled: bool) -> Self {
        self.password_ime = enabled;
        self
    }

    pub fn set_password_ime(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.password_ime != enabled {
            self.password_ime = enabled;
            if self.is_password_masked() && !enabled {
                self.cancel_composition();
            }
            cx.notify();
        }
    }

    pub fn is_password_visible(&self) -> bool {
        self.mode == InputMode::Password && self.password_visible
    }
    pub(super) fn is_password_masked(&self) -> bool {
        self.mode == InputMode::Password && !self.password_visible
    }

    pub fn set_mode(&mut self, mode: InputMode, cx: &mut Context<Self>) {
        cx.notify();
        self.cancel_composition();
        self.history.clear();
        self.stop_selection();
        self.mode = mode;
        self.password_visible = false;
        self.last_layout = None;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        self.single_line_scroll_offset = px(0.);
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        self.placeholder = placeholder.into();
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the accessible name. The placeholder is used when no name is supplied.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Updates the accessible name without changing the field's content.
    pub fn set_aria_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.accessible_label = Some(label.into());
        cx.notify();
    }

    pub fn initial_value(mut self, value: impl Into<SharedString>) -> Self {
        self.content = value.into();
        self.committed_content = self.content.clone();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self
    }

    /// Whether the input method currently owns a pre-edit range.
    pub fn is_composing(&self) -> bool {
        self.marked_range.is_some()
    }

    pub fn value(&self) -> SharedString {
        self.content.clone()
    }

    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.history.clear();
        self.pending_edit = None;
        self.stop_selection();
        self.content = value.into();
        self.committed_content = self.content.clone();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self.marked_range = None;
        self.scroll_cursor_pending = true;
        cx.emit(InputEvent::Change(self.content.clone()));
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_value("", cx);
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        cx.emit(InputEvent::Submit(self.content.clone()));
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(false, false, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(true, false, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1., false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1., false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(false, true, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(true, true, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1., true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1., true, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let offset = if self.mode == InputMode::Multiline {
            self.last_layout
                .as_ref()
                .map(|layout| layout.row_range_for_offset(self.cursor_offset()).start)
                .unwrap_or(0)
        } else {
            0
        };
        self.move_to(offset, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let offset = if self.mode == InputMode::Multiline {
            self.last_layout
                .as_ref()
                .map(|layout| layout.row_range_for_offset(self.cursor_offset()).end)
                .unwrap_or(self.content.len())
        } else {
            self.content.len()
        };
        self.move_to(offset, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_horizontal(false, window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_horizontal(true, window, cx);
    }

    fn delete_horizontal(&mut self, right: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only {
            return;
        }
        self.remember_edit();
        let mut anchor = None;
        if self.selected_range.is_empty() {
            if self.marked_range.is_none()
                && !self.is_password_masked()
                && let Some(layout) = self.last_layout.as_mut()
                && layout.lines.iter().any(|line| {
                    line.runs()
                        .iter()
                        .any(|run| run.glyphs.iter().any(|glyph| glyph.is_rtl))
                })
            {
                layout.refresh_for_edit(&self.content, window.text_system());
            }
            let caret = TextCaret {
                index: self.cursor_offset(),
                affinity: self.caret_affinity,
            };
            let visual = self
                .marked_range
                .is_none()
                .then(|| {
                    self.last_layout
                        .as_ref()?
                        .bidi_deletion_target(&self.content, caret, right)
                })
                .flatten();
            let range = if let Some((range, target)) = visual {
                anchor = Some(target);
                range
            } else if right {
                caret.index..self.next_boundary(caret.index)
            } else {
                self.previous_boundary(caret.index)..caret.index
            };
            if range.is_empty() {
                self.pending_edit = None;
                window.play_system_bell();
                return;
            }
            self.selected_range = range;
            self.selection_reversed = false;
        }
        self.replace_text_in_range(None, "", window, cx);
        if let Some(anchor) = anchor {
            self.selected_range = anchor.index..anchor.index;
            self.caret_affinity = anchor.affinity;
        }
    }

    fn insert_newline(&mut self, _: &InsertNewline, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only || self.mode != InputMode::Multiline {
            return;
        }
        self.replace_text_in_range(None, "\n", window, cx);
        cx.stop_propagation();
    }

    pub(super) fn focus_at(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus_handle, cx);
        self.stop_selection();
        let caret = self.caret_for_mouse_position(position);
        self.move_to(caret.index, cx);
        self.caret_affinity = caret.affinity;
        self.scroll_cursor_pending = false;
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus_handle, cx);
        self.stop_selection();
        self.is_selecting = true;

        let caret = self.caret_for_mouse_position(event.position);
        let offset = caret.index;
        if event.click_count >= 2 {
            let range = self.line_range_at(offset);
            self.selection_line_anchor = Some(range.clone());
            self.preferred_x = None;
            self.selected_range = range;
            self.selection_reversed = false;
            self.caret_affinity = CaretAffinity::Downstream;
            self.horizontal_selection_anchor = None;
            self.scroll_cursor_pending = true;
            cx.notify();
        } else if event.modifiers.shift {
            self.preferred_x = None;
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx)
        }
        if event.click_count < 2 {
            self.caret_affinity = caret.affinity;
        }
    }

    fn stop_selection(&mut self) {
        self.is_selecting = false;
        self.selection_pointer = None;
        self.selection_line_anchor = None;
        self.selection_scroll_task = None;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || !event.dragging() || !self.focus_handle.is_focused(window) {
            self.stop_selection();
            return;
        }
        if !self.is_selecting {
            return;
        }
        self.preferred_x = None;
        self.selection_pointer = Some(event.position);
        self.select_at_drag_pointer(cx);
        if self.selection_scroll_delta() == px(0.) {
            self.selection_scroll_task = None;
        } else if self.selection_scroll_task.is_none() {
            self.selection_scroll_task = Some(cx.spawn_in(window, async move |input, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    let keep_scrolling = input
                        .update_in(cx, |input, window, cx| {
                            if input.disabled
                                || !input.is_selecting
                                || !input.focus_handle.is_focused(window)
                            {
                                input.stop_selection();
                                return false;
                            }
                            if !input.scroll_selection(cx) {
                                input.selection_scroll_task = None;
                                return false;
                            }
                            true
                        })
                        .unwrap_or(false);
                    if !keep_scrolling {
                        break;
                    }
                }
            }));
        }
    }

    fn select_at_drag_pointer(&mut self, cx: &mut Context<Self>) {
        let Some(mut position) = self.selection_pointer else {
            return;
        };
        if self.mode != InputMode::Multiline
            && let Some(viewport) = self.last_viewport_bounds
        {
            position.x = position.x.clamp(viewport.left(), viewport.right());
        }
        let caret = self.caret_for_mouse_position(position);
        let offset = caret.index;
        if let Some(anchor) = &self.selection_line_anchor {
            let range = self.line_range_at(offset);
            self.selection_reversed = range.start < anchor.start;
            self.selected_range = range.start.min(anchor.start)..range.end.max(anchor.end);
            self.scroll_cursor_pending = true;
            cx.notify();
        } else {
            self.select_to(offset, cx);
            self.caret_affinity = caret.affinity;
        }
    }

    fn line_range_at(&self, offset: usize) -> Range<usize> {
        if self.mode != InputMode::Multiline {
            return 0..self.content.len();
        }
        let mut offset = offset.min(self.content.len());
        while !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        let start = self.content[..offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let end = self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |index| offset + index);
        start..end
    }

    fn selection_scroll_delta(&self) -> Pixels {
        if self.mode == InputMode::Multiline {
            return px(0.);
        }
        let (Some(position), Some(viewport)) = (self.selection_pointer, self.last_viewport_bounds)
        else {
            return px(0.);
        };
        // A small edge zone also scrolls when the pointer is held just inside the field.
        let edge = px(8.).min(viewport.size.width / 2.);
        let distance = if position.x < viewport.left() + edge {
            viewport.left() + edge - position.x
        } else if position.x > viewport.right() - edge {
            viewport.right() - edge - position.x
        } else {
            return px(0.);
        };
        (distance * 0.35).clamp(px(-24.), px(24.))
    }

    fn scroll_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let (Some(bounds), Some(viewport)) = (self.last_bounds, self.last_viewport_bounds) else {
            return false;
        };
        let max_scroll = (bounds.size.width - viewport.size.width).max(px(0.));
        let offset = (self.single_line_scroll_offset + self.selection_scroll_delta())
            .clamp(-max_scroll, px(0.));
        let change = offset - self.single_line_scroll_offset;
        if change == px(0.) {
            return false;
        }
        self.single_line_scroll_offset = offset;
        // Keep hit testing synchronized even if multiple timer ticks precede paint.
        self.last_bounds.as_mut().unwrap().origin.x += change;
        self.select_at_drag_pointer(cx);
        self.scroll_cursor_pending = false;
        true
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.is_password_masked() {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only || self.mode == InputMode::Password {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        cx.notify()
    }

    fn move_horizontal(&mut self, right: bool, selecting: bool, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let caret = TextCaret {
            index: self.cursor_offset(),
            affinity: self.caret_affinity,
        };
        let anchor = self.horizontal_selection_anchor.unwrap_or_else(|| {
            if self.selected_range.is_empty() {
                caret
            } else {
                (if self.selection_reversed {
                    self.selected_range.end
                } else {
                    self.selected_range.start
                })
                .into()
            }
        });
        let collapse =
            (!selecting && !self.selected_range.is_empty()).then(|| self.selected_range.clone());
        let visual_target = self.last_layout.as_ref().and_then(|layout| {
            (!self.is_password_masked())
                .then(|| {
                    layout.bidi_horizontal_target(&self.content, caret, right, collapse.clone())
                })
                .flatten()
        });
        let mut target = visual_target.unwrap_or_else(|| {
            if let Some(range) = collapse {
                (if right { range.end } else { range.start }).into()
            } else if right {
                self.next_boundary(caret.index).into()
            } else {
                self.previous_boundary(caret.index).into()
            }
        });
        if selecting {
            if visual_target.is_some()
                && let Some(layout) = &self.last_layout
                && layout.position_for_caret(target.index, target.affinity)
                    == layout.position_for_caret(anchor.index, anchor.affinity)
            {
                target = anchor;
            }
            self.select_to(target.index, cx);
            self.horizontal_selection_anchor = Some(anchor);
        } else {
            self.move_to(target.index, cx);
        }
        self.caret_affinity = target.affinity;
        self.preferred_x = None;
    }

    fn move_vertical(&mut self, rows: f32, selecting: bool, cx: &mut Context<Self>) {
        self.horizontal_selection_anchor = None;
        if self.disabled || self.mode != InputMode::Multiline {
            return;
        }
        let Some(layout) = self.last_layout.as_ref() else {
            return;
        };
        let cursor = self.cursor_offset();
        let position = layout.position_for_caret(cursor, self.caret_affinity);
        let preferred_x = self.preferred_x.unwrap_or(position.x);
        let target =
            layout.caret_for_position(point(preferred_x, position.y + layout.line_height * rows));
        self.preferred_x = Some(preferred_x);
        if selecting {
            self.select_to(target.index, cx);
        } else {
            self.selected_range = target.index..target.index;
            self.selection_reversed = false;
            self.caret_affinity = CaretAffinity::Downstream;
            self.horizontal_selection_anchor = None;
            self.scroll_cursor_pending = true;
            cx.notify();
        }
        self.caret_affinity = target.affinity;
    }

    pub(super) fn display_offset(&self, offset: usize) -> usize {
        if self.is_password_masked() {
            self.content
                .grapheme_indices(true)
                .take_while(|(start, _)| *start < offset)
                .count()
                * '•'.len_utf8()
        } else {
            offset
        }
    }
    pub(super) fn source_offset(&self, offset: usize) -> usize {
        if self.is_password_masked() {
            self.content
                .grapheme_indices(true)
                .nth(offset / '•'.len_utf8())
                .map_or(self.content.len(), |(offset, _)| offset)
        } else {
            offset
        }
    }
    pub(super) fn password_mask(&self) -> SharedString {
        "•".repeat(self.content.graphemes(true).count()).into()
    }

    pub(super) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    #[cfg(test)]
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        self.caret_for_mouse_position(position).index
    }

    fn caret_for_mouse_position(&self, position: Point<Pixels>) -> TextCaret {
        if self.content.is_empty() {
            return 0.into();
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0.into();
        };
        if self.mode != InputMode::Multiline {
            let caret = line.caret_for_position(point(position.x - bounds.left(), px(0.)));
            return TextCaret {
                index: self.source_offset(caret.index),
                ..caret
            };
        }
        if position.y < bounds.top() {
            return 0.into();
        }
        if position.y > bounds.bottom() {
            return self.content.len().into();
        }
        line.caret_for_position(point(position.x - bounds.left(), position.y - bounds.top()))
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.scroll_cursor_pending = true;
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn normalize_inserted_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if !text.contains(['\r', '\n']) {
            return Cow::Borrowed(text);
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.mode == InputMode::Multiline {
            Cow::Owned(normalized)
        } else {
            Cow::Owned(normalized.replace('\n', " "))
        }
    }

    fn snapshot(&self) -> super::history::Snapshot {
        super::history::Snapshot {
            content: self.content.clone(),
            selection: self.selected_range.clone(),
            reversed: self.selection_reversed,
            affinity: self.caret_affinity,
        }
    }
    fn remember_edit(&mut self) {
        if self.pending_edit.is_none() {
            self.pending_edit = Some(self.snapshot());
        }
    }
    fn restore_snapshot(&mut self, snapshot: super::history::Snapshot) {
        self.content = snapshot.content;
        self.selected_range = snapshot.selection;
        self.selection_reversed = snapshot.reversed;
        self.caret_affinity = snapshot.affinity;
        self.marked_range = None;
        self.preedit_cursor_hidden = false;
        self.horizontal_selection_anchor = None;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        self.stop_selection();
    }
    fn cancel_composition(&mut self) {
        if self.marked_range.take().is_some() {
            if let Some(snapshot) = self.pending_edit.take() {
                self.restore_snapshot(snapshot);
            }
        }
        self.pending_edit = None;
    }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.travel_history(false, cx);
    }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.travel_history(true, cx);
    }
    fn travel_history(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self.disabled || self.read_only || self.is_composing() {
            return;
        }
        if let Some(snapshot) = self.history.travel(self.snapshot(), redo) {
            self.restore_snapshot(snapshot);
            self.committed_content = self.content.clone();
            cx.emit(InputEvent::Change(self.content.clone()));
            cx.notify();
        }
    }

    fn emit_committed_change(&mut self, cx: &mut Context<Self>) {
        if self.content == self.committed_content {
            self.pending_edit = None;
            return;
        }
        if let Some(snapshot) = self.pending_edit.take() {
            if self.mode != InputMode::Password {
                self.history.commit(snapshot);
            }
        }
        self.committed_content = self.content.clone();
        cx.emit(InputEvent::Change(self.content.clone()));
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.history.clear();
        self.pending_edit = None;
        self.content = "".into();
        self.committed_content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.last_viewport_bounds = None;
        self.last_cursor_scroll = None;
        self.stop_selection();
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        self.single_line_scroll_offset = px(0.);
        cx.emit(InputEvent::Change(self.content.clone()));
        cx.notify();
    }
}

impl EntityInputHandler for InputState {
    fn text_input_action(
        &self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui::TextInputAction> {
        self.input_action
    }

    fn perform_text_input_action(
        &mut self,
        action: gpui::TextInputAction,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.disabled || self.read_only || self.input_action != Some(action) {
            return false;
        }
        cx.emit(InputActionEvent {
            action,
            text: self.content.clone(),
        });
        true
    }

    fn text_input_purpose(&self, _: &mut Window, _: &mut Context<Self>) -> gpui::TextInputPurpose {
        self.input_purpose
    }

    fn text_input_mode(&self, _: &mut Window, _: &mut Context<Self>) -> gpui::TextInputMode {
        match self.mode {
            InputMode::Text => gpui::TextInputMode::SingleLine,
            InputMode::Multiline => gpui::TextInputMode::Multiline,
            InputMode::Password => gpui::TextInputMode::Password,
        }
    }

    fn ime_enabled(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        !self.disabled && !self.read_only && (!self.is_password_masked() || self.password_ime)
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        !self.disabled && !self.read_only
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.stop_selection();
        let anchor = self.offset_from_utf16(range.start);
        let head = self.offset_from_utf16(range.end);
        self.selected_range = anchor.min(head)..anchor.max(head);
        self.selection_reversed = head < anchor;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        cx.notify();
    }

    fn element_bounds(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(if self.mode == InputMode::Multiline {
            self.scroll_handle.bounds()
        } else {
            self.last_viewport_bounds.unwrap_or(bounds)
        })
    }

    fn scroll_text_input(
        &mut self,
        delta: Point<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.disabled || !f32::from(delta.x).is_finite() || !f32::from(delta.y).is_finite() {
            return false;
        }
        self.scroll_cursor_pending = false;
        if self.mode == InputMode::Multiline {
            let mut offset = self.scroll_handle.offset();
            let next = (offset.y + delta.y).clamp(-self.scroll_handle.max_offset().y, px(0.));
            if next == offset.y {
                return false;
            }
            offset.y = next;
            self.scroll_handle.set_offset(offset);
        } else {
            let (Some(bounds), Some(viewport)) = (self.last_bounds, self.last_viewport_bounds)
            else {
                return false;
            };
            let max_scroll = (bounds.size.width - viewport.size.width).max(px(0.));
            let next = (self.single_line_scroll_offset + delta.x).clamp(-max_scroll, px(0.));
            if next == self.single_line_scroll_offset {
                return false;
            }
            self.single_line_scroll_offset = next;
        }
        cx.notify();
        true
    }

    fn surrounding_text(
        &mut self,
        max_bytes: usize,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<gpui::SurroundingText> {
        if self.disabled || self.mode == InputMode::Password {
            return None;
        }
        let anchor = if self.selection_reversed {
            self.selected_range.end
        } else {
            self.selected_range.start
        };
        gpui::SurroundingText::from_utf8(
            &self.content,
            self.cursor_offset(),
            anchor,
            self.marked_range.clone(),
            max_bytes,
        )
    }

    fn delete_surrounding_text(
        &mut self,
        before_utf16: usize,
        after_utf16: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.disabled
            || self.read_only
            || self.mode == InputMode::Password
            || self.marked_range.is_some()
        {
            return false;
        }
        let selection = self.range_to_utf16(&self.selected_range);
        let Some(start) = selection.start.checked_sub(before_utf16) else {
            return false;
        };
        let Some(end) = selection.end.checked_add(after_utf16) else {
            return false;
        };
        let start_byte = self.offset_from_utf16(start);
        let end_byte = self.offset_from_utf16(end);
        if self.offset_to_utf16(start_byte) != start || self.offset_to_utf16(end_byte) != end {
            return false;
        }
        let selected_len = self.selected_range.len();
        self.remember_edit();
        self.stop_selection();
        self.content = format!(
            "{}{}{}",
            &self.content[..start_byte],
            &self.content[self.selected_range.clone()],
            &self.content[end_byte..]
        )
        .into();
        self.selected_range = start_byte..start_byte + selected_len;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        self.emit_committed_change(cx);
        cx.notify();
        true
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let had_marked_text = self.marked_range.take().is_some();
        if had_marked_text {
            self.emit_committed_change(cx);
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }
        self.remember_edit();
        self.stop_selection();
        let new_text = self.normalize_inserted_text(new_text);
        let new_text = new_text.as_ref();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self.marked_range.take();
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        self.emit_committed_change(cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.ime_enabled(_window, cx) {
            return;
        }
        self.remember_edit();
        self.stop_selection();
        let selected_offsets = new_selected_range_utf16.map(|range| {
            let offset = |utf16| {
                let end = preedit_offset_from_utf16(new_text, utf16);
                self.normalize_inserted_text(&new_text[..end]).len()
            };
            offset(range.start)..offset(range.end)
        });
        let new_text = self.normalize_inserted_text(new_text);
        let new_text = new_text.as_ref();
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = selected_offsets
            .map(|new_range| {
                new_range.start.min(new_range.end) + range.start
                    ..new_range.end.max(new_range.start) + range.start
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.horizontal_selection_anchor = None;
        self.preedit_cursor_hidden = false;
        self.preferred_x = None;
        self.scroll_cursor_pending = true;
        if self.marked_range.is_none() && self.content == self.committed_content {
            self.pending_edit = None;
        }

        cx.notify();
    }

    fn replace_and_mark_text_with_selection(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        selection: PreeditSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.ime_enabled(window, cx) {
            return;
        }
        self.replace_and_mark_text_in_range(range_utf16, new_text, selection.range(), window, cx);
        self.selection_reversed =
            matches!(selection, PreeditSelection::Range { anchor, head } if head < anchor);
        self.preedit_cursor_hidden = selection == PreeditSelection::Hidden;
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let bounds = if self.mode == InputMode::Multiline {
            bounds
        } else {
            self.last_bounds.unwrap_or(bounds)
        };
        let logical = self.range_from_utf16(&range_utf16);
        let range = self.display_offset(logical.start)..self.display_offset(logical.end);
        if range.is_empty() {
            let affinity = if logical.start == self.cursor_offset() {
                self.caret_affinity
            } else {
                CaretAffinity::Downstream
            };
            let position = last_layout.position_for_caret(range.start, affinity);
            return Some(Bounds::new(
                bounds.origin + position,
                gpui::size(px(0.), last_layout.line_height),
            ));
        }
        let mut selected: Option<Bounds<Pixels>> = None;
        let mut rows_before = 0;
        for (ix, line) in last_layout.lines.iter().enumerate() {
            let start = last_layout.line_starts[ix];
            let local =
                range.start.saturating_sub(start)..range.end.saturating_sub(start).min(line.len());
            for mut rect in line.selection_bounds(local, last_layout.line_height) {
                rect.origin += bounds.origin + point(px(0.), last_layout.line_height * rows_before);
                selected = Some(selected.map_or(rect, |old| old.union(&rect)));
            }
            rows_before += line.wrap_boundaries().len() + 1;
        }
        selected.or_else(|| {
            Some(Bounds::new(
                bounds.origin + last_layout.position_for_offset(range.start),
                gpui::size(px(0.), last_layout.line_height),
            ))
        })
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line_point = bounds.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        let utf8_index = self.source_offset(last_layout.offset_for_position(line_point));
        Some(self.offset_to_utf16(utf8_index))
    }
}

impl Render for InputState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let multiline = self.mode == InputMode::Multiline;
        let scroll_handle = self.scroll_handle.clone();
        let input = cx.weak_entity();
        let accessibility = window
            .is_a11y_active()
            .then(|| super::accessibility::InputAccessibility::new(self));
        let geometry = accessibility.as_ref().map(|state| state.geometry.clone());
        let element = div()
            .on_paint_before_children(move |_, _, window, _| {
                let moving_input = input.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture
                        && let Some(input) = moving_input.upgrade()
                        && input.read(cx).is_selecting
                    {
                        input.update(cx, |input, cx| input.on_mouse_move(event, window, cx));
                        cx.stop_propagation();
                    }
                });
                let input = input.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture
                        && event.button == MouseButton::Left
                        && let Some(input) = input.upgrade()
                        && input.read(cx).is_selecting
                    {
                        input.update(cx, |input, _| input.stop_selection());
                    }
                });
            })
            .id(("fusion-text-input", cx.entity_id()))
            .flex()
            .w_full()
            .min_w_0()
            .when(multiline, |this| {
                this.h_full()
                    .flex_col()
                    .overflow_scroll()
                    .track_scroll(&scroll_handle)
            })
            .when(!multiline, |this| this.overflow_hidden())
            .key_context(if multiline {
                "InputState multiline"
            } else {
                "InputState"
            })
            .track_focus(&self.focus_handle(cx))
            .cursor(if self.disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .on_scroll_wheel(cx.listener(|this, _, window, cx| {
                if this.mode == InputMode::Multiline
                    && !this.disabled
                    && this.focus_handle.is_focused(window)
                {
                    this.scroll_cursor_pending = false;
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::insert_newline))
            .on_action(cx.listener(Self::submit))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .when(multiline, |this| this.flex_none())
                    .child(TextElement {
                        input: cx.entity(),
                        accessibility: geometry,
                    }),
            );
        match accessibility {
            Some(accessibility) => accessibility.decorate(element, cx),
            None => element,
        }
    }
}

impl Focusable for InputState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
