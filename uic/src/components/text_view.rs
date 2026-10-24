use gpui::{
    ElementId, Hsla, InteractiveText, IntoElement, Refineable, RenderOnce, SharedString,
    StyleRefinement, Styled, StyledText, TextSelectionHandle, div, prelude::*,
};

/// Read-only text with selection and system clipboard support.
#[derive(IntoElement)]
pub struct TextView {
    id: ElementId,
    text: StyledText,
    selectable: bool,
    selection: Hsla,
    style: StyleRefinement,
    selection_handle: Option<TextSelectionHandle>,
}

impl TextView {
    /// Use a stable ID to preserve selection across renders.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self::styled(id, StyledText::new(text))
    }

    pub fn styled(id: impl Into<ElementId>, text: StyledText) -> Self {
        Self {
            id: id.into(),
            text,
            selectable: true,
            selection: gpui::rgba(0x4488ff55).into(),
            style: StyleRefinement::default(),
            selection_handle: None,
        }
    }

    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }
    pub fn selection_color(mut self, color: impl Into<Hsla>) -> Self {
        self.selection = color.into();
        self
    }

    /// Exposes read-only selection state and displayed range geometry.
    pub fn selection_handle(mut self, handle: &TextSelectionHandle) -> Self {
        self.selection_handle = Some(handle.clone());
        self
    }
}

impl Styled for TextView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TextView {
    fn render(self, _: &mut gpui::Window, _: &mut gpui::App) -> impl IntoElement {
        let mut surface = div().whitespace_normal();
        surface.style().refine(&self.style);
        surface.child(
            InteractiveText::new(self.id, self.text)
                .when_some(self.selection_handle, |text, handle| {
                    text.selection_handle(&handle)
                })
                .when(self.selectable, |text| text.selectable(self.selection)),
        )
    }
}
