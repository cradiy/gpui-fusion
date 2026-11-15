use crate::theme::{ResolveContext, StyleError, ThemeConfig, ThemeStore};
use crate::{
    default_styles,
    style::{ComponentStyles, MaterialMode},
};
use gpui::{App, AppContext, Context, Entity, Global};

struct InjectedTheme(Entity<ThemeRuntime>);
impl Global for InjectedTheme {}

/// The application's observable theme, installed in GPUI's context.
pub struct ThemeRuntime {
    store: ThemeStore,
    styles: ComponentStyles,
    style_set: String,
    revision: u64,
}

impl ThemeRuntime {
    /// Installs the application theme. Reinstallation updates the existing entity.
    pub fn install(
        store: ThemeStore,
        style_set: impl Into<String>,
        cx: &mut App,
    ) -> Result<Entity<Self>, StyleError> {
        let styles = cx
            .try_global::<InjectedTheme>()
            .map(|theme| theme.0.read(cx).styles.clone())
            .unwrap_or_else(default_styles);
        Self::install_with_styles(store, style_set, styles, cx)
    }

    pub fn install_with_styles(
        store: ThemeStore,
        style_set: impl Into<String>,
        styles: ComponentStyles,
        cx: &mut App,
    ) -> Result<Entity<Self>, StyleError> {
        let style_set = style_set.into();
        ResolveContext::new(store.theme(), store.mode(), &style_set)?;
        styles.validate(store.theme())?;
        crate::theme::material_binding::MaterialRuntime::init(cx);
        let material = store.theme().material;
        crate::theme::material_binding::MaterialRuntime::global(cx)
            .update(cx, |runtime, cx| runtime.set_mode(material, cx));
        if let Some(injected) = cx.try_global::<InjectedTheme>() {
            let entity = injected.0.clone();
            entity.update(cx, |this, cx| {
                this.store = store;
                this.styles = styles;
                this.style_set = style_set;
                this.revision = this.revision.wrapping_add(1);
                cx.notify();
            });
            return Ok(entity);
        }
        let entity = cx.new(|_| Self {
            store,
            styles,
            style_set,
            revision: 0,
        });
        cx.set_global(InjectedTheme(entity.clone()));
        Ok(entity)
    }

    pub fn styles(&self) -> &ComponentStyles {
        &self.styles
    }

    pub fn replace_styles_json(
        &mut self,
        source: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let styles = ComponentStyles::from_json(source)?;
        styles.validate(self.store.theme())?;
        self.styles = styles;
        self.revision = self.revision.wrapping_add(1);
        cx.notify();
        Ok(())
    }

    /// Returns the theme installed before constructing Fusion components.
    pub fn global(cx: &App) -> Entity<Self> {
        cx.try_global::<InjectedTheme>()
            .expect("call ThemeRuntime::install before creating Fusion components")
            .0
            .clone()
    }

    pub fn store(&self) -> &ThemeStore {
        &self.store
    }
    pub fn style_set(&self) -> &str {
        &self.style_set
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn set_style_set(
        &mut self,
        style_set: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let style_set = style_set.into();
        ResolveContext::new(self.store.theme(), self.store.mode(), &style_set)?;
        if self.style_set != style_set {
            self.style_set = style_set;
            self.revision = self.revision.wrapping_add(1);
            cx.notify();
        }
        Ok(())
    }

    pub fn set_mode(
        &mut self,
        mode: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        if self.store.set_mode(mode)? {
            self.revision = self.revision.wrapping_add(1);
            cx.notify();
        }
        Ok(())
    }

    pub fn material_mode(&self) -> MaterialMode {
        self.store.theme().material
    }

    pub fn set_material_mode(&mut self, mode: MaterialMode, cx: &mut Context<Self>) {
        if self.store.set_material_mode(mode) {
            crate::theme::material_binding::MaterialRuntime::global(cx)
                .update(cx, |runtime, cx| runtime.set_mode(mode, cx));
            cx.notify();
        }
    }

    pub fn configuration(&self) -> ThemeConfig {
        ThemeConfig {
            theme: self.store.theme().clone(),
            mode: self.store.mode().into(),
            style_set: self.style_set.clone(),
        }
    }

    pub fn install_file(
        path: impl AsRef<std::path::Path>,
        cx: &mut App,
    ) -> Result<Entity<Self>, StyleError> {
        Self::install_config(ThemeConfig::from_file(path)?, cx)
    }

    pub fn install_config(config: ThemeConfig, cx: &mut App) -> Result<Entity<Self>, StyleError> {
        Self::install(
            ThemeStore::new(config.theme, config.mode)?,
            config.style_set,
            cx,
        )
    }

    pub fn reload_file(
        &mut self,
        path: impl AsRef<std::path::Path>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        self.apply_config(ThemeConfig::from_file(path)?, cx)
    }

    pub fn replace_json(&mut self, json: &str, cx: &mut Context<Self>) -> Result<(), StyleError> {
        self.apply_config(ThemeConfig::from_json(json)?, cx)
    }

    pub fn apply_config(
        &mut self,
        config: ThemeConfig,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let store = ThemeStore::new(config.theme, config.mode)?;
        ResolveContext::new(store.theme(), store.mode(), &config.style_set)?;
        self.styles.validate(store.theme())?;
        let theme_changed = self.store.theme().modes != store.theme().modes
            || self.store.theme().style_sets != store.theme().style_sets
            || self.store.mode() != store.mode()
            || self.style_set != config.style_set;
        let material = store.theme().material;
        let material_changed = material != self.store.theme().material;
        self.store = store;
        self.style_set = config.style_set;
        if theme_changed {
            self.revision = self.revision.wrapping_add(1);
        }
        if theme_changed || material_changed {
            cx.notify();
        }
        crate::theme::material_binding::MaterialRuntime::global(cx)
            .update(cx, |runtime, cx| runtime.set_mode(material, cx));
        Ok(())
    }
}
