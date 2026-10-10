use crate::components::{
    input::{InputEvent, TextInput},
    modal,
};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, ScrollHandle,
    SharedString, Subscription, Window,
};
use std::rc::Weak;

/// A command description. The application owns the action associated with its ID.
#[derive(Clone, Debug)]
pub struct CommandItem {
    pub id: SharedString,
    pub label: SharedString,
    pub description: SharedString,
    pub keywords: SharedString,
    pub group: Option<SharedString>,
    pub icon: Option<SharedString>,
    pub shortcut: Option<SharedString>,
    pub disabled: bool,
}
impl CommandItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: "".into(),
            keywords: "".into(),
            group: None,
            icon: None,
            shortcut: None,
            disabled: false,
        }
    }
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = text.into();
        self
    }
    pub fn keywords(mut self, text: impl Into<SharedString>) -> Self {
        self.keywords = text.into();
        self
    }
    pub fn group(mut self, label: impl Into<SharedString>) -> Self {
        self.group = Some(label.into());
        self
    }
    /// An SVG path resolved by the application's asset source.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }
    /// Display text only; this does not register a shortcut.
    pub fn shortcut(mut self, text: impl Into<SharedString>) -> Self {
        self.shortcut = Some(text.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug)]
pub enum CommandPaletteEvent {
    QueryChanged(SharedString),
    Invoked(CommandItem),
    /// Escape requested dismissal. Modal presentation closes automatically.
    Dismissed,
}

pub(super) enum Row {
    Group(SharedString),
    Item(usize),
}

/// Search and keyboard selection for one command palette in one window.
pub struct CommandPaletteState {
    pub(super) input: Entity<TextInput>,
    pub(super) items: Vec<CommandItem>,
    query: SharedString,
    pub(super) active: Option<SharedString>,
    pub(super) loading: bool,
    filtering: bool,
    pub(super) label: SharedString,
    pub(super) scroll: ScrollHandle,
    pub(super) modal_lifetime: Weak<()>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<CommandPaletteEvent> for CommandPaletteState {}
impl Focusable for CommandPaletteState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl CommandPaletteState {
    pub fn new(items: Vec<CommandItem>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        validate(&items);
        let input = cx.new(TextInput::new);
        let subscriptions = vec![
            cx.subscribe_in(
                &input,
                window,
                |state, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change(value) => {
                        if value != &state.input.read(cx).value() || state.query == *value {
                            return;
                        }
                        state.query = value.clone();
                        state.active = None;
                        state.reconcile();
                        state.scroll.set_offset(Default::default());
                        state.reveal();
                        cx.emit(CommandPaletteEvent::QueryChanged(value.clone()));
                        cx.notify();
                    }
                    InputEvent::Submit(_) => state.accept(window, cx),
                },
            ),
            cx.observe(&input, |_, _, cx| cx.notify()),
            window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
        ];
        let mut state = Self {
            input,
            items,
            query: "".into(),
            active: None,
            loading: false,
            filtering: true,
            label: "".into(),
            scroll: ScrollHandle::new(),
            modal_lifetime: Weak::new(),
            _subscriptions: subscriptions,
        };
        state.reconcile();
        state
    }
    /// Disable local filtering when the application supplies ranked search results.
    pub fn filter(mut self, enabled: bool) -> Self {
        self.filtering = enabled;
        self.reconcile();
        self
    }
    pub fn input(&self) -> &Entity<TextInput> {
        &self.input
    }
    pub fn query(&self) -> &SharedString {
        &self.query
    }
    pub fn items(&self) -> &[CommandItem] {
        &self.items
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn set_query(&mut self, query: impl Into<SharedString>, cx: &mut Context<Self>) {
        let query = query.into();
        self.input
            .update(cx, |input, cx| input.set_value(query.clone(), cx));
        if self.query != query {
            self.query = query.clone();
            self.active = None;
            self.reconcile();
            self.scroll.set_offset(Default::default());
            self.reveal();
            cx.emit(CommandPaletteEvent::QueryChanged(query));
            cx.notify();
        }
    }
    pub fn set_items(&mut self, items: Vec<CommandItem>, cx: &mut Context<Self>) {
        validate(&items);
        self.items = items;
        self.reconcile();
        self.reveal();
        cx.notify();
    }
    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        if self.loading != loading {
            self.loading = loading;
            self.reconcile();
            self.reveal();
            cx.notify();
        }
    }
    pub(super) fn rows(&self) -> Vec<Row> {
        let query = self.query.to_lowercase();
        let words: Vec<_> = query.split_whitespace().collect();
        let visible: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if !self.filtering || words.is_empty() {
                    return true;
                }
                let text = format!(
                    "{} {} {} {}",
                    item.label,
                    item.description,
                    item.keywords,
                    item.group.as_deref().unwrap_or("")
                )
                .to_lowercase();
                words.iter().all(|word| text.contains(word))
            })
            .map(|(index, _)| index)
            .collect();
        let mut groups = Vec::new();
        for &index in &visible {
            if !groups.contains(&self.items[index].group) {
                groups.push(self.items[index].group.clone());
            }
        }
        let mut rows = Vec::new();
        for group in groups {
            if let Some(label) = &group {
                rows.push(Row::Group(label.clone()));
            }
            rows.extend(
                visible
                    .iter()
                    .filter(|&&index| self.items[index].group == group)
                    .map(|&index| Row::Item(index)),
            );
        }
        rows
    }
    fn enabled_ids(&self) -> Vec<SharedString> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                Row::Item(index) if !self.items[index].disabled => {
                    Some(self.items[index].id.clone())
                }
                _ => None,
            })
            .collect()
    }
    fn reconcile(&mut self) {
        if self.loading {
            self.active = None;
            return;
        }
        let ids = self.enabled_ids();
        if !self.active.as_ref().is_some_and(|id| ids.contains(id)) {
            self.active = ids.first().cloned();
        }
    }
    fn reveal(&self) {
        if let Some(row) = self.rows().iter().position(|row| matches!(row, Row::Item(index) if self.active.as_ref() == Some(&self.items[*index].id))) {
            self.scroll.scroll_to_item(row);
        }
    }
    pub(super) fn blocked(&self, cx: &App) -> bool {
        self.loading || self.input.read(cx).is_composing() || self.input.read(cx).is_disabled()
    }
    pub(super) fn navigate(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.blocked(cx) {
            return;
        }
        let ids = self.enabled_ids();
        if ids.is_empty() {
            return;
        }
        let index = match ids.iter().position(|id| Some(id) == self.active.as_ref()) {
            Some(index) if forward => (index + 1).min(ids.len() - 1),
            Some(index) => index.saturating_sub(1),
            None if forward => 0,
            None => ids.len() - 1,
        };
        self.active = Some(ids[index].clone());
        self.reveal();
        cx.notify();
    }
    pub(super) fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.active.clone() {
            self.invoke(&id, window, cx);
        }
    }
    pub(super) fn invoke(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked(cx) || !self.enabled_ids().iter().any(|visible| visible == id) {
            return;
        }
        let item = self
            .items
            .iter()
            .find(|item| item.id == id)
            .unwrap()
            .clone();
        self.close_modal(window, cx);
        cx.emit(CommandPaletteEvent::Invoked(item));
    }
    pub(super) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_modal(window, cx);
        cx.emit(CommandPaletteEvent::Dismissed);
    }
    fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_lifetime.upgrade().is_some() {
            self.modal_lifetime = Weak::new();
            modal::dismiss(window, cx);
        }
    }
}
fn validate(items: &[CommandItem]) {
    let mut ids = std::collections::HashSet::new();
    assert!(
        items.iter().all(|item| ids.insert(&item.id)),
        "command IDs must be unique"
    );
}
