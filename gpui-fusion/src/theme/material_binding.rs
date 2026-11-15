use crate::style::MaterialMode;
use gpui::{App, AppContext, Context, Entity, Global, Subscription};

struct InjectedMaterial(Entity<MaterialRuntime>);
impl Global for InjectedMaterial {}

/// Observable application material preference, independent of theme updates.
pub(crate) struct MaterialRuntime {
    mode: MaterialMode,
}
impl MaterialRuntime {
    pub(crate) fn init(cx: &mut App) {
        if !cx.has_global::<InjectedMaterial>() {
            let entity = cx.new(|_| Self {
                mode: MaterialMode::Plain,
            });
            cx.set_global(InjectedMaterial(entity));
        }
    }
    pub(crate) fn global(cx: &App) -> Entity<Self> {
        cx.global::<InjectedMaterial>().0.clone()
    }
    pub(crate) fn mode(&self) -> MaterialMode {
        self.mode
    }
    pub(crate) fn set_mode(&mut self, mode: MaterialMode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            cx.notify();
        }
    }
}

pub(crate) struct MaterialBinding {
    runtime: Entity<MaterialRuntime>,
    fixed: Option<MaterialMode>,
    subscription: Option<Subscription>,
}
impl MaterialBinding {
    pub(crate) fn new<T: 'static>(cx: &mut Context<T>) -> Self {
        let runtime = MaterialRuntime::global(cx);
        let subscription = Some(cx.observe(&runtime, |_, _, cx| cx.notify()));
        Self {
            runtime,
            fixed: None,
            subscription,
        }
    }
    pub(crate) fn mode(&self, cx: &App) -> MaterialMode {
        self.fixed.unwrap_or_else(|| self.runtime.read(cx).mode())
    }
    pub(crate) fn fix(&mut self, mode: MaterialMode) {
        self.fixed = Some(mode);
        self.subscription = None;
    }
    pub(crate) fn set<T: 'static>(&mut self, mode: Option<MaterialMode>, cx: &mut Context<T>) {
        if mode == self.fixed {
            return;
        }
        self.fixed = mode;
        self.subscription = if mode.is_none() {
            Some(cx.observe(&self.runtime, |_, _, cx| cx.notify()))
        } else {
            None
        };
        cx.notify();
    }
}
