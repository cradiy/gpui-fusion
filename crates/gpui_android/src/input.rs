use crate::window::AndroidWindow;
use gpui::PlatformInputHandler;
use std::ops::Range;

pub(crate) struct InputState {
    pub epoch: u64,
    pub text: Option<String>,
    pub offset: usize,
    pub anchor: usize,
    pub head: usize,
    pub marked: Option<Range<usize>>,
    pub hit: bool,
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
        if self.input_focus.replace(focus) != focus {
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
            let text =
                if end.saturating_sub(start) <= 4096 && handler.surrounding_text(4096).is_some() {
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
            Some(InputState {
                epoch,
                text,
                offset,
                anchor,
                head,
                marked,
                hit: handler
                    .element_bounds()
                    .is_some_and(|bounds| bounds.contains(&self.pointer.get())),
            })
        })
        .flatten()
    }

    pub(crate) fn edit(&self, epoch: u64, operation: i32, text: &str, a: i32, b: i32) -> bool {
        let edited = self
            .with_input(|handler, current| {
                if epoch != current {
                    return false;
                }
                let Some(selection) = handler.selected_text_range(false) else {
                    return false;
                };
                match operation {
                    0 | 1 => {
                        let range = handler.marked_text_range().unwrap_or(selection.range);
                        if operation == 1 {
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
                        let cursor = if a > 0 {
                            end.saturating_add(a as usize - 1)
                        } else {
                            range.start.saturating_sub(a.unsigned_abs() as usize)
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
                        handler.unmark_text();
                        handler.replace_and_mark_text_in_range(
                            Some(actual.unwrap_or(range)),
                            &text,
                            None,
                        );
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
    if after > 0 {
        handler.replace_text_in_range(Some(end..end + after), "");
    }
    if before > 0 {
        handler.replace_text_in_range(Some(start - before..start), "");
    }
    if let Some(marked) = marked {
        let range = marked.start - before..marked.end - before;
        if let Some(text) = handler.text_for_range(range.clone(), &mut None) {
            handler.replace_and_mark_text_in_range(Some(range), &text, None);
        }
    }
    let range = selection.range.start - before..selection.range.end - before;
    handler.set_selected_text_range(if selection.reversed {
        range.end..range.start
    } else {
        range
    });
    true
}
