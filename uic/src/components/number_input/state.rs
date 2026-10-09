use crate::components::input::{InputActionEvent, InputEvent, TextInput};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString,
    Subscription, TextInputAction, TextInputPurpose, Window,
};

/// Numeric bounds and decimal formatting. Precision is the maximum fractional digit count.
#[derive(Clone, Copy, Debug)]
pub struct NumberInputOptions {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub precision: usize,
}
impl Default for NumberInputOptions {
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            step: 1.,
            precision: 2,
        }
    }
}
impl NumberInputOptions {
    fn validate(self) {
        assert!(
            self.precision <= 12,
            "number input precision must be at most 12"
        );
        assert!(
            self.step.is_finite() && self.step > 0.,
            "number input step must be positive and finite"
        );
        assert_eq!(
            self.round(self.step),
            self.step,
            "step must be representable at the configured precision"
        );
        for bound in [self.min, self.max].into_iter().flatten() {
            assert!(bound.is_finite(), "number input bounds must be finite");
            assert_eq!(
                self.round(bound),
                bound,
                "bounds must be representable at the configured precision"
            );
        }
        assert!(
            self.min.unwrap_or(f64::MIN) <= self.max.unwrap_or(f64::MAX),
            "number input min must not exceed max"
        );
    }
    fn round(self, value: f64) -> f64 {
        format!("{:.*}", self.precision, value)
            .parse()
            .unwrap_or(value)
    }
    fn normalize(self, value: f64) -> f64 {
        let value = value.clamp(self.min.unwrap_or(f64::MIN), self.max.unwrap_or(f64::MAX));
        let value = self.round(value);
        if value == 0. { 0. } else { value }
    }
    fn format(self, value: f64) -> String {
        let text = format!("{:.*}", self.precision, value);
        if self.precision == 0 {
            text
        } else {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }
    fn purpose(self) -> TextInputPurpose {
        TextInputPurpose::Number {
            decimal: self.precision > 0,
            signed: self.min.is_none_or(|min| min < 0.),
        }
    }
}

/// A user committed a different numeric value. Draft edits and programmatic updates do not emit it.
#[derive(Clone, Copy, Debug)]
pub struct NumberInputChanged {
    pub value: f64,
}

pub struct NumberInputState {
    value: f64,
    options: NumberInputOptions,
    pub(super) input: Entity<TextInput>,
    pub(super) label: SharedString,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<NumberInputChanged> for NumberInputState {}
impl Focusable for NumberInputState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl NumberInputState {
    pub fn new(
        value: f64,
        options: NumberInputOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        options.validate();
        assert!(value.is_finite(), "number input value must be finite");
        let value = options.normalize(value);
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .initial_value(options.format(value))
                .aria_label("Value")
                .input_purpose(options.purpose())
                .input_action(TextInputAction::Done)
        });
        let focus = input.focus_handle(cx);
        let subscriptions = vec![
            cx.subscribe(&input, |state, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Submit(_)) {
                    state.commit(cx);
                }
                cx.notify();
            }),
            cx.subscribe(&input, |state, _, event: &InputActionEvent, cx| {
                if event.action == TextInputAction::Done {
                    state.commit(cx);
                }
            }),
            cx.on_blur(&focus, window, |state, _, cx| {
                state.commit(cx);
            }),
            window.observe(&cx.entity(), cx, |_, window, _| window.refresh()),
        ];
        Self {
            value,
            options,
            input,
            label: "Value".into(),
            _subscriptions: subscriptions,
        }
    }
    pub fn value(&self) -> f64 {
        self.value
    }
    pub fn options(&self) -> NumberInputOptions {
        self.options
    }
    pub fn draft(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }
    pub fn is_disabled(&self, cx: &App) -> bool {
        self.input.read(cx).is_disabled()
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if disabled && !self.is_disabled(cx) {
            self.cancel(cx);
        }
        self.input
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        cx.notify();
    }
    /// Replaces the draft and committed value without emitting NumberInputChanged. Rejects non-finite values.
    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) -> bool {
        if !value.is_finite() {
            return false;
        }
        self.value = self.options.normalize(value);
        self.sync_text(cx);
        cx.notify();
        true
    }
    pub fn set_options(&mut self, options: NumberInputOptions, cx: &mut Context<Self>) {
        options.validate();
        self.options = options;
        self.input.update(cx, |input, cx| {
            input.set_input_purpose(options.purpose(), cx)
        });
        self.set_value(self.value, cx);
    }
    /// Confirms a finite draft, clamping and rounding it. Invalid drafts revert to the committed value.
    pub fn commit(&mut self, cx: &mut Context<Self>) {
        if self.is_disabled(cx) || self.input.read(cx).is_composing() {
            return;
        }
        let value = self.parsed(cx).unwrap_or(self.value);
        self.apply(value, cx);
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.sync_text(cx);
    }
    pub fn increment(&mut self, cx: &mut Context<Self>) {
        self.adjust(true, cx);
    }
    pub fn decrement(&mut self, cx: &mut Context<Self>) {
        self.adjust(false, cx);
    }
    pub(super) fn can_adjust(&self, forward: bool, cx: &App) -> bool {
        !self.is_disabled(cx)
            && !self.input.read(cx).is_composing()
            && self.next(forward, cx) != self.base(cx)
    }
    fn parsed(&self, cx: &App) -> Option<f64> {
        self.draft(cx)
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    }
    fn base(&self, cx: &App) -> f64 {
        self.options
            .normalize(self.parsed(cx).unwrap_or(self.value))
    }
    fn next(&self, forward: bool, cx: &App) -> f64 {
        let delta = if forward {
            self.options.step
        } else {
            -self.options.step
        };
        self.options.normalize(self.base(cx) + delta)
    }
    fn adjust(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.is_disabled(cx) || self.input.read(cx).is_composing() {
            return;
        }
        self.apply(self.next(forward, cx), cx);
    }
    fn apply(&mut self, value: f64, cx: &mut Context<Self>) {
        let value = self.options.normalize(value);
        if self.value != value {
            self.value = value;
            cx.emit(NumberInputChanged { value });
        }
        self.sync_text(cx);
        cx.notify();
    }
    fn sync_text(&mut self, cx: &mut Context<Self>) {
        let text = self.options.format(self.value);
        if self.draft(cx).as_ref() != text || self.input.read(cx).is_composing() {
            self.input.update(cx, |input, cx| input.set_value(text, cx));
        }
    }
}
