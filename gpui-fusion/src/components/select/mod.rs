mod render;

use crate::{
    components::{
        dropdown::{DropdownPlacement, motion::MenuMotion},
        input::{Input, InputEvent, InputState},
    },
    style::{MaterialMode, Part, StylePatch, Styled},
    theme::{StyleError, ThemeScope, material_binding::MaterialBinding, runtime::ThemeRuntime},
};
use gpui::{
    App, AppContext, Context, ElementBounds, Entity, FocusHandle, Focusable, KeyDownEvent,
    ScrollStrategy, SharedString, Subscription, UniformListScrollHandle, Window, point, px,
};
use std::{
    collections::BTreeSet,
    rc::Rc,
    time::{Duration, Instant},
};

/// A stable value, visible label, and optional additional search terms.
#[derive(Clone, Debug)]
pub struct SelectOption {
    value: SharedString,
    label: SharedString,
    keywords: SharedString,
    icon: Option<SharedString>,
    disabled: bool,
}
impl SelectOption {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            keywords: "".into(),
            icon: None,
            disabled: false,
        }
    }
    pub fn keywords(mut self, keywords: impl Into<SharedString>) -> Self {
        self.keywords = keywords.into();
        self
    }
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
#[derive(Clone, Copy, Debug)]
pub enum SelectPart {
    Panel,
    Item,
    Icon,
    Placeholder,
    Empty,
    Clear,
}
impl Part for SelectPart {
    fn name(&self) -> &str {
        match self {
            Self::Panel => "panel",
            Self::Item => "item",
            Self::Icon => "icon",
            Self::Placeholder => "placeholder",
            Self::Empty => "empty",
            Self::Clear => "clear",
        }
    }
}
struct Search {
    editor: Entity<InputState>,
    input: Entity<Input>,
    _subscription: Subscription,
}
type Change = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;

/// A single-selection field with an anchored, optionally searchable virtual list.
pub struct Select {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    options: Vec<SelectOption>,
    search_terms: Vec<String>,
    filtered: Vec<usize>,
    query: String,
    selected: Option<usize>,
    active: Option<usize>,
    label: SharedString,
    placeholder: SharedString,
    search_placeholder: SharedString,
    empty_text: SharedString,
    searchable: bool,
    clearable: bool,
    disabled: bool,
    hovered: bool,
    pressed: bool,
    open: bool,
    search: Option<Search>,
    focus: FocusHandle,
    clear_focus: FocusHandle,
    scope_focus: FocusHandle,
    panel_focus: FocusHandle,
    anchor: ElementBounds,
    scroll: UniformListScrollHandle,
    placement: DropdownPlacement,
    duration: Duration,
    motion: MenuMotion,
    on_change: Option<Change>,
    cache: Option<render::Cache>,
    error: Option<StyleError>,
    _subscriptions: Vec<Subscription>,
}
impl Select {
    /// Values must be unique and nonempty; labels must not be empty. Initially unselected.
    pub fn new(
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = SelectOption>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, StyleError> {
        let options: Vec<_> = options.into_iter().collect();
        let search_terms = validate(&options)?;
        let theme = ThemeRuntime::global(cx);
        let focus = cx.focus_handle();
        let clear_focus = cx.focus_handle();
        let scope_focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe(&theme, |_, _, cx| cx.notify()),
            cx.on_focus(&focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.pressed = false;
                cx.notify();
            }),
            cx.on_focus(&clear_focus, window, |_, _, cx| cx.notify()),
            cx.on_blur(&clear_focus, window, |_, _, cx| cx.notify()),
            cx.on_focus_out(&scope_focus, window, |this, _, window, cx| {
                if this.open && !this.has_focus(window, cx) {
                    this.set_open(false, false, window, cx);
                }
            }),
        ];
        Ok(Self {
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            filtered: (0..options.len()).collect(),
            options,
            search_terms,
            query: String::new(),
            selected: None,
            active: None,
            label: label.into(),
            placeholder: "Choose an option".into(),
            search_placeholder: "Search options".into(),
            empty_text: "No matching options".into(),
            searchable: false,
            clearable: false,
            disabled: false,
            hovered: false,
            pressed: false,
            open: false,
            search: None,
            focus,
            clear_focus,
            scope_focus,
            panel_focus: cx.focus_handle(),
            anchor: ElementBounds::default(),
            scroll: UniformListScrollHandle::new(),
            placement: DropdownPlacement::default(),
            duration: Duration::from_millis(600),
            motion: MenuMotion::new(Instant::now()),
            on_change: None,
            cache: None,
            error: None,
            _subscriptions: subscriptions,
        })
    }
    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.map(|i| &self.options[i].value)
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    /// Updates selection without invoking the user callback.
    pub fn set_selected(
        &mut self,
        value: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        self.selected = value
            .map(|value| {
                self.options
                    .iter()
                    .position(|o| o.value == value && !o.disabled)
                    .ok_or_else(|| {
                        StyleError::new("select.selected", "value must name an enabled option")
                    })
            })
            .transpose()?;
        self.reconcile();
        cx.notify();
        Ok(())
    }
    /// Replaces options atomically and retains an enabled selection with the same value.
    pub fn set_options(
        &mut self,
        options: impl IntoIterator<Item = SelectOption>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let options: Vec<_> = options.into_iter().collect();
        let terms = validate(&options)?;
        let selected = self.selected().cloned();
        self.selected = options
            .iter()
            .position(|o| Some(&o.value) == selected.as_ref() && !o.disabled);
        self.options = options;
        self.search_terms = terms;
        self.filter();
        cx.notify();
        Ok(())
    }
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }
    pub fn search_placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.search_placeholder = text.into();
        self
    }
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }
    pub fn searchable(mut self, enabled: bool) -> Self {
        self.searchable = enabled;
        self
    }
    pub fn clearable(mut self, enabled: bool) -> Self {
        self.clearable = enabled;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.pressed = false;
        self.hovered = false;
        if disabled {
            self.set_open(false, false, window, cx);
            if self.has_focus(window, cx) {
                window.blur();
            }
        }
        cx.notify();
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn placement(mut self, placement: DropdownPlacement) -> Self {
        self.placement = placement;
        self
    }
    pub fn set_placement(&mut self, placement: DropdownPlacement, cx: &mut Context<Self>) {
        self.placement = placement;
        cx.notify();
    }
    pub fn transition_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.cache = None;
        self
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
        self.cache = None;
        self
    }
    pub fn set_variant(&mut self, variant: Option<String>, cx: &mut Context<Self>) {
        self.variant = variant;
        self.cache = None;
        cx.notify();
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.cache = None;
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    pub fn on_change(
        mut self,
        callback: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(true, true, window, cx);
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(false, true, window, cx);
    }
    fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.is_focused(window)
            || self.clear_focus.is_focused(window)
            || self.panel_focus.is_focused(window)
            || self
                .search
                .as_ref()
                .is_some_and(|s| s.editor.focus_handle(cx).is_focused(window))
    }
    fn ensure_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.searchable || self.search.is_some() {
            return;
        }
        let editor = cx.new(|cx| {
            InputState::new(cx)
                .placeholder(self.search_placeholder.clone())
                .aria_label(self.search_placeholder.clone())
        });
        let input = cx.new(|cx| {
            let mut input = Input::new(&editor, window, cx)
                .blur_on_click_outside(false)
                .material_mode(MaterialMode::Plain);
            for scope in &self.scopes {
                input = input.scope(scope.clone());
            }
            input
        });
        let subscription = cx.subscribe_in(
            &editor,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change(value) => {
                    this.query = value.to_string();
                    this.filter();
                    cx.notify();
                }
                InputEvent::Submit(_) => {
                    if this.open && !this.composing(cx) {
                        this.accept(window, cx);
                    }
                }
            },
        );
        self.search = Some(Search {
            editor,
            input,
            _subscription: subscription,
        });
    }
    fn composing(&self, cx: &App) -> bool {
        self.search
            .as_ref()
            .is_some_and(|s| s.editor.read(cx).is_composing())
    }
    fn set_open(&mut self, open: bool, restore: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.open == open || (open && self.disabled) {
            return;
        }
        self.progress();
        self.open = open;
        self.progress();
        self.pressed = false;
        if open {
            self.ensure_search(window, cx);
            self.query.clear();
            self.filter();
            if let Some(search) = &self.search {
                search
                    .editor
                    .update(cx, |editor, cx| editor.set_value("", cx));
                search.editor.focus_handle(cx).focus(window, cx);
            } else {
                self.panel_focus.focus(window, cx);
            }
        } else {
            if let Some(search) = &self.search {
                search
                    .editor
                    .update(cx, |editor, cx| editor.set_value(editor.value(), cx));
            }
            if restore {
                self.focus.focus(window, cx);
            } else if self.has_focus(window, cx) {
                window.blur();
            }
        }
        cx.notify();
    }
    fn progress(&mut self) -> f32 {
        self.motion.sample(self.open, Instant::now(), self.duration)
    }
    fn filter(&mut self) {
        let query = self.query.to_lowercase();
        self.filtered.clear();
        self.filtered.extend(
            self.search_terms
                .iter()
                .enumerate()
                .filter_map(|(i, text)| text.contains(query.trim()).then_some(i)),
        );
        self.scroll
            .0
            .borrow()
            .base_handle
            .set_offset(point(px(0.), px(0.)));
        self.reconcile();
    }
    fn reconcile(&mut self) {
        self.active = self
            .filtered
            .iter()
            .position(|i| Some(*i) == self.selected && !self.options[*i].disabled)
            .or_else(|| {
                self.filtered
                    .iter()
                    .position(|i| !self.options[*i].disabled)
            });
        self.reveal();
    }
    fn reveal(&self) {
        if let Some(i) = self.active {
            self.scroll.scroll_to_item(i, ScrollStrategy::Nearest);
        }
    }
    fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active.and_then(|i| self.filtered.get(i)).copied() {
            self.choose(index, window, cx);
        }
    }
    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.disabled || self.options.get(index).is_none_or(|o| o.disabled) {
            return;
        }
        let changed = self.selected != Some(index);
        self.selected = Some(index);
        self.close(window, cx);
        if changed {
            self.emit(window, cx);
        }
    }
    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || !self.clearable || self.selected.is_none() {
            return;
        }
        self.selected = None;
        if self.open {
            self.close(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        self.reconcile();
        self.emit(window, cx);
        cx.notify();
    }
    fn emit(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = &self.on_change {
            callback(self.selected().cloned(), window, cx);
        }
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.composing(cx) {
            return;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if key == "tab" && self.open && !modifiers.control && !modifiers.alt && !modifiers.platform
        {
            self.close(window, cx);
            if modifiers.shift {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        if modifiers.modified() {
            return;
        }
        if !self.open {
            if matches!(key, "up" | "down") {
                self.open(window, cx);
                cx.stop_propagation();
                window.prevent_default();
            }
            return;
        }
        let editing = self
            .search
            .as_ref()
            .is_some_and(|s| s.editor.focus_handle(cx).is_focused(window));
        match key {
            "escape" => self.close(window, cx),
            "enter" => self.accept(window, cx),
            "space" if !editing => self.accept(window, cx),
            "up" | "down" => {
                let n = self.filtered.len();
                for step in 1..=n {
                    let start = self.active.unwrap_or(if key == "up" { 0 } else { n - 1 });
                    let next = if key == "up" {
                        (start + n - step) % n
                    } else {
                        (start + step) % n
                    };
                    if !self.options[self.filtered[next]].disabled {
                        self.active = Some(next);
                        break;
                    }
                }
                self.reveal();
                cx.notify();
            }
            "home" | "end" if !editing => {
                self.active = if key == "home" {
                    self.filtered
                        .iter()
                        .position(|i| !self.options[*i].disabled)
                } else {
                    self.filtered
                        .iter()
                        .rposition(|i| !self.options[*i].disabled)
                };
                self.reveal();
                cx.notify();
            }
            _ => return,
        }
        cx.stop_propagation();
        window.prevent_default();
    }
}
impl Styled for Select {
    type Part = SelectPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Focusable for Select {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
fn validate(options: &[SelectOption]) -> Result<Vec<String>, StyleError> {
    let mut values = BTreeSet::new();
    options
        .iter()
        .map(|o| {
            if o.value.trim().is_empty()
                || o.label.trim().is_empty()
                || !values.insert(o.value.clone())
            {
                return Err(StyleError::new(
                    "select.options",
                    "values must be unique and nonempty; labels must not be empty",
                ));
            }
            Ok(format!("{} {}", o.label, o.keywords).to_lowercase())
        })
        .collect()
}

#[cfg(test)]
mod tests;
