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

/// Emitted when a multiple selection changes, in selection order.
#[derive(Clone, Debug)]
pub struct MultiSelectChanged {
    pub selected: Vec<SharedString>,
}

/// Retained options, selection and query for one Select or MultiSelect trigger.
pub struct SelectState {
    pub(super) options: Vec<SelectOption>,
    selected: Vec<SharedString>,
    pub(super) multiple: bool,
    limit: Option<usize>,
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
impl EventEmitter<MultiSelectChanged> for SelectState {}
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
            selected: Vec::new(),
            multiple: false,
            limit: Some(1),
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
    pub fn multiple(
        options: Vec<SelectOption>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut state = Self::new(options, window, cx);
        state.multiple = true;
        state.limit = None;
        state
    }
    /// Limits the number of selected IDs in a multiple-selection state.
    pub fn max_selected(mut self, maximum: usize) -> Self {
        assert!(self.multiple, "max_selected requires SelectState::multiple");
        assert!(
            self.selected.len() <= maximum,
            "selection exceeds max_selected"
        );
        self.limit = Some(maximum);
        self
    }
    pub fn selection_limit(&self) -> Option<usize> {
        self.limit
    }
    pub fn selected_ids(&self) -> &[SharedString] {
        &self.selected
    }
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.iter().any(|selected| selected == id)
    }
    pub fn selected_id(&self) -> Option<&SharedString> {
        self.selected.first()
    }
    pub fn selected_option(&self) -> Option<&SelectOption> {
        self.options
            .iter()
            .find(|item| Some(&item.id) == self.selected.first())
    }
    pub fn is_open(&self) -> bool {
        self.opened
    }
    pub fn options(&self) -> &[SelectOption] {
        &self.options
    }
    /// Replaces options, retaining selected IDs that still exist.
    pub fn set_options(&mut self, options: Vec<SelectOption>, cx: &mut Context<Self>) {
        validate(&options);
        self.options = options;
        let previous = self.selected.len();
        self.selected
            .retain(|id| self.options.iter().any(|item| &item.id == id));
        if self.selected.len() != previous {
            self.changed(cx);
        }
        self.reconcile();
        self.reveal();
        cx.notify();
    }
    /// Selects an enabled option, or adds it in multiple mode. Returns false if unavailable or full.
    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(option) = self
            .options
            .iter()
            .find(|item| item.id == id && !item.disabled)
        else {
            return false;
        };
        let id = option.id.clone();
        if self.is_selected(&id) {
            return true;
        }
        if self.multiple {
            if self.at_limit() {
                return false;
            }
            self.selected.push(id);
        } else {
            self.selected = vec![id];
        }
        self.changed(cx);
        true
    }
    /// Removes an ID, including an option that has become disabled.
    pub fn deselect(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.selected.iter().position(|selected| selected == id) else {
            return false;
        };
        self.selected.remove(index);
        self.changed(cx);
        true
    }
    /// Atomically replaces selection with unique, enabled IDs within the limit.
    pub fn set_selected_ids(&mut self, ids: Vec<SharedString>, cx: &mut Context<Self>) -> bool {
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        if unique.len() != ids.len()
            || self.limit.is_some_and(|limit| ids.len() > limit)
            || ids.iter().any(|id| {
                !self
                    .options
                    .iter()
                    .any(|item| &item.id == id && !item.disabled)
            })
        {
            return false;
        }
        if self.selected != ids {
            self.selected = ids;
            self.changed(cx);
        }
        true
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.selected.is_empty() {
            self.selected.clear();
            self.changed(cx);
        }
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.multiple {
            cx.emit(MultiSelectChanged {
                selected: self.selected.clone(),
            });
        } else {
            cx.emit(SelectChanged {
                selected: self.selected.first().cloned(),
            });
        }
        self.reconcile();
        cx.notify();
    }
    pub(super) fn at_limit(&self) -> bool {
        self.limit.is_some_and(|limit| self.selected.len() >= limit)
    }
    pub(super) fn unavailable(&self, option: &SelectOption) -> bool {
        option.disabled || (self.multiple && self.at_limit() && !self.is_selected(&option.id))
    }
    pub(super) fn choose(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled
            || self
                .options
                .iter()
                .find(|item| item.id == id)
                .is_none_or(|item| item.disabled)
        {
            return;
        }
        if self.multiple && self.is_selected(id) {
            self.deselect(id, cx);
        } else if self.select(id, cx) && !self.multiple {
            self.close(window, cx);
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
        self.active = self.selected.first().cloned();
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
            !self.unavailable(&self.options[i]) && self.active.as_ref() == Some(&self.options[i].id)
        }) {
            self.active = visible
                .into_iter()
                .find(|&i| !self.unavailable(&self.options[i]))
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
            .filter(|&i| !self.unavailable(&self.options[i]))
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
        {
            self.choose(&id, window, cx);
        }
    }
}
fn validate(options: &[SelectOption]) {
    let ids: std::collections::HashSet<_> = options.iter().map(|item| &item.id).collect();
    assert_eq!(ids.len(), options.len(), "select option IDs must be unique");
}
