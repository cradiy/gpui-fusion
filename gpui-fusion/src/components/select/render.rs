use super::*;
use crate::{
    components::{
        dropdown::{glass::GlassMenu, motion::phase, presentation::PlacedMenu},
        selection_surface::{color, length, selection_surface},
    },
    style::{ComputedStyle, State, StateSelector},
    theme::ResolveContext,
};
use gpui::{Styled, prelude::*, *};
use gpui_icons::GpuiIcons;
use std::ops::Range;

#[derive(Clone)]
struct Styles {
    trigger: ComputedStyle,
    panel: ComputedStyle,
    placeholder: ComputedStyle,
    icon: ComputedStyle,
    empty: ComputedStyle,
    clear: ComputedStyle,
    rows: Vec<ComputedStyle>,
    row_height: Pixels,
}
pub(super) struct Cache {
    revision: u64,
    material: MaterialMode,
    flags: u8,
    styles: Rc<Styles>,
}
impl Select {
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<Rc<Styles>, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let flags = [
            (State::Disabled, self.disabled),
            (State::Hovered, !self.disabled && self.hovered),
            (State::Pressed, !self.disabled && self.pressed),
            (State::Selected, self.open),
            (
                State::FocusVisible,
                !self.disabled && self.has_focus(window, cx) && window.last_input_was_keyboard(),
            ),
        ];
        let key = flags
            .iter()
            .enumerate()
            .fold(0, |key, (i, (_, active))| key | (u8::from(*active) << i));
        if let Some(cache) = &self.cache {
            if cache.revision == runtime.revision()
                && cache.material == material
                && cache.flags == key
            {
                return Ok(cache.styles.clone());
            }
        }
        let mut context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style.validate(
            &context,
            &["panel", "item", "icon", "placeholder", "empty", "clear"],
        )?;
        let recipe = runtime.styles().recipe("select")?;
        let resolve = |states: &StateSelector, part| {
            recipe.resolve(
                self.variant.as_deref(),
                material,
                &self.style,
                &context,
                states,
                part,
            )
        };
        let states = StateSelector(
            flags
                .into_iter()
                .filter_map(|(s, active)| active.then_some(s))
                .collect(),
        );
        let rows = (0..8)
            .map(|key| {
                let states = StateSelector(
                    [
                        (State::Hovered, key & 1 != 0),
                        (State::Selected, key & 2 != 0),
                        (State::Disabled, key & 4 != 0),
                    ]
                    .into_iter()
                    .filter_map(|(s, active)| active.then_some(s))
                    .collect(),
                );
                resolve(&states, Some("item"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let row_height = length(&rows[0].height, 36.).max(px(1.));
        if rows
            .iter()
            .any(|s| length(&s.height, 36.).max(px(1.)) != row_height)
        {
            return Err(StyleError::new(
                "select.item.height",
                "virtual list items must have the same height in every state",
            ));
        }
        let styles = Rc::new(Styles {
            trigger: resolve(&states, None)?,
            panel: resolve(&StateSelector::default(), Some("panel"))?,
            placeholder: resolve(&states, Some("placeholder"))?,
            icon: resolve(&states, Some("icon"))?,
            empty: resolve(&states, Some("empty"))?,
            clear: resolve(&states, Some("clear"))?,
            rows,
            row_height,
        });
        self.cache = Some(Cache {
            revision: runtime.revision(),
            material,
            flags: key,
            styles: styles.clone(),
        });
        Ok(styles)
    }
    fn content(&self, styles: &Styles) -> Div {
        let mut label = div().flex_1().min_w_0().text_ellipsis().child(
            self.selected
                .map(|i| self.options[i].label.clone())
                .unwrap_or_else(|| self.placeholder.clone()),
        );
        if self.selected.is_none() {
            label.style().refine(&styles.placeholder.to_gpui());
        }
        let mut content = div()
            .flex()
            .items_center()
            .size_full()
            .gap(px(8.))
            .child(label);
        if self.clearable && self.selected.is_some() {
            content = content.child(
                svg()
                    .path(GpuiIcons::LucideX.path())
                    .size(px(16.))
                    .text_color(color(&styles.clear.foreground)),
            );
        }
        content.child(
            svg()
                .path(GpuiIcons::LucideChevronDown.path())
                .size(px(16.))
                .text_color(color(&styles.icon.foreground)),
        )
    }
    fn rows(
        &self,
        range: Range<usize>,
        styles: &Styles,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        range
            .map(|position| {
                let index = self.filtered[position];
                let option = &self.options[index];
                let selected = self.selected == Some(index);
                let flags = usize::from(self.active == Some(position))
                    | (usize::from(selected) << 1)
                    | (usize::from(option.disabled) << 2);
                let style = &styles.rows[flags];
                let mut row = div()
                    .id(("option", index))
                    .debug_selector(move || format!("fusion-select-option-{index}"))
                    .role(Role::ListBoxOption)
                    .aria_selected(selected)
                    .aria_disabled(option.disabled)
                    .aria_label(option.label.clone())
                    .aria_position_in_set(position + 1)
                    .aria_size_of_set(self.filtered.len())
                    .flex()
                    .items_center()
                    .flex_none()
                    .overflow_hidden();
                row.style().refine(&style.to_gpui());
                row = row.w_full().h(styles.row_height);
                if self.active == Some(position) {
                    row = row.aria_active_descendant();
                }
                if let Some(icon) = &option.icon {
                    row = row.child(
                        svg()
                            .path(icon.clone())
                            .size(px(16.))
                            .text_color(color(&style.foreground)),
                    );
                }
                row = row
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(option.label.clone()),
                    )
                    .child(div().size(px(16.)).flex_none().when(selected, |s| {
                        s.child(
                            svg()
                                .path(GpuiIcons::LucideCheck.path())
                                .size_full()
                                .text_color(color(&style.foreground)),
                        )
                    }));
                if self.open && !option.disabled {
                    let weak = cx.weak_entity();
                    row = row
                        .cursor_pointer()
                        .on_hover(cx.listener(move |this, hovered, _, cx| {
                            if *hovered && this.active != Some(position) {
                                this.active = Some(position);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.choose(index, window, cx);
                            cx.stop_propagation();
                        }))
                        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| this.choose(index, window, cx));
                        });
                }
                row.into_any_element()
            })
            .collect()
    }
    fn popup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.anchor.visible_bounds(window).is_none() {
            if self.open {
                let weak = cx.weak_entity();
                window.defer(cx, move |window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this.has_focus(window, cx) {
                            window.blur();
                        }
                        this.set_open(false, false, window, cx);
                    });
                });
            }
            return None;
        }
        let anchor_bounds = self.anchor.bounds(window)?;
        let styles = match self.resolve(window, cx) {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(e);
                return None;
            }
        };
        let progress = self.progress();
        if self.motion.active() {
            window.request_animation_frame();
        }
        let mut panel_style = styles.panel.clone();
        panel_style.material = styles.trigger.material;
        if panel_style.material.is_some() {
            panel_style.background = styles.trigger.background.clone();
            panel_style.background_gradient = styles.trigger.background_gradient.clone();
        }
        let fused = panel_style.material.is_some();
        let width = length(&styles.panel.width, anchor_bounds.size.width.as_f32())
            .max(px(1.))
            .min(window.viewport_size().width);
        let room = (window.viewport_size().height - anchor_bounds.bottom())
            .max(anchor_bounds.top())
            - px(12.);
        let padding =
            length(&panel_style.padding_top, 0.) + length(&panel_style.padding_bottom, 0.);
        let header = if self.searchable { px(48.) } else { px(0.) };
        let max_height = length(&panel_style.height, 320.)
            .min(room.max(px(1.)))
            .min(window.viewport_size().height);
        let list_height = (styles.row_height * self.filtered.len().max(1) as f32)
            .min((max_height - header - padding).max(px(1.)));
        let anchor = self.anchor.clone();
        let mut panel = div()
            .id("select-popup")
            .debug_selector(|| "fusion-select-popup".into())
            .track_focus(&self.panel_focus)
            .occlude()
            .flex()
            .flex_col()
            .overflow_hidden();
        panel.style().refine(&panel_style.to_gpui());
        panel = panel
            .w(width)
            .h((header + padding + list_height).min(max_height))
            .bg(transparent_black())
            .border_color(transparent_black())
            .opacity(if fused {
                phase(progress, 0.82, 1.)
            } else {
                progress
            });
        if self.open {
            panel = panel
                .on_mouse_down_out(
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if !anchor.contains(event.position, window) {
                            this.set_open(false, false, window, cx);
                        }
                    }),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        }
        if let Some(search) = &self.search {
            panel = panel.child(
                div()
                    .h(header)
                    .flex_none()
                    .pb(px(8.))
                    .child(search.input.clone()),
            );
        }
        let mut list = div()
            .id("select-list-scope")
            .role(Role::ListBox)
            .aria_label(self.label.clone())
            .h(list_height)
            .min_h_0()
            .overflow_hidden();
        if self.filtered.is_empty() {
            let mut empty = div()
                .debug_selector(|| "fusion-select-empty".into())
                .flex()
                .items_center()
                .h(styles.row_height)
                .child(self.empty_text.clone());
            empty.style().refine(&styles.empty.to_gpui());
            list = list.child(empty);
        } else {
            let row_styles = styles.clone();
            list = list.child(
                uniform_list(
                    "select-options",
                    self.filtered.len(),
                    cx.processor(move |this, range, _, cx| this.rows(range, &row_styles, cx)),
                )
                .track_scroll(&self.scroll)
                .size_full(),
            );
        }
        panel = panel.child(list);
        let mut trigger_content = self.content(&styles);
        let mut refinement = styles.trigger.to_gpui();
        refinement.background = Some(transparent_black().into());
        refinement.border_color = Some(transparent_black());
        trigger_content.style().refine(&refinement);
        trigger_content = trigger_content
            .w(anchor_bounds.size.width)
            .h(anchor_bounds.size.height);
        if fused {
            Some(
                GlassMenu {
                    content: panel.into_any_element(),
                    trigger_content: trigger_content.into_any_element(),
                    anchor: anchor_bounds,
                    progress,
                    placement: self.placement,
                    trigger_radius: length(&styles.trigger.radius, 8.),
                    trigger_scale: [1., 1.],
                    keep_trigger: true,
                    style: panel_style,
                }
                .into_any_element(),
            )
        } else {
            Some(
                PlacedMenu {
                    content: panel.into_any_element(),
                    settled_trigger: div()
                        .w(anchor_bounds.size.width)
                        .h(anchor_bounds.size.height)
                        .into_any_element(),
                    anchor: anchor_bounds,
                    placement: self.placement,
                    progress,
                    style: panel_style,
                }
                .into_any_element(),
            )
        }
    }
}
impl Render for Select {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let styles = match self.resolve(window, cx) {
            Ok(s) => {
                self.error = None;
                s
            }
            Err(e) => {
                self.error = Some(e.clone());
                return div()
                    .child(format!("Select style error: {e}"))
                    .into_any_element();
            }
        };
        let progress = self.progress();
        let visible = self.open || self.motion.active() || progress > 0.;
        if self.motion.active() {
            window.request_animation_frame();
        }
        let fused = visible && styles.trigger.material.is_some();
        let weak = cx.weak_entity();
        let overlay = visible.then(|| {
            deferred_overlay(move |window, cx| {
                weak.update(cx, |this, cx| this.popup(window, cx))
                    .ok()
                    .flatten()
            })
            .with_priority(100)
        });
        let mut content = div().flex().items_center().gap(px(8.)).size_full();
        let mut text = div().flex_1().min_w_0().text_ellipsis().child(
            self.selected
                .map(|i| self.options[i].label.clone())
                .unwrap_or_else(|| self.placeholder.clone()),
        );
        if self.selected.is_none() {
            text.style().refine(&styles.placeholder.to_gpui());
        }
        content = content.child(text);
        if self.clearable && self.selected.is_some() {
            let weak = cx.weak_entity();
            let mut clear = div()
                .id("clear")
                .debug_selector(|| "fusion-select-clear".into())
                .role(Role::Button)
                .aria_label("Clear selection")
                .aria_disabled(self.disabled)
                .track_focus(&self.clear_focus.clone().tab_stop(!self.disabled))
                .flex_none()
                .flex()
                .items_center()
                .justify_center();
            clear.style().refine(&styles.clear.to_gpui());
            if !self.disabled {
                clear = clear
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear(window, cx);
                        cx.stop_propagation();
                    }))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.clear(window, cx));
                    });
            }
            content = content.child(
                clear.child(
                    svg()
                        .path(GpuiIcons::LucideX.path())
                        .size(px(16.))
                        .text_color(color(&styles.clear.foreground)),
                ),
            );
        }
        content = content.child(
            svg()
                .path(GpuiIcons::LucideChevronDown.path())
                .size(px(16.))
                .flex_none()
                .text_color(color(&styles.icon.foreground)),
        );
        let weak = cx.weak_entity();
        let mut trigger = selection_surface(
            div().relative(),
            styles.trigger.clone(),
            styles.trigger.clone(),
            false,
            0.,
            1.,
            0.,
        )
        .id("select-trigger")
        .debug_selector(|| "fusion-select-trigger".into())
        .track_focus(&self.focus.clone().tab_stop(!self.disabled))
        .role(Role::ComboBox)
        .aria_label(self.label.clone())
        .aria_expanded(self.open)
        .aria_disabled(self.disabled)
        .child(content)
        .opacity(if fused { 0. } else { 1. });
        if !self.disabled {
            trigger = trigger
                .cursor_pointer()
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.hovered = *hovered;
                    if !hovered {
                        this.pressed = false;
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus.focus(window, cx);
                        this.pressed = true;
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.pressed = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.pressed = false;
                        cx.notify();
                    }),
                )
                .on_click(
                    cx.listener(|this, _, window, cx| this.set_open(!this.open, true, window, cx)),
                )
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.set_open(!this.open, true, window, cx));
                });
        }
        let anchor = self.anchor.clone();
        div()
            .id("select")
            .w(length(&styles.trigger.width, 240.))
            .relative()
            .track_focus(&self.scope_focus)
            .capture_key_down(
                cx.listener(|this, event, window, cx| this.key_down(event, window, cx)),
            )
            .child(trigger)
            .children(overlay)
            .child(
                canvas(
                    move |bounds, window, _| window.track_element_bounds(&anchor, bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }
}
