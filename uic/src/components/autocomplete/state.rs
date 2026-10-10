use crate::components::{
    input::{InputEvent, TextInput},
    select::SelectOption,
};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, ScrollHandle,
    SharedString, Subscription, Window,
};

#[derive(Clone, Debug)]
pub enum AutocompleteEvent {
    Change(SharedString),
    Selected(SelectOption),
    Submit(SharedString),
}

/// Editable text and suggestions for one autocomplete input in one window.
pub struct AutocompleteState {
    pub(super) input: Entity<TextInput>,
    pub(super) options: Vec<SelectOption>,
    value: SharedString,
    pub(super) label: SharedString,
    pub(super) active: Option<SharedString>,
    pub(super) opened: bool,
    pub(super) loading: bool,
    filtering: bool,
    auto_highlight: bool,
    highlight_on_update: bool,
    pub(super) scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<AutocompleteEvent> for AutocompleteState {}
impl Focusable for AutocompleteState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl AutocompleteState {
    pub fn new(options: Vec<SelectOption>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        validate(&options);
        let input = cx.new(TextInput::new);
        let focus = input.focus_handle(cx);
        let subscriptions = vec![
            cx.subscribe_in(
                &input,
                window,
                |state, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change(value) => {
                        if value != &state.input.read(cx).value() || state.value == *value {
                            return;
                        }
                        state.value = value.clone();
                        state.active = None;
                        state.opened =
                            state.focus_handle(cx).is_focused(window) && !state.is_disabled(cx);
                        state.highlight_on_update = state.opened;
                        state.highlight_first();
                        state.scroll.set_offset(Default::default());
                        state.reveal();
                        cx.emit(AutocompleteEvent::Change(value.clone()));
                        cx.notify();
                    }
                    InputEvent::Submit(_) if !state.input.read(cx).is_composing() => {
                        state.submit(window, cx)
                    }
                    _ => {}
                },
            ),
            cx.on_focus(&focus, window, |state, _, cx| state.open(cx)),
            cx.on_blur(&focus, window, |state, _, cx| state.close(cx)),
            cx.observe(&input, |state, _, cx| {
                if state.is_disabled(cx) {
                    state.opened = false;
                }
                cx.notify();
            }),
            window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
        ];
        Self {
            input,
            options,
            value: "".into(),
            label: "".into(),
            active: None,
            opened: false,
            loading: false,
            filtering: true,
            auto_highlight: false,
            highlight_on_update: false,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }
    /// Disable local filtering when the application supplies already-filtered suggestions.
    pub fn filter(mut self, enabled: bool) -> Self {
        self.filtering = enabled;
        self
    }
    /// Highlight the first enabled suggestion after editing. Disabled by default.
    pub fn auto_highlight(mut self, enabled: bool) -> Self {
        self.auto_highlight = enabled;
        self
    }
    pub fn input(&self) -> &Entity<TextInput> {
        &self.input
    }
    pub fn value(&self) -> &SharedString {
        &self.value
    }
    pub fn options(&self) -> &[SelectOption] {
        &self.options
    }
    pub fn is_open(&self) -> bool {
        self.opened
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn is_disabled(&self, cx: &App) -> bool {
        self.input.read(cx).is_disabled()
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        if disabled {
            self.close(cx);
        }
    }
    /// Update text without opening suggestions. Emits Change only when the committed value changes.
    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let value = value.into();
        self.input
            .update(cx, |input, cx| input.set_value(value.clone(), cx));
        if self.value != value {
            self.value = value.clone();
            self.active = None;
            self.highlight_on_update = false;
            cx.emit(AutocompleteEvent::Change(value));
        }
        cx.notify();
    }
    /// Replace suggestions without changing text or reopening a dismissed menu.
    pub fn set_options(&mut self, options: Vec<SelectOption>, cx: &mut Context<Self>) {
        validate(&options);
        self.options = options;
        if !self.visible().iter().any(|&i| {
            !self.options[i].disabled && self.active.as_ref() == Some(&self.options[i].id)
        }) {
            self.active = None;
        }
        self.highlight_first();
        self.reveal();
        cx.notify();
    }
    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        if self.loading != loading {
            self.loading = loading;
            self.active = None;
            self.highlight_first();
            self.reveal();
            cx.notify();
        }
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.highlight_on_update = false;
        if self.opened {
            self.opened = false;
            self.active = None;
            cx.notify();
        }
    }
    pub(super) fn open(&mut self, cx: &mut Context<Self>) {
        if !self.opened && !self.is_disabled(cx) && !self.input.read(cx).is_composing() {
            self.opened = true;
            self.active = None;
            self.highlight_on_update = false;
            cx.notify();
        }
    }
    pub(super) fn visible(&self) -> Vec<usize> {
        let query = self.value.to_lowercase();
        self.options
            .iter()
            .enumerate()
            .filter(|(_, option)| {
                !self.filtering
                    || query.is_empty()
                    || option.label.to_lowercase().contains(&query)
                    || option.keywords.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn highlight_first(&mut self) {
        if self.auto_highlight
            && self.highlight_on_update
            && self.opened
            && !self.loading
            && self.active.is_none()
        {
            self.active = self
                .visible()
                .into_iter()
                .find(|&i| !self.options[i].disabled)
                .map(|i| self.options[i].id.clone());
        }
    }
    fn reveal(&self) {
        if let Some(index) = self
            .visible()
            .iter()
            .position(|&i| self.active.as_ref() == Some(&self.options[i].id))
        {
            self.scroll.scroll_to_item(index);
        }
    }
    pub(super) fn navigate(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.open(cx);
        if self.loading {
            return;
        }
        let ids: Vec<_> = self
            .visible()
            .into_iter()
            .filter(|&i| !self.options[i].disabled)
            .map(|i| self.options[i].id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        let next = match ids.iter().position(|id| Some(id) == self.active.as_ref()) {
            Some(i) if forward => (i + 1).min(ids.len() - 1),
            Some(i) => i.saturating_sub(1),
            None if forward => 0,
            None => ids.len() - 1,
        };
        self.active = Some(ids[next].clone());
        self.reveal();
        cx.notify();
    }
    pub(super) fn choose(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.opened
            || self.loading
            || self.is_disabled(cx)
            || self.input.read(cx).is_composing()
        {
            return;
        }
        let Some(option) = self
            .visible()
            .into_iter()
            .map(|i| &self.options[i])
            .find(|option| option.id == id && !option.disabled)
            .cloned()
        else {
            return;
        };
        self.set_value(option.label.clone(), cx);
        self.close(cx);
        self.focus_handle(cx).focus(window, cx);
        cx.emit(AutocompleteEvent::Selected(option));
    }
    pub(super) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_disabled(cx) || self.input.read(cx).is_composing() {
            return;
        }
        if self.opened
            && let Some(id) = self.active.clone()
        {
            self.choose(&id, window, cx);
        } else {
            self.close(cx);
            cx.emit(AutocompleteEvent::Submit(self.value.clone()));
        }
    }
}
fn validate(options: &[SelectOption]) {
    let mut ids = std::collections::HashSet::new();
    assert!(
        options.iter().all(|option| ids.insert(&option.id)),
        "autocomplete option IDs must be unique"
    );
}
