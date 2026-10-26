use super::WebView;
use crate::backend::Input;
use gpui::*;
use std::ops::Range;

impl WebView {
    fn queue_composition(&mut self, text: String, commit: bool, cx: &mut Context<Self>) {
        let schedule = self.pending_composition.is_empty();
        let revision = self.signals.ime_revision.get();
        if self
            .pending_composition
            .last()
            .is_some_and(|(previous, _, committed)| *previous == revision && !committed)
        {
            self.pending_composition.pop();
        }
        self.pending_composition.push((revision, text, commit));
        if schedule {
            cx.spawn(async move |weak, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1))
                    .await;
                let _ = weak.update(cx, |this, cx| {
                    let pending = std::mem::take(&mut this.pending_composition);
                    if let Some(engine) = this.engine.borrow_mut().as_mut() {
                        for (revision, text, commit) in pending {
                            if this.signals.ime_revision.get() != revision {
                                continue;
                            }
                            let commit = commit || text.is_empty();
                            engine.input(Input::Composition { text, commit });
                        }
                    }
                    this.poll(cx);
                });
            })
            .detach();
        }
    }
}

impl EntityInputHandler for WebView {
    fn text_input_mode(&self, _: &mut Window, _: &mut Context<Self>) -> TextInputMode {
        self.signals
            .ime
            .borrow()
            .as_ref()
            .map(|ime| ime.mode)
            .unwrap_or_default()
    }
    fn text_input_purpose(&self, _: &mut Window, _: &mut Context<Self>) -> TextInputPurpose {
        self.signals
            .ime
            .borrow()
            .as_ref()
            .map(|ime| ime.purpose)
            .unwrap_or_default()
    }
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let units: Vec<_> = self
            .marked_text
            .as_deref()
            .unwrap_or_default()
            .encode_utf16()
            .collect();
        let text = String::from_utf16(units.get(range.clone())?).ok()?;
        *adjusted = Some(range);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.signals.ime.borrow().as_ref()?;
        let end = self
            .marked_text
            .as_deref()
            .unwrap_or_default()
            .encode_utf16()
            .count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        Some(0..self.marked_text.as_ref()?.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.marked_text.take() {
            self.queue_composition(text, true, cx);
        }
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = None;
        self.queue_composition(text.into(), true, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = (!text.is_empty()).then(|| text.into());
        self.queue_composition(text.into(), false, cx);
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let ime = self.signals.ime.borrow();
        let ime = ime.as_ref()?;
        let scale = window.scale_factor();
        Some(Bounds::new(
            bounds.origin + ime.bounds.origin / scale,
            ime.bounds.size / scale,
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
