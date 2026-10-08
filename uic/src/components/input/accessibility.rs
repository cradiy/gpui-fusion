use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    A11ySubtreeBuilder, Bounds, Context, Div, EntityInputHandler, Pixels, SharedString, Stateful,
    accesskit::{self, Action, ActionData, Node, NodeId, Role, TextPosition, TextSelection},
    prelude::*,
};
use unicode_segmentation::UnicodeSegmentation;

use super::{InputMode, TextInput, state::TextLayout};

pub(super) type Geometry = Rc<RefCell<TextGeometry>>;

#[derive(Default)]
pub(super) struct TextGeometry {
    rows: Vec<(Range<usize>, accesskit::Rect)>,
}

impl TextGeometry {
    pub fn update(&mut self, text: &str, layout: &TextLayout, bounds: Bounds<Pixels>, scale: f32) {
        self.rows.clear();
        let mut start = 0;
        let mut y = layout.position_for_offset(0).y;
        for (offset, _) in text.grapheme_indices(true).skip(1) {
            let next_y = layout.position_for_offset(offset).y;
            if next_y != y {
                self.push_row(start..offset, y, layout, bounds, scale);
                start = offset;
                y = next_y;
            }
        }
        self.push_row(start..text.len(), y, layout, bounds, scale);
        if text.ends_with('\n') {
            let next_y = layout.position_for_offset(text.len()).y;
            if next_y != y {
                self.push_row(text.len()..text.len(), next_y, layout, bounds, scale);
            }
        }
    }

    fn push_row(
        &mut self,
        range: Range<usize>,
        y: Pixels,
        layout: &TextLayout,
        bounds: Bounds<Pixels>,
        scale: f32,
    ) {
        // TextRun character geometry is optional. Keep one bounding box per visual
        // row without inventing left-to-right positions for bidirectional glyphs.
        let left = f32::from(bounds.left()) * scale;
        let top = f32::from(bounds.top() + y) * scale;
        self.rows.push((
            range,
            accesskit::Rect::new(
                left as f64,
                top as f64,
                (left + f32::from(bounds.size.width).max(1.) * scale) as f64,
                (top + f32::from(layout.line_height) * scale) as f64,
            ),
        ));
    }
}

struct Run {
    id: NodeId,
    offsets: Vec<usize>,
}

pub(super) struct InputAccessibility {
    text: SharedString,
    selection: Range<usize>,
    reversed: bool,
    password: bool,
    multiline: bool,
    disabled: bool,
    label: SharedString,
    placeholder: SharedString,
    pub geometry: Geometry,
}

impl InputAccessibility {
    pub fn new(input: &TextInput) -> Self {
        Self {
            text: input.content.clone(),
            selection: input.selected_range.clone(),
            reversed: input.selection_reversed,
            password: input.mode == InputMode::Password,
            multiline: input.mode == InputMode::Multiline,
            disabled: input.disabled,
            label: input
                .accessible_label
                .clone()
                .unwrap_or_else(|| input.placeholder.clone()),
            placeholder: input.placeholder.clone(),
            geometry: Geometry::default(),
        }
    }

    pub fn decorate(self, element: Stateful<Div>, cx: &mut Context<TextInput>) -> Stateful<Div> {
        let runs = Rc::new(RefCell::new(Vec::<Run>::new()));
        let snapshot = self.text.clone();
        let action_runs = runs.clone();
        let mut element = element
            .role(if self.password {
                Role::PasswordInput
            } else if self.multiline {
                Role::MultilineTextInput
            } else {
                Role::TextInput
            })
            .aria_label(self.label.clone())
            .aria_placeholder(self.placeholder.clone())
            .aria_disabled(self.disabled);
        if !self.disabled {
            for action in [Action::Focus, Action::Click] {
                let input = cx.weak_entity();
                element = element.on_a11y_action(action, move |_, window, cx| {
                    let _ = input.update(cx, |input, cx| {
                        if !input.disabled {
                            window.focus(&input.focus_handle, cx);
                            window.show_soft_keyboard();
                        }
                    });
                });
            }
            let input = cx.weak_entity();
            element = element.on_a11y_action(Action::SetTextSelection, move |data, window, cx| {
                let Some(ActionData::SetTextSelection(selection)) = data else {
                    return;
                };
                let runs = action_runs.borrow();
                let offset = |position: TextPosition| {
                    runs.iter()
                        .find(|run| run.id == position.node)
                        .and_then(|run| run.offsets.get(position.character_index).copied())
                };
                let (Some(anchor), Some(focus)) =
                    (offset(selection.anchor), offset(selection.focus))
                else {
                    return;
                };
                let _ = input.update(cx, |input, cx| {
                    if input.disabled || input.content != snapshot {
                        return;
                    }
                    let anchor = input.content[..anchor].encode_utf16().count();
                    let focus = input.content[..focus].encode_utf16().count();
                    window.focus(&input.focus_handle, cx);
                    input.set_selected_text_range(anchor..focus, window, cx);
                });
            });
            for action in [Action::SetValue, Action::ReplaceSelectedText] {
                let input = cx.weak_entity();
                element = element.on_a11y_action(action, move |data, window, cx| {
                    let Some(ActionData::Value(value)) = data else {
                        return;
                    };
                    let _ = input.update(cx, |input, cx| {
                        if input.disabled {
                            return;
                        }
                        let range = (action == Action::SetValue)
                            .then(|| 0..input.content.encode_utf16().count());
                        input.replace_text_in_range(range, value, window, cx);
                    });
                });
            }
        }
        element.a11y_synthetic_children(move |builder| {
            *runs.borrow_mut() = self.build_tree(builder);
        })
    }

    fn build_tree(&self, builder: &mut A11ySubtreeBuilder) -> Vec<Run> {
        let mut runs = Vec::new();
        let mut values = Vec::new();
        let word_starts = if self.password {
            Vec::new()
        } else {
            self.text
                .unicode_word_indices()
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>()
        };
        for (range, bounds) in &self.geometry.borrow().rows {
            let text = &self.text[range.clone()];
            let mut offsets = Vec::new();
            for (offset, grapheme) in text.grapheme_indices(true) {
                if !self.password && grapheme.len() > u8::MAX as usize {
                    offsets.extend(
                        grapheme
                            .char_indices()
                            .map(|(index, _)| range.start + offset + index),
                    );
                } else {
                    offsets.push(range.start + offset);
                }
            }
            offsets.push(range.end);
            let mut start = 0;
            loop {
                let end = (start + 255).min(offsets.len() - 1);
                let part = offsets[start..=end].to_vec();
                let value = if self.password {
                    "•".repeat(end - start)
                } else {
                    self.text[part[0]..part[end - start]].to_owned()
                };
                let mut node = Node::new(Role::TextRun);
                node.set_value(value.clone());
                node.set_character_lengths(if self.password {
                    vec![3; end - start]
                } else {
                    part.windows(2)
                        .map(|pair| (pair[1] - pair[0]) as u8)
                        .collect()
                });
                let words = word_starts
                    .iter()
                    .copied()
                    .filter(|byte| *byte >= part[0] && *byte < part[end - start])
                    .filter_map(|byte| u8::try_from(part.binary_search(&byte).ok()?).ok())
                    .collect::<Vec<_>>();
                node.set_word_starts(words);
                node.set_bounds(*bounds);
                let id = builder.synthetic_node_id(("text", runs.len()));
                if start > 0 {
                    node.set_previous_on_line(builder.synthetic_node_id(("text", runs.len() - 1)));
                }
                if end < offsets.len() - 1 {
                    node.set_next_on_line(builder.synthetic_node_id(("text", runs.len() + 1)));
                }
                builder.push_child(id, node);
                runs.push(Run { id, offsets: part });
                values.push(value);
                if end == offsets.len() - 1 {
                    break;
                }
                start = end;
            }
        }
        builder.parent_node().set_value(values.concat());
        let position = |offset: usize| {
            let run = runs.iter().rev().find(|run| run.offsets[0] <= offset)?;
            let index = run
                .offsets
                .partition_point(|byte| *byte <= offset)
                .saturating_sub(1);
            Some(TextPosition {
                node: run.id,
                character_index: index,
            })
        };
        if let (Some(start), Some(end)) =
            (position(self.selection.start), position(self.selection.end))
        {
            builder.parent_node().set_text_selection(if self.reversed {
                TextSelection {
                    anchor: end,
                    focus: start,
                }
            } else {
                TextSelection {
                    anchor: start,
                    focus: end,
                }
            });
        }
        runs
    }
}
