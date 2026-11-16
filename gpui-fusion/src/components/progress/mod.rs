mod paint;

use crate::{
    components::selection_surface::length,
    style::{ComputedStyle, MaterialMode, Part, State, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{
    App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Refineable, Render,
    SharedString, StatefulInteractiveElement, Styled as _, Subscription, Window, canvas, div, px,
};
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProgressShape {
    #[default]
    Bar,
    Ring,
}
#[derive(Clone, Copy, Debug)]
pub enum ProgressPart {
    Track,
    Fill,
    Label,
}
impl Part for ProgressPart {
    fn name(&self) -> &str {
        match self {
            Self::Track => "track",
            Self::Fill => "fill",
            Self::Label => "label",
        }
    }
}
#[derive(Clone)]
struct Styles {
    revision: u64,
    material: MaterialMode,
    indeterminate: bool,
    root: ComputedStyle,
    track: ComputedStyle,
    fill: ComputedStyle,
    label: ComputedStyle,
}

/// A non-interactive progress bar or ring. `None` represents indeterminate work.
pub struct Progress {
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    label: SharedString,
    value: Option<f32>,
    shape: ProgressShape,
    show_value: bool,
    animated: bool,
    started: Instant,
    cache: Option<Styles>,
    paths: Rc<RefCell<paint::Paths>>,
    error: Option<StyleError>,
    _subscription: Subscription,
}
impl Progress {
    /// Finite values are clamped to `0..=1`; non-finite values are rejected.
    pub fn new(
        label: impl Into<SharedString>,
        value: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Result<Self, StyleError> {
        let value = normalize(value)?;
        let theme = ThemeRuntime::global(cx);
        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
        Ok(Self {
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            label: label.into(),
            value,
            shape: ProgressShape::Bar,
            show_value: false,
            animated: true,
            started: Instant::now(),
            cache: None,
            paths: Default::default(),
            error: None,
            _subscription: subscription,
        })
    }
    pub fn value(&self) -> Option<f32> {
        self.value
    }
    pub fn set_value(
        &mut self,
        value: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Result<(), StyleError> {
        let value = normalize(value)?;
        if self.value != value {
            if self.value.is_some() && value.is_none() {
                self.started = Instant::now();
            }
            self.value = value;
            cx.notify();
        }
        Ok(())
    }
    pub fn shape(mut self, shape: ProgressShape) -> Self {
        self.shape = shape;
        self
    }
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }
    pub fn set_animated(&mut self, animated: bool, cx: &mut Context<Self>) {
        if self.animated != animated {
            self.animated = animated;
            self.started = Instant::now();
            cx.notify();
        }
    }
    pub fn material_mode(mut self, mode: MaterialMode) -> Self {
        self.material.fix(mode);
        self
    }
    pub fn set_material_mode(&mut self, mode: Option<MaterialMode>, cx: &mut Context<Self>) {
        self.material.set(mode, cx);
    }
    pub fn scope(mut self, scope: ThemeScope) -> Self {
        self.scopes.push(scope);
        self.cache = None;
        self
    }
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.variant = Some(name.into());
        self.cache = None;
        self
    }
    pub fn set_variant(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        self.variant = name;
        self.cache = None;
        cx.notify();
    }
    pub fn set_style(&mut self, style: StylePatch, cx: &mut Context<Self>) {
        self.style = style;
        self.cache = None;
        cx.notify();
    }
    pub fn style_error(&self) -> Option<&StyleError> {
        self.error.as_ref()
    }
    fn resolve(&mut self, cx: &App) -> Result<Styles, StyleError> {
        let runtime = self.theme.read(cx);
        let material = self.material.mode(cx);
        let indeterminate = self.value.is_none();
        if let Some(cache) = &self.cache {
            if cache.revision == runtime.revision()
                && cache.material == material
                && cache.indeterminate == indeterminate
            {
                return Ok(cache.clone());
            }
        }
        let mut context = ResolveContext::new(
            runtime.store().theme(),
            runtime.store().mode(),
            runtime.style_set(),
        )?;
        for scope in &self.scopes {
            context = context.scoped(scope)?;
        }
        self.style.validate(&context, &["track", "fill", "label"])?;
        let recipe = runtime.styles().recipe("progress")?;
        let states = if indeterminate {
            State::Indeterminate.into()
        } else {
            StateSelector::default()
        };
        let resolve = |part| {
            recipe.resolve(
                self.variant.as_deref(),
                material,
                &self.style,
                &context,
                &states,
                part,
            )
        };
        let styles = Styles {
            revision: runtime.revision(),
            material,
            indeterminate,
            root: resolve(None)?,
            track: resolve(Some("track"))?,
            fill: resolve(Some("fill"))?,
            label: resolve(Some("label"))?,
        };
        self.cache = Some(styles.clone());
        Ok(styles)
    }
}
fn normalize(value: Option<f32>) -> Result<Option<f32>, StyleError> {
    value
        .map(|v| {
            if v.is_finite() {
                Ok(v.clamp(0., 1.))
            } else {
                Err(StyleError::new("progress.value", "expected a finite value"))
            }
        })
        .transpose()
}
impl Styled for Progress {
    type Part = ProgressPart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Render for Progress {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let styles = match self.resolve(cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Progress style error: {error}"))
                    .into_any_element();
            }
        };
        let ring = self.shape == ProgressShape::Ring;
        let width = length(&styles.root.width, if ring { 56. } else { 240. });
        let height = length(&styles.root.height, if ring { 56. } else { 24. });
        let value = self.value;
        let shape = self.shape;
        let started = self.started;
        let animated = self.animated;
        let track = styles.track;
        let fill = styles.fill;
        let paths = self.paths.clone();
        let graphic = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if bounds.intersect(&window.content_mask().bounds).is_empty() {
                    return;
                }
                let phase = if animated {
                    (started.elapsed().as_secs_f64() / 1.4).fract() as f32
                } else {
                    0.25
                };
                paint::Frame {
                    bounds,
                    shape,
                    value,
                    phase,
                    track: &track,
                    fill: &fill,
                }
                .paint(&mut paths.borrow_mut(), window);
                if value.is_none() && animated {
                    window.request_animation_frame();
                }
            },
        );
        let mut root = div()
            .id("progress")
            .debug_selector(|| "fusion-progress".into())
            .role(gpui::Role::ProgressIndicator)
            .aria_label(self.label.clone())
            .aria_min_numeric_value(0.)
            .aria_max_numeric_value(1.)
            .flex()
            .items_center()
            .justify_center()
            .relative();
        root.style().refine(&styles.root.to_gpui());
        root = root.w(width).h(height);
        if let Some(value) = value {
            root = root
                .aria_numeric_value(f64::from(value))
                .aria_value(format!("{:.0}%", value * 100.));
        } else {
            root = root.aria_value("In progress");
        }
        root = root.child(if ring {
            graphic.absolute().inset_0().into_any_element()
        } else {
            graphic.flex_1().min_w_0().h_full().into_any_element()
        });
        if self.show_value {
            if let Some(value) = value {
                let mut label = div()
                    .id("progress-value")
                    .debug_selector(|| "fusion-progress-value".into())
                    .flex_none()
                    .child(format!("{:.0}%", value * 100.));
                label.style().refine(&styles.label.to_gpui());
                if !ring {
                    label = label.w(px(42.)).text_right();
                }
                root = root.child(label);
            } else if !ring {
                root = root.child(div().w(px(42.)).flex_none());
            }
        }
        root.into_any_element()
    }
}

#[cfg(test)]
mod tests;
