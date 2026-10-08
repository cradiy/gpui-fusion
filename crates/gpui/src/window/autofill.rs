use super::Window;
use crate::{
    App, AutofillField, AutofillOptions, Bounds, FocusHandle, FocusId, Pixels, PointerMapping,
    SharedString,
};
use slotmap::Key;
use std::rc::Rc;

type Fill = Rc<dyn Fn(String, &mut Window, &mut App)>;

#[derive(Clone)]
pub(super) struct AutofillEntry {
    focus: FocusId,
    options: AutofillOptions,
    value: SharedString,
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    pub mapping: PointerMapping,
    fill: Fill,
}

impl Window {
    /// Whether this window's platform supports system autofill.
    pub fn supports_autofill(&self) -> bool {
        self.platform_window.supports_autofill()
    }

    /// Registers an editable field during paint, including when it is not focused.
    /// Use a stable focus handle and a weak entity reference in `fill`. Omit disabled
    /// fields. Bounds are mapped through the current transform and content mask.
    pub fn handle_autofill(
        &mut self,
        focus: &FocusHandle,
        options: AutofillOptions,
        value: SharedString,
        bounds: Bounds<Pixels>,
        fill: impl Fn(String, &mut Window, &mut App) + 'static,
    ) {
        self.invalidator.debug_assert_paint();
        if !self.supports_autofill() || options.name.is_empty() {
            return;
        }
        self.next_frame.autofill.push(AutofillEntry {
            focus: focus.id,
            options,
            value,
            bounds,
            clip: self.content_mask().bounds,
            mapping: self.pointer_mapping.clone(),
            fill: Rc::new(fill),
        });
    }

    /// Finishes a successful form submission and allows the system to offer saving.
    /// Call after the application accepts the form, not when a field loses focus.
    pub fn commit_autofill(&mut self) -> anyhow::Result<()> {
        self.platform_window.finish_autofill(true)
    }

    /// Discards the system autofill session, for example when a form is reset.
    pub fn cancel_autofill(&mut self) -> anyhow::Result<()> {
        self.platform_window.finish_autofill(false)
    }

    pub(super) fn publish_autofill(&self) {
        if !self.supports_autofill() {
            return;
        }
        let fields = self
            .rendered_frame
            .autofill
            .iter()
            .filter_map(|entry| {
                let bounds = entry
                    .mapping
                    .visible_bounds_to_display(entry.bounds, entry.clip)?;
                Some(AutofillField {
                    id: entry.focus.data().as_ffi(),
                    name: entry.options.name.clone(),
                    hint: entry.options.hint,
                    value: entry.value.clone(),
                    bounds,
                    focused: self.focus == Some(entry.focus),
                })
            })
            .collect();
        self.platform_window.set_autofill_fields(fields);
    }

    pub(super) fn focus_autofill(&mut self, id: u64, cx: &mut App) {
        debug_assert!(self.invalidator.not_drawing());
        if let Some(entry) = self
            .rendered_frame
            .autofill
            .iter()
            .find(|entry| entry.focus.data().as_ffi() == id)
            && let Some(handle) = FocusHandle::for_id(entry.focus, &cx.focus_handles)
        {
            self.focus(&handle, cx);
        }
    }

    pub(super) fn apply_autofill(&mut self, id: u64, value: String, cx: &mut App) {
        debug_assert!(self.invalidator.not_drawing());
        let fill = self
            .rendered_frame
            .autofill
            .iter()
            .find(|entry| entry.focus.data().as_ffi() == id)
            .map(|entry| entry.fill.clone());
        if let Some(fill) = fill {
            fill(value, self, cx);
        }
    }
}
