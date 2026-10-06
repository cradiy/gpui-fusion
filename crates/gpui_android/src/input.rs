use crate::window::AndroidWindow;
use gpui::{
    Bounds, Pixels, PlatformInputHandler, TextInputAction, TextInputMode, TextInputPurpose,
};
use std::ops::Range;

pub(crate) struct InputState {
    pub epoch: u64,
    pub mode: TextInputMode,
    pub purpose: TextInputPurpose,
    pub action: Option<TextInputAction>,
    pub text: Option<String>,
    pub offset: usize,
    pub anchor: usize,
    pub head: usize,
    pub marked: Option<Range<usize>>,
    pub hit: bool,
    pub caret_bounds: Option<Bounds<Pixels>>,
    pub anchor_bounds: Option<Bounds<Pixels>>,
    pub head_bounds: Option<Bounds<Pixels>>,
    pub editor_bounds: Option<Bounds<Pixels>>,
}

impl AndroidWindow {
    pub(crate) fn with_input<T>(
        &self,
        f: impl FnOnce(&mut PlatformInputHandler, u64) -> T,
    ) -> Option<T> {
        let mut handler = self.handler.borrow_mut().take();
        let focus = handler.as_mut().and_then(|handler| {
            handler
                .query_accepts_text_input()
                .then(|| handler.input_focus_id())
                .flatten()
        });
        let mode = handler
            .as_mut()
            .map(|handler| handler.text_input_mode())
            .unwrap_or_default();
        let focus_changed = self.input_focus.replace(focus) != focus;
        let mode_changed = self.input_mode.replace(mode) != mode;
        let purpose = if mode == TextInputMode::SingleLine {
            handler
                .as_mut()
                .map(|handler| handler.text_input_purpose())
                .unwrap_or_default()
        } else {
            TextInputPurpose::Text
        };
        let purpose_changed = self.input_purpose.replace(purpose) != purpose;
        let action = handler
            .as_mut()
            .and_then(|handler| handler.text_input_action())
            .or_else(|| (mode != TextInputMode::Multiline).then_some(TextInputAction::Done));
        let action_changed = self.input_action.replace(action) != action;
        if (mode_changed || purpose_changed || action_changed) && !focus_changed && focus.is_some()
        {
            if let Some(handler) = handler.as_mut() {
                handler.unmark_text();
            }
        }
        if focus_changed || mode_changed || purpose_changed || action_changed {
            self.input_epoch.set(self.input_epoch.get().wrapping_add(1));
        }
        let result = if focus.is_some() {
            handler
                .as_mut()
                .map(|handler| f(handler, self.input_epoch.get()))
        } else {
            None
        };
        *self.handler.borrow_mut() = handler;
        result
    }

    pub(crate) fn input_state(&self) -> Option<InputState> {
        self.with_input(|handler, epoch| {
            let mode = self.input_mode.get();
            let selection = handler.selected_text_range(false)?;
            let marked = handler.marked_text_range();
            let start = selection
                .range
                .start
                .min(marked.as_ref().map_or(usize::MAX, |r| r.start));
            let end = selection
                .range
                .end
                .max(marked.as_ref().map_or(0, |r| r.end));
            let mut offset = start;
            // A handler may withhold surrounding text for sensitive fields.
            let text = if mode != TextInputMode::Password
                && end.saturating_sub(start) <= 4096
                && handler.surrounding_text(4096).is_some()
            {
                let range = start.saturating_sub(1024)..end.saturating_add(1024);
                let mut actual = None;
                let text = handler.text_for_range(range.clone(), &mut actual);
                offset = actual.unwrap_or(range).start;
                text
            } else {
                None
            };
            let (anchor, head) = if selection.reversed {
                (selection.range.end, selection.range.start)
            } else {
                (selection.range.start, selection.range.end)
            };
            let editor_bounds = handler.element_bounds();
            let head_bounds = handler.bounds_for_range(head..head);
            let anchor_bounds = if anchor == head {
                head_bounds
            } else {
                handler.bounds_for_range(anchor..anchor)
            };
            let caret_bounds = (anchor == head).then_some(head_bounds).flatten();
            Some(InputState {
                epoch,
                mode,
                purpose: self.input_purpose.get(),
                action: self.input_action.get(),
                text,
                offset,
                anchor,
                head,
                marked,
                hit: editor_bounds.is_some_and(|bounds| bounds.contains(&self.pointer.get())),
                caret_bounds,
                anchor_bounds,
                head_bounds,
                editor_bounds,
            })
        })
        .flatten()
    }

    pub(crate) fn input_index(&self, epoch: u64, x: f32, y: f32) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        self.with_input(|handler, current| {
            if epoch != current {
                return None;
            }
            handler.character_index_for_point(gpui::point(gpui::px(x), gpui::px(y)))
        })
        .flatten()
    }

    pub(crate) fn scroll_input(&self, epoch: u64, dx: f32, dy: f32) -> bool {
        if !dx.is_finite() || !dy.is_finite() {
            return false;
        }
        let changed = self
            .with_input(|handler, current| {
                epoch == current
                    && handler.scroll_text_input(gpui::point(gpui::px(dx), gpui::px(dy)))
            })
            .unwrap_or(false);
        if changed {
            self.input_dirty.set(true);
        }
        changed
    }

    pub(crate) fn perform_input_action(&self, epoch: u64, code: i32) -> bool {
        let handled = self
            .with_input(|handler, current| {
                let action = self.input_action.get();
                if epoch != current || code != action_code(action) {
                    return false;
                }
                action.is_some_and(|action| handler.perform_text_input_action(action))
            })
            .unwrap_or(false);
        self.input_dirty.set(true);
        handled
    }

    pub(crate) fn edit(
        &self,
        epoch: u64,
        operation: i32,
        text: &str,
        a: i32,
        b: i32,
        cursor: i32,
    ) -> bool {
        let edited = self
            .with_input(|handler, current| {
                if epoch != current {
                    return false;
                }
                let Some(selection) = handler.selected_text_range(false) else {
                    return false;
                };
                match operation {
                    0 | 1 | 7 => {
                        let range = if operation == 7 {
                            if a < 0 || b < 0 {
                                return false;
                            }
                            let range = a.min(b) as usize..a.max(b) as usize;
                            let mut actual = None;
                            if handler.text_for_range(range.clone(), &mut actual).is_none() {
                                return false;
                            }
                            actual.unwrap_or(range)
                        } else {
                            handler.marked_text_range().unwrap_or(selection.range)
                        };
                        if operation == 1 && !text.is_empty() {
                            let end = text.encode_utf16().count();
                            handler.replace_and_mark_text_in_range(
                                Some(range.clone()),
                                text,
                                Some(end..end),
                            );
                        } else {
                            handler.replace_text_in_range(Some(range.clone()), text);
                        }
                        let end = handler
                            .selected_text_range(false)
                            .map_or(range.start, |s| s.range.end);
                        let cursor = if operation == 7 { cursor } else { a };
                        let cursor = if cursor > 0 {
                            end.saturating_add(cursor as usize - 1)
                        } else {
                            range.start.saturating_sub(cursor.unsigned_abs() as usize)
                        };
                        handler.set_selected_text_range(cursor..cursor);
                    }
                    2 => handler.unmark_text(),
                    3 if a >= 0 && b >= 0 => {
                        handler.set_selected_text_range(a as usize..b as usize)
                    }
                    4 if a >= 0 && b >= 0 => {
                        let range = a.min(b) as usize..a.max(b) as usize;
                        let mut actual = None;
                        let Some(text) = handler.text_for_range(range.clone(), &mut actual) else {
                            return false;
                        };
                        let range = actual.unwrap_or(range);
                        if range.is_empty() {
                            handler.unmark_text();
                        } else {
                            handler.replace_and_mark_text_in_range(Some(range), &text, None);
                        }
                        let range = selection.range;
                        handler.set_selected_text_range(if selection.reversed {
                            range.end..range.start
                        } else {
                            range
                        });
                    }
                    5 | 6 if a >= 0 && b >= 0 => {
                        return delete_surrounding(handler, a as usize, b as usize, operation == 6);
                    }
                    _ => return false,
                }
                true
            })
            .unwrap_or(false);
        self.input_dirty.set(true);
        edited
    }
}

pub(crate) fn action_code(action: Option<TextInputAction>) -> i32 {
    match action {
        None => 1,
        Some(TextInputAction::Go) => 2,
        Some(TextInputAction::Search) => 3,
        Some(TextInputAction::Send) => 4,
        Some(TextInputAction::Next) => 5,
        Some(TextInputAction::Done) => 6,
        Some(TextInputAction::Previous) => 7,
    }
}

fn delete_surrounding(
    handler: &mut PlatformInputHandler,
    before: usize,
    after: usize,
    codepoints: bool,
) -> bool {
    let Some(selection) = handler.selected_text_range(false) else {
        return false;
    };
    let marked = handler.marked_text_range();
    let start = selection
        .range
        .start
        .min(marked.as_ref().map_or(usize::MAX, |r| r.start));
    let end = selection
        .range
        .end
        .max(marked.as_ref().map_or(0, |r| r.end));
    let width = if codepoints { 2 } else { 1 };
    let mut left_range = None;
    let mut right_range = None;
    let Some(left) = handler.text_for_range(
        start.saturating_sub(before.saturating_mul(width))..start,
        &mut left_range,
    ) else {
        return false;
    };
    let Some(right) = handler.text_for_range(
        end..end.saturating_add(after.saturating_mul(width)),
        &mut right_range,
    ) else {
        return false;
    };
    let before = if codepoints {
        left.chars().rev().take(before).map(char::len_utf16).sum()
    } else {
        left.encode_utf16().count()
    };
    let after = if codepoints {
        right.chars().take(after).map(char::len_utf16).sum()
    } else {
        right.encode_utf16().count()
    };
    if before == 0 && after == 0 {
        return true;
    }
    if before > start {
        return false;
    }
    let Some(retained) = handler.text_for_range(start..end, &mut None) else {
        return false;
    };
    let replacement = Some(start - before..end + after);
    if let Some(marked) = marked {
        // Both deletions belong to the active composition. Keep provisional text
        // out of committed-change notifications, then restore its precise span.
        handler.replace_and_mark_text_in_range(replacement, &retained, None);
        let range = marked.start - before..marked.end - before;
        if let Some(text) = handler.text_for_range(range.clone(), &mut None) {
            handler.replace_and_mark_text_in_range(Some(range), &text, None);
        }
    } else {
        handler.replace_text_in_range(replacement, &retained);
    }
    let range = selection.range.start - before..selection.range.end - before;
    handler.set_selected_text_range(if selection.reversed {
        range.end..range.start
    } else {
        range
    });
    true
}
