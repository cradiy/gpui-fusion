use gpui::{
    App, ElementId, Entity, IntoElement, RenderOnce, SharedString, StyleRefinement, Styled, Window,
    px,
};

use super::{Select, SelectAppearance, SelectItemState, SelectMenu, SelectOption, SelectState};

/// A searchable multiple-selection dropdown with wrapping, removable labels.
#[derive(IntoElement)]
pub struct MultiSelect(Select);

impl MultiSelect {
    /// Uses a state created with `SelectState::multiple`.
    pub fn new(id: impl Into<ElementId>, state: &Entity<SelectState>) -> Self {
        let mut inner = Select::new(id, state)
            .placeholder("Select options")
            .label("Choose options")
            .h_auto()
            .min_h(px(44.))
            .px_2()
            .py(px(6.));
        inner.multiple = true;
        Self(inner)
    }
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.0 = self.0.label(label);
        self
    }
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.0 = self.0.placeholder(text);
        self
    }
    pub fn search_placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.0 = self.0.search_placeholder(text);
        self
    }
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.0 = self.0.searchable(searchable);
        self
    }
    pub fn clearable(mut self, clearable: bool) -> Self {
        self.0 = self.0.clearable(clearable);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.0 = self.0.disabled(disabled);
        self
    }
    pub fn menu(mut self, menu: SelectMenu) -> Self {
        self.0 = self.0.menu(menu);
        self
    }
    pub fn appearance(mut self, appearance: SelectAppearance) -> Self {
        self.0 = self.0.appearance(appearance);
        self
    }
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.0 = self.0.empty_text(text);
        self
    }
    pub fn render_option<E: IntoElement>(
        mut self,
        render: impl Fn(&SelectOption, SelectItemState, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.0 = self.0.render_option(render);
        self
    }
}

impl Styled for MultiSelect {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for MultiSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0.render(window, cx)
    }
}
