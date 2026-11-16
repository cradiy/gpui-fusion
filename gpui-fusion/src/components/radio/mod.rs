use crate::{
    components::{
        motion::{GlassMotion, Spring},
        selection_surface::{length, selection_surface},
    },
    style::{ComputedStyle, MaterialMode, Part, State, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{Styled as _, prelude::*, *};
use std::{collections::BTreeSet, rc::Rc, time::Instant};

/// A stable selection value and its accessible, visible label.
#[derive(Clone, Debug)]
pub struct RadioOption {
    value: SharedString,
    label: SharedString,
    disabled: bool,
}
impl RadioOption {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RadioPart {
    Item,
    Indicator,
    Dot,
    Label,
}
impl Part for RadioPart {
    fn name(&self) -> &str {
        match self {
            Self::Item => "item",
            Self::Indicator => "indicator",
            Self::Dot => "dot",
            Self::Label => "label",
        }
    }
}

type Change = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
struct Item {
    option: RadioOption,
    focus: FocusHandle,
    selection: Spring,
    motion: GlassMotion,
}
#[derive(Clone)]
struct Styles {
    root: ComputedStyle,
    items: Vec<[ComputedStyle; 5]>,
}
struct Cache {
    revision: u64,
    material: MaterialMode,
    focused: Option<usize>,
    styles: Styles,
}

/// Mutually exclusive choices. Only the circular indicators accept pointer activation.
pub struct RadioGroup {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    items: Vec<Item>,
    selected: Option<usize>,
    label: SharedString,
    fallback_focus: FocusHandle,
    horizontal: bool,
    disabled: bool,
    hovered: Option<usize>,
    pressed: Option<usize>,
    style: StylePatch,
    variant: Option<String>,
    scopes: Vec<ThemeScope>,
    on_change: Option<Change>,
    _subscriptions: Vec<Subscription>,
    resolved: Option<Cache>,
    error: Option<StyleError>,
    last_frame: Instant,
    animated: bool,
}
impl RadioGroup {
    /// Values must be unique and nonempty; labels must not be empty.
    /// The first enabled option is initially selected. Empty groups have no selection.
    pub fn new(
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = RadioOption>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, StyleError> {
        let options: Vec<_> = options.into_iter().collect();
        let mut values = BTreeSet::new();
        for option in &options {
            if option.value.trim().is_empty()
                || option.label.trim().is_empty()
                || !values.insert(option.value.clone())
            {
                return Err(StyleError::new(
                    "radio.options",
                    "values must be unique and nonempty; labels must not be empty",
                ));
            }
        }
        let selected = options.iter().position(|option| !option.disabled);
        let now = Instant::now();
        let theme = ThemeRuntime::global(cx);
        let mut subscriptions = vec![cx.observe(&theme, |_, _, cx| cx.notify())];
        let items = options
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                let focus = cx.focus_handle();
                subscriptions.push(cx.on_focus(&focus, window, |_, _, cx| cx.notify()));
                subscriptions.push(cx.on_blur(&focus, window, |this, _, cx| {
                    this.pressed = None;
                    this.invalidate(cx);
                }));
                Item {
                    option,
                    focus,
                    selection: Spring {
                        value: f32::from(selected == Some(index)),
                        velocity: 0.,
                    },
                    motion: GlassMotion::new(now),
                }
            })
            .collect();
        Ok(Self {
            theme,
            material: MaterialBinding::new(cx),
            items,
            selected,
            label: label.into(),
            fallback_focus: cx.focus_handle(),
            horizontal: false,
            disabled: false,
            hovered: None,
            pressed: None,
            style: StylePatch::default(),
            variant: None,
            scopes: Vec::new(),
            on_change: None,
            _subscriptions: subscriptions,
            resolved: None,
            error: None,
            last_frame: now,
            animated: true,
        })
    }
    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.map(|index| &self.items[index].option.value)
    }
    /// Updates selection without invoking the activation callback or moving focus.
    pub fn set_selected(&mut self, value: &str, cx: &mut Context<Self>) -> Result<(), StyleError> {
        let index = self
            .items
            .iter()
            .position(|item| item.option.value == value && !item.option.disabled)
            .ok_or_else(|| {
                StyleError::new("radio.selected", "value must name an enabled option")
            })?;
        self.selected = Some(index);
        self.invalidate(cx);
        Ok(())
    }
    pub fn horizontal(mut self, horizontal: bool) -> Self {
        self.horizontal = horizontal;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.resolved = None;
        self
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.hovered = None;
        self.pressed = None;
        self.invalidate(cx);
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.resolved = None;
        self
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
        self.resolved = None;
        self
    }
    pub fn set_variant(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        self.variant = name;
        self.invalidate(cx);
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.invalidate(cx);
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.resolved = None;
        cx.notify();
    }
    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.items[index].option.disabled {
            return;
        }
        self.items[index].focus.focus(window, cx);
        if self.selected == Some(index) {
            return;
        }
        self.selected = Some(index);
        self.invalidate(cx);
        if let Some(callback) = self.on_change.clone() {
            callback(self.items[index].option.value.clone(), window, cx);
        }
    }
    fn navigate(
        &mut self,
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || event.keystroke.modifiers.modified() {
            return;
        }
        let count = self.items.len();
        let next = match event.keystroke.key.as_str() {
            "left" | "up" => (1..=count)
                .map(|step| (index + count - step) % count)
                .find(|index| !self.items[*index].option.disabled),
            "right" | "down" => (1..=count)
                .map(|step| (index + step) % count)
                .find(|index| !self.items[*index].option.disabled),
            "home" => self.items.iter().position(|item| !item.option.disabled),
            "end" => self.items.iter().rposition(|item| !item.option.disabled),
            "space" | "enter" => {
                self.pressed = Some(index);
                self.invalidate(cx);
                return;
            }
            _ => {
                if self.pressed.take().is_some() {
                    self.invalidate(cx);
                }
                return;
            }
        };
        self.pressed = None;
        if let Some(next) = next {
            self.activate(next, window, cx);
        }
        cx.stop_propagation();
    }
    fn resolve(&mut self, window: &Window, cx: &App) -> Result<Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let focused = window
            .last_input_was_keyboard()
            .then(|| {
                self.items
                    .iter()
                    .position(|item| item.focus.is_focused(window))
            })
            .flatten();
        if let Some(cache) = &self.resolved {
            if cache.revision == runtime.revision()
                && cache.material == material
                && cache.focused == focused
            {
                return Ok(cache.styles.clone());
            }
        }
        let store = runtime.store();
        let mut context = ResolveContext::new(store.theme(), store.mode(), runtime.style_set())?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style
            .validate(&context, &["item", "indicator", "dot", "label"])?;
        let recipe = runtime.styles().recipe("radio")?;
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
        let root = resolve(
            &StateSelector(if self.disabled {
                BTreeSet::from([State::Disabled])
            } else {
                BTreeSet::new()
            }),
            None,
        )?;
        let items = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let disabled = self.disabled || item.option.disabled;
                let states = StateSelector(
                    [
                        (State::Selected, self.selected == Some(index)),
                        (State::Disabled, disabled),
                        (State::Hovered, !disabled && self.hovered == Some(index)),
                        (State::Pressed, !disabled && self.pressed == Some(index)),
                        (State::FocusVisible, !disabled && focused == Some(index)),
                    ]
                    .into_iter()
                    .filter_map(|(state, active)| active.then_some(state))
                    .collect(),
                );
                let mut off = states.clone();
                off.0.remove(&State::Selected);
                let mut on = states.clone();
                on.0.insert(State::Selected);
                Ok([
                    resolve(&states, Some("item"))?,
                    resolve(&off, Some("indicator"))?,
                    resolve(&on, Some("indicator"))?,
                    resolve(&states, Some("dot"))?,
                    resolve(&states, Some("label"))?,
                ])
            })
            .collect::<Result<_, StyleError>>()?;
        let styles = Styles { root, items };
        self.resolved = Some(Cache {
            revision: runtime.revision(),
            material,
            focused,
            styles: styles.clone(),
        });
        Ok(styles)
    }
}
impl Styled for RadioGroup {
    type Part = RadioPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.resolved = None;
        &mut self.style
    }
}
impl Focusable for RadioGroup {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.selected
            .map(|index| self.items[index].focus.clone())
            .unwrap_or_else(|| self.fallback_focus.clone())
    }
}
impl Render for RadioGroup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let styles = match self.resolve(window, cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Radio style error: {error}"))
                    .into_any_element();
            }
        };
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.032);
        self.last_frame = now;
        let mut root = div()
            .id("radio-group")
            .role(Role::RadioGroup)
            .aria_label(self.label.clone())
            .aria_disabled(self.disabled)
            .flex()
            .when(!self.horizontal, |s| s.flex_col());
        root.style().refine(&styles.root.to_gpui());
        let item_count = self.items.len();
        for (index, (item, [row_style, off, on, dot, label_style])) in
            self.items.iter_mut().zip(styles.items).enumerate()
        {
            let selected = self.selected == Some(index);
            let disabled = self.disabled || item.option.disabled;
            let target = f32::from(selected);
            if self.animated {
                item.selection.advance(target, dt, 24., 0.9);
            } else {
                item.selection = Spring {
                    value: target,
                    velocity: 0.,
                };
            }
            if item.selection.value != target || item.selection.velocity != 0. {
                window.request_animation_frame();
            }
            let amount = item.selection.value.clamp(0., 1.);
            let (scale, moving) = item.motion.sample(
                if !disabled && self.pressed == Some(index) {
                    if off.material.is_some() { 1.12 } else { 0.94 }
                } else {
                    1.
                },
                now,
                self.animated && !disabled,
            );
            if moving {
                window.request_animation_frame();
            }
            let mut dot_element = div().flex_none().rounded_full();
            dot_element.style().refine(&dot.to_gpui());
            let dot_element = dot_element
                .w(length(&dot.width, 8.) * scale * amount)
                .h(length(&dot.height, 8.) * scale * amount)
                .opacity(amount);
            let indicator = selection_surface(
                div()
                    .relative()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .debug_selector(move || format!("fusion-radio-indicator-{index}")),
                off,
                on,
                selected,
                amount,
                scale,
                ((scale - 1.) / 0.12).clamp(0., 1.),
            )
            .child(dot_element);
            let mut control = indicator
                .id(("radio", index))
                .track_focus(&item.focus.clone().tab_stop(selected && !disabled))
                .role(Role::RadioButton)
                .aria_label(item.option.label.clone())
                .aria_toggled(if selected {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .aria_disabled(disabled)
                .aria_position_in_set(index + 1)
                .aria_size_of_set(item_count);
            if !disabled {
                let weak = cx.weak_entity();
                control = control
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered {
                            this.hovered = Some(index);
                        } else if this.hovered == Some(index) {
                            this.hovered = None;
                        }
                        if !hovered && this.pressed == Some(index) {
                            this.pressed = None;
                        }
                        this.invalidate(cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.items[index].focus.focus(window, cx);
                            this.pressed = Some(index);
                            this.invalidate(cx);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pressed = None;
                            this.invalidate(cx);
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pressed = None;
                            this.invalidate(cx);
                        }),
                    )
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.activate(index, window, cx)),
                    )
                    .on_key_down(cx.listener(move |this, event, window, cx| {
                        this.navigate(index, event, window, cx)
                    }))
                    .on_key_up(cx.listener(|this, _: &KeyUpEvent, _, cx| {
                        if this.pressed.take().is_some() {
                            this.invalidate(cx);
                        }
                    }))
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.activate(index, window, cx));
                    });
            }
            let mut label = div()
                .child(item.option.label.clone())
                .debug_selector(move || format!("fusion-radio-label-{index}"));
            label.style().refine(&label_style.to_gpui());
            let mut row = div().flex().items_center().child(control).child(label);
            row.style().refine(&row_style.to_gpui());
            root = root.child(row);
        }
        root.into_any_element()
    }
}
