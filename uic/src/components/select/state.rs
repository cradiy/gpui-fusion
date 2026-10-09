use crate::components::input::{InputEvent, TextInput};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, ScrollHandle,
    SharedString, Subscription, Window,
};

/// A stable value and its searchable display label.
#[derive(Clone, Debug)]
pub struct SelectOption {
    pub id: SharedString,
    pub label: SharedString,
    pub keywords: SharedString,
    pub disabled: bool,
}
impl SelectOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            keywords: "".into(),
            disabled: false,
        }
    }
    pub fn keywords(mut self, keywords: impl Into<SharedString>) -> Self {
        self.keywords = keywords.into();
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Emitted when the selected ID changes, including programmatic selection and clearing.
#[derive(Clone, Debug)]
pub struct SelectChanged {
    pub selected: Option<SharedString>,
}

/// Retained options, selection and query for one Select trigger.
pub struct SelectState {
    pub(super) options: Vec<SelectOption>,
    selected: Option<SharedString>,
    pub(super) query: String,
    pub(super) search: Entity<TextInput>,
    pub(super) active: Option<SharedString>,
    pub(super) opened: bool,
    pub(super) trigger_focus: FocusHandle,
    pub(super) scope_focus: FocusHandle,
    pub(super) panel_focus: FocusHandle,
    pub(super) scroll: ScrollHandle,
    pub(super) searchable: bool,
    pub(super) disabled: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<SelectChanged> for SelectState {}
impl Focusable for SelectState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.trigger_focus.clone()
    }
}
impl SelectState {
    pub fn new(options: Vec<SelectOption>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        validate(&options);
        let search = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("Search options")
                .aria_label("Search options")
        });
        let scope_focus = cx.focus_handle();
        let subscriptions = vec![
            cx.subscribe_in(
                &search,
                window,
                |state, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change(value) => {
                        state.query = value.to_lowercase();
                        state.reconcile();
                        state.reveal();
                        cx.notify();
                    }
                    InputEvent::Submit(_) => {
                        if !state.search.read(cx).is_composing() {
                            state.accept(window, cx);
                        }
                    }
                },
            ),
            window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
            cx.on_focus_out(&scope_focus, window, |state, _, window, cx| {
                if state.opened
                    && !state.search.focus_handle(cx).is_focused(window)
                    && !state.panel_focus.is_focused(window)
                {
                    state.finish(cx);
                    window.refresh();
                }
            }),
        ];
        Self {
            options,
            selected: None,
            query: String::new(),
            search,
            active: None,
            opened: false,
            trigger_focus: cx.focus_handle(),
            scope_focus,
            panel_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            searchable: true,
            disabled: false,
            _subscriptions: subscriptions,
        }
    }
    pub fn selected_id(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }
    pub fn selected_option(&self) -> Option<&SelectOption> {
        self.options
            .iter()
            .find(|item| Some(&item.id) == self.selected.as_ref())
    }
    pub fn is_open(&self) -> bool {
        self.opened
    }
    pub fn options(&self) -> &[SelectOption] {
        &self.options
    }
    /// Replaces options, preserving selection by ID. Removing that ID clears the selection.
    pub fn set_options(&mut self, options: Vec<SelectOption>, cx: &mut Context<Self>) {
        validate(&options);
        self.options = options;
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.options.iter().any(|item| &item.id == id))
        {
            self.clear(cx);
        }
        self.reconcile();
        self.reveal();
        cx.notify();
    }
    /// Selects an enabled option. Returns false if the ID is absent or disabled.
    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(option) = self
            .options
            .iter()
            .find(|item| item.id == id && !item.disabled)
        else {
            return false;
        };
        let id = option.id.clone();
        if self.selected.as_ref() != Some(&id) {
            self.selected = Some(id);
            cx.emit(SelectChanged {
                selected: self.selected.clone(),
            });
            cx.notify();
        }
        true
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if self.selected.take().is_some() {
            cx.emit(SelectChanged { selected: None });
            cx.notify();
        }
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.opened {
            return;
        }
        self.finish(cx);
        self.trigger_focus.focus(window, cx);
        window.refresh();
    }
    pub(super) fn finish(&mut self, cx: &mut Context<Self>) {
        self.opened = false;
        self.active = None;
        cx.notify();
    }
    pub(super) fn begin(&mut self, cx: &mut Context<Self>) {
        self.query.clear();
        self.search.update(cx, |input, cx| input.clear(cx));
        self.active = self.selected.clone();
        self.reconcile();
        self.scroll.set_offset(Default::default());
        self.reveal();
        self.opened = true;
        cx.notify();
    }
    pub(super) fn visible(&self) -> Vec<usize> {
        self.options
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                self.query.is_empty()
                    || item.label.to_lowercase().contains(&self.query)
                    || item.keywords.to_lowercase().contains(&self.query)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn reconcile(&mut self) {
        let visible = self.visible();
        if !visible.iter().any(|&i| {
            !self.options[i].disabled && self.active.as_ref() == Some(&self.options[i].id)
        }) {
            self.active = visible
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
    pub(super) fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.disabled
            && self.is_open()
            && let Some(id) = self.active.clone()
            && self.select(&id, cx)
        {
            self.close(window, cx);
        }
    }
}
fn validate(options: &[SelectOption]) {
    let ids: std::collections::HashSet<_> = options.iter().map(|item| &item.id).collect();
    assert_eq!(ids.len(), options.len(), "select option IDs must be unique");
}
