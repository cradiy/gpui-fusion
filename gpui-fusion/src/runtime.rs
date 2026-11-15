use crate::theme::{StyleError, ThemeStore};
use gpui::Context;

/// An observable theme shared by component entities.
pub struct ThemeRuntime {
    store: ThemeStore,
}

impl ThemeRuntime {
    pub fn new(store: ThemeStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &ThemeStore {
        &self.store
    }

    pub fn set_mode(
        &mut self,
        mode: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        if self.store.set_mode(mode)? {
            cx.notify();
        }
        Ok(())
    }

    pub fn replace_json(&mut self, json: &str, cx: &mut Context<Self>) -> Result<(), StyleError> {
        self.store.replace_json(json)?;
        cx.notify();
        Ok(())
    }
}
