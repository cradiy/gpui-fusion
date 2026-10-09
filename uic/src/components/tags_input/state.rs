use std::rc::Rc;

use crate::components::input::{InputActionEvent, InputEvent, TextInput};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString,
    Subscription, TextInputAction, Window,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct TagsInputOptions {
    pub max_tags: Option<usize>,
    pub case_sensitive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagsInputError {
    Empty,
    Duplicate,
    LimitReached,
    Invalid(SharedString),
}
impl std::fmt::Display for TagsInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("Enter a tag"),
            Self::Duplicate => f.write_str("This tag already exists"),
            Self::LimitReached => f.write_str("The tag limit has been reached"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for TagsInputError {}

#[derive(Clone, Debug)]
pub struct TagsInputChanged {
    pub tags: Vec<SharedString>,
}

type Validator = Rc<dyn Fn(&str) -> Result<(), SharedString>>;

pub struct TagsInputState {
    tags: Vec<SharedString>,
    options: TagsInputOptions,
    validator: Option<Validator>,
    error: Option<TagsInputError>,
    pub(super) input: Entity<TextInput>,
    pub(super) selected: Option<SharedString>,
    pub(super) label: SharedString,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<TagsInputChanged> for TagsInputState {}
impl Focusable for TagsInputState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl TagsInputState {
    pub fn new(
        tags: Vec<SharedString>,
        options: TagsInputOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("Add a tag")
                .aria_label("Tags")
                .input_action(TextInputAction::Done)
        });
        let focus = input.focus_handle(cx);
        let subscriptions = vec![
            cx.subscribe(&input, |state, _, event: &InputEvent, cx| match event {
                InputEvent::Change(_) => {
                    state.error = None;
                    state.selected = None;
                    cx.notify();
                }
                InputEvent::Submit(_) => state.submit(cx),
            }),
            cx.subscribe(&input, |state, _, event: &InputActionEvent, cx| {
                if event.action == TextInputAction::Done {
                    state.submit(cx);
                }
            }),
            cx.on_blur(&focus, window, |state, _, cx| {
                state.selected = None;
                cx.notify();
            }),
            window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
        ];
        let mut state = Self {
            tags: Vec::new(),
            options,
            validator: None,
            error: None,
            input,
            selected: None,
            label: "Tags".into(),
            _subscriptions: subscriptions,
        };
        state.tags = state.validate_tags(tags).expect("invalid initial tags");
        state
    }
    /// Validates future additions and replacements. Existing tags are unchanged.
    pub fn validator(
        mut self,
        validate: impl Fn(&str) -> Result<(), SharedString> + 'static,
    ) -> Self {
        self.validator = Some(Rc::new(validate));
        self
    }
    pub fn tags(&self) -> &[SharedString] {
        &self.tags
    }
    pub fn options(&self) -> TagsInputOptions {
        self.options
    }
    pub fn error(&self) -> Option<&TagsInputError> {
        self.error.as_ref()
    }
    pub fn draft(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }
    pub fn is_disabled(&self, cx: &App) -> bool {
        self.input.read(cx).is_disabled()
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        if disabled {
            self.selected = None;
        }
        cx.notify();
    }
    /// Replaces tags atomically without emitting TagsInputChanged. Invalid replacements leave the state intact.
    pub fn set_tags(
        &mut self,
        tags: Vec<SharedString>,
        cx: &mut Context<Self>,
    ) -> Result<(), TagsInputError> {
        let tags = self.validate_tags(tags)?;
        self.tags = tags;
        self.selected = None;
        self.error = None;
        self.input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
        Ok(())
    }
    /// Adds a trimmed tag and emits TagsInputChanged. Validation failures leave tags unchanged.
    pub fn add(
        &mut self,
        tag: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Result<(), TagsInputError> {
        let tag = tag.into();
        let tag = self.validate_tag(&tag, &self.tags)?;
        self.tags.push(tag);
        self.selected = None;
        self.error = None;
        self.changed(cx);
        Ok(())
    }
    pub fn remove(&mut self, tag: &str, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.tags.iter().position(|value| value == tag) else {
            return false;
        };
        self.tags.remove(index);
        if self.selected.as_deref() == Some(tag) {
            self.selected = self
                .tags
                .get(index.min(self.tags.len().saturating_sub(1)))
                .cloned();
        }
        self.error = None;
        self.changed(cx);
        true
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(TagsInputChanged {
            tags: self.tags.clone(),
        });
        cx.notify();
    }
    pub(super) fn submit(&mut self, cx: &mut Context<Self>) {
        if self.is_disabled(cx) || self.input.read(cx).is_composing() {
            return;
        }
        match self.add(self.draft(cx), cx) {
            Ok(()) => self.input.update(cx, |input, cx| input.clear(cx)),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    pub(super) fn navigate(&mut self, forward: bool, cx: &mut Context<Self>) {
        let index = self
            .selected
            .as_ref()
            .and_then(|selected| self.tags.iter().position(|tag| tag == selected));
        self.selected = match (index, forward) {
            (Some(index), true) => self.tags.get(index + 1).cloned(),
            (Some(index), false) => self.tags.get(index.saturating_sub(1)).cloned(),
            (None, false) => self.tags.last().cloned(),
            (None, true) => None,
        };
        cx.notify();
    }
    pub(super) fn backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(tag) = self.selected.clone() {
            self.remove(&tag, cx);
        } else {
            self.navigate(false, cx);
        }
    }
    pub(super) fn cancel_selection(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.error = None;
        cx.notify();
    }
    fn validate_tag(
        &self,
        value: &str,
        existing: &[SharedString],
    ) -> Result<SharedString, TagsInputError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(TagsInputError::Empty);
        }
        if value.chars().any(char::is_control) {
            return Err(TagsInputError::Invalid(
                "Tags cannot contain control characters".into(),
            ));
        }
        if existing.iter().any(|tag| {
            if self.options.case_sensitive {
                tag.as_ref() == value
            } else {
                tag.to_lowercase() == value.to_lowercase()
            }
        }) {
            return Err(TagsInputError::Duplicate);
        }
        if self
            .options
            .max_tags
            .is_some_and(|limit| existing.len() >= limit)
        {
            return Err(TagsInputError::LimitReached);
        }
        if let Some(validate) = &self.validator {
            validate(value).map_err(TagsInputError::Invalid)?;
        }
        Ok(value.to_owned().into())
    }
    fn validate_tags(&self, tags: Vec<SharedString>) -> Result<Vec<SharedString>, TagsInputError> {
        let mut validated = Vec::with_capacity(tags.len());
        for tag in tags {
            validated.push(self.validate_tag(&tag, &validated)?);
        }
        Ok(validated)
    }
}
