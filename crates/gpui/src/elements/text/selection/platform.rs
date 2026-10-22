use super::*;
use crate::{InputHandler, SurroundingText, UTF16Selection};

pub(super) struct SelectionInput {
    state: Rc<RefCell<TextSelection>>,
    rows: Rc<Vec<Row>>,
    bounds: Bounds<Pixels>,
}

impl SelectionInput {
    pub fn new(
        state: Rc<RefCell<TextSelection>>,
        rows: Rc<Vec<Row>>,
        bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            state,
            rows,
            bounds,
        }
    }
}

fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut offset = 0;
    for (byte, character) in text.char_indices() {
        if offset + character.len_utf16() > utf16 {
            return byte;
        }
        offset += character.len_utf16();
    }
    text.len()
}

impl InputHandler for SelectionInput {
    fn accepts_text_input(&mut self, _: &mut Window, _: &mut App) -> bool {
        false
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        let state = self.state.borrow();
        let range = state.range();
        Some(UTF16Selection {
            range: state.text[..range.start].encode_utf16().count()
                ..state.text[..range.end].encode_utf16().count(),
            reversed: state.head < state.anchor,
        })
    }

    fn set_selected_text_range(&mut self, range: Range<usize>, window: &mut Window, _: &mut App) {
        let mut state = self.state.borrow_mut();
        state.anchor = byte_offset(&state.text, range.start);
        state.head = byte_offset(&state.text, range.end);
        drop(state);
        window.refresh();
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        if range.start > range.end {
            return None;
        }
        let state = self.state.borrow();
        let start = byte_offset(&state.text, range.start);
        let end = byte_offset(&state.text, range.end);
        *adjusted = Some(
            state.text[..start].encode_utf16().count()..state.text[..end].encode_utf16().count(),
        );
        Some(state.text[start..end].to_owned())
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut App) -> Option<usize> {
        Some(self.state.borrow().text.encode_utf16().count())
    }

    fn surrounding_text(
        &mut self,
        max_bytes: usize,
        _: &mut Window,
        _: &mut App,
    ) -> Option<SurroundingText> {
        let state = self.state.borrow();
        SurroundingText::from_utf8(&state.text, state.head, state.anchor, None, max_bytes)
    }

    fn element_bounds(&mut self, _: &mut Window, _: &mut App) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let state = self.state.borrow();
        let index = byte_offset(&state.text, range.start);
        let row = self
            .rows
            .iter()
            .find(|row| row.range.contains(&index))
            .or_else(|| self.rows.iter().rev().find(|row| row.range.end == index))?;
        let local = index.saturating_sub(row.line_start).min(row.layout.len);
        let x = row.layout.x_for_index(local) - row.base_x;
        Some(Bounds::new(
            row.origin + point(x, px(0.)),
            size(px(1.), row.height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        let state = self.state.borrow();
        Some(
            state.text[..index_at(&self.rows, position).min(state.text.len())]
                .encode_utf16()
                .count(),
        )
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}
}
