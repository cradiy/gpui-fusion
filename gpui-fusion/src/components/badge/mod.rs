mod interaction;
mod motion;
mod paint;

use interaction::{BadgeInteraction, CaptureToken, DragPhase};
use motion::{DragState, length};
use std::rc::Rc;

use crate::{
    components::selection_surface::{color, selection_surface},
    style::{ComputedStyle, MaterialMode, Part, StateSelector, StylePatch, Styled},
    theme::{
        ResolveContext, StyleError, ThemeScope, material_binding::MaterialBinding,
        runtime::ThemeRuntime,
    },
};
use gpui::{
    AnyElement, App, Context, CursorStyle, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, Refineable, Render, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled as _, Subscription, Window, div, point, px,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadgeContent {
    Count(u64),
    Dot,
    Text(SharedString),
}

#[derive(Clone, Copy, Debug)]
pub enum BadgePart {
    Label,
}
impl Part for BadgePart {
    fn name(&self) -> &str {
        match self {
            Self::Label => "label",
        }
    }
}

#[derive(Clone)]
struct Styles {
    revision: u64,
    material: MaterialMode,
    root: ComputedStyle,
    label: ComputedStyle,
}

type DismissCallback = Rc<dyn Fn(&mut Window, &mut App)>;

/// A count, dot, or short status label with optional drag-to-dismiss interaction.
pub struct Badge {
    label: SharedString,
    content: BadgeContent,
    limit: Option<u64>,
    show_zero: bool,
    dismissible: bool,
    threshold: Pixels,
    animated: bool,
    drag: DragState,
    capture: CaptureToken,
    on_dismiss: Option<DismissCallback>,
    theme: Entity<ThemeRuntime>,
    material: MaterialBinding,
    style: StylePatch,
    scopes: Vec<ThemeScope>,
    variant: Option<String>,
    cache: Option<Styles>,
    error: Option<StyleError>,
    _subscription: Subscription,
}
impl Badge {
    pub fn new(
        label: impl Into<SharedString>,
        content: BadgeContent,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = ThemeRuntime::global(cx);
        let subscription = cx.observe(&theme, |_, _, cx| cx.notify());
        Self {
            label: label.into(),
            content,
            limit: Some(99),
            show_zero: false,
            dismissible: false,
            threshold: px(72.),
            animated: true,
            drag: DragState::default(),
            capture: CaptureToken::default(),
            on_dismiss: None,
            theme,
            material: MaterialBinding::new(cx),
            style: StylePatch::default(),
            scopes: Vec::new(),
            variant: None,
            cache: None,
            error: None,
            _subscription: subscription,
        }
    }
    pub fn dismissible(mut self, enabled: bool) -> Self {
        self.dismissible = enabled;
        self
    }
    pub fn dismiss_threshold(mut self, threshold: Pixels) -> Self {
        self.threshold = if f32::from(threshold).is_finite() {
            threshold.max(px(1.))
        } else {
            px(72.)
        };
        self
    }
    pub fn animated(mut self, enabled: bool) -> Self {
        self.animated = enabled;
        self
    }
    pub fn on_dismiss(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(callback));
        self
    }
    pub fn is_dismissed(&self) -> bool {
        self.drag.is_dismissed()
    }
    pub fn is_dragging(&self) -> bool {
        self.drag.is_dragging()
    }
    /// Restores the badge at its anchor without changing its content or emitting a dismissal.
    pub fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.capture.release(window);
        self.drag.reset();
        cx.notify();
    }
    pub fn set_dismissible(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.dismissible != enabled {
            self.dismissible = enabled;
            if !enabled {
                self.reset(window, cx);
            }
            cx.notify();
        }
    }
    pub fn content(&self) -> &BadgeContent {
        &self.content
    }
    pub fn set_content(&mut self, content: BadgeContent, cx: &mut Context<Self>) {
        if self.content != content {
            self.content = content;
            cx.notify();
        }
    }
    /// Counts above the limit display `limit+`; `None` displays the complete count.
    pub fn count_limit(mut self, limit: Option<u64>) -> Self {
        self.limit = limit;
        self
    }
    pub fn show_zero(mut self, show: bool) -> Self {
        self.show_zero = show;
        self
    }
    pub fn set_show_zero(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_zero != show {
            self.show_zero = show;
            cx.notify();
        }
    }
    pub fn is_visible(&self) -> bool {
        !self.drag.is_dismissed() && self.content_visible()
    }
    fn content_visible(&self) -> bool {
        match &self.content {
            BadgeContent::Count(value) => *value != 0 || self.show_zero,
            BadgeContent::Text(text) => !text.is_empty(),
            BadgeContent::Dot => true,
        }
    }
    /// The visible text, excluding dots and hidden badges.
    pub fn displayed_text(&self) -> Option<SharedString> {
        if !self.is_visible() {
            return None;
        }
        match &self.content {
            BadgeContent::Count(value) => Some(match self.limit {
                Some(limit) if *value > limit => format!("{limit}+").into(),
                _ => value.to_string().into(),
            }),
            BadgeContent::Text(text) => Some(text.clone()),
            BadgeContent::Dot => None,
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
        if let Some(cache) = &self.cache {
            if cache.revision == runtime.revision() && cache.material == material {
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
        self.style.validate(&context, &["label"])?;
        let recipe = runtime.styles().recipe("badge")?;
        let resolve = |part| {
            recipe.resolve(
                self.variant.as_deref(),
                material,
                &self.style,
                &context,
                &StateSelector::default(),
                part,
            )
        };
        let styles = Styles {
            revision: runtime.revision(),
            material,
            root: resolve(None)?,
            label: resolve(Some("label"))?,
        };
        self.cache = Some(styles.clone());
        Ok(styles)
    }
}
impl Styled for Badge {
    type Part = BadgePart;
    fn style_mut(&mut self) -> &mut StylePatch {
        self.cache = None;
        &mut self.style
    }
}
impl Render for Badge {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.content_visible() {
            self.capture.release(window);
            self.drag.reset();
        }
        if !self.is_visible() {
            return div().size_0().into_any_element();
        }
        let styles = match self.resolve(cx) {
            Ok(styles) => {
                self.error = None;
                styles
            }
            Err(error) => {
                self.error = Some(error.clone());
                return div()
                    .child(format!("Badge style error: {error}"))
                    .into_any_element();
            }
        };
        let visual = self.drag.visual(cx.background_executor().now());
        if visual.active {
            window.request_animation_frame();
        }
        if let Some(id) = visual.completion {
            let entity = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                let callback = entity
                    .update(cx, |badge, cx| {
                        let dismissed = badge.drag.finish(id);
                        cx.notify();
                        dismissed.then(|| badge.on_dismiss.clone()).flatten()
                    })
                    .ok()
                    .flatten();
                if let Some(callback) = callback {
                    callback(window, cx);
                }
            });
        }
        let tether_color = color(&styles.root.background);
        let dot = self.content == BadgeContent::Dot;
        let mut body = div().flex().items_center().justify_center().flex_shrink_0();
        body = if dot {
            body.size(px(8.))
        } else {
            body.min_w(px(20.)).h(px(20.)).px(px(6.))
        };
        let mut root = selection_surface(body, styles.root.clone(), styles.root, true, 1., 1., 0.)
            .id("badge")
            .debug_selector(|| "fusion-badge".into())
            .role(gpui::Role::Label)
            .aria_label(self.label.clone());
        if let Some(text) = self.displayed_text() {
            let mut label = div().line_height(px(16.)).whitespace_nowrap();
            label.style().refine(&styles.label.to_gpui());
            root = root.child(label.child(text.clone()));
            root = root.aria_value(match &self.content {
                BadgeContent::Count(value) => SharedString::from(value.to_string()),
                _ => text,
            });
        }
        if self.dismissible {
            let entity = cx.weak_entity();
            root = root
                .cursor(if self.is_dragging() {
                    CursorStyle::ClosedHand
                } else {
                    CursorStyle::OpenHand
                })
                .child(
                    BadgeInteraction::new(
                        self.capture.clone(),
                        move |position, phase, window, cx| {
                            let _ = entity.update(cx, |badge, cx| {
                                let now = cx.background_executor().now();
                                let animated = badge.animated && !window.prefers_reduced_motion();
                                match phase {
                                    DragPhase::Start => badge.drag.start(position, now),
                                    DragPhase::Move => badge.drag.move_to(position, now),
                                    DragPhase::End => {
                                        badge.drag.release(position, badge.threshold, now, animated)
                                    }
                                    DragPhase::Cancel => badge.drag.cancel(now, animated),
                                }
                                cx.notify();
                            });
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
        }
        let threshold = self.threshold;
        let tethered = self.dismissible
            && !visual.dismissing
            && length(visual.offset) > 0.5
            && length(visual.offset) < f32::from(threshold);
        let mut moving = div()
            .relative()
            .flex()
            .flex_shrink_0()
            .left(visual.offset.x)
            .top(visual.offset.y)
            .opacity(visual.opacity);
        if tethered {
            moving = moving.on_paint_before_children(move |bounds, _, window, _| {
                paint::paint_elastic_tether(bounds, visual.offset, threshold, tether_color, window);
            });
        }
        moving.child(root).into_any_element()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeCorner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Attaches a badge without adding it to the content's layout or focus order.
#[derive(IntoElement)]
pub struct Badged {
    content: AnyElement,
    badge: Entity<Badge>,
    corner: BadgeCorner,
    offset: Point<Pixels>,
}
impl Badged {
    pub fn new(content: impl IntoElement, badge: Entity<Badge>) -> Self {
        Self {
            content: content.into_any_element(),
            badge,
            corner: BadgeCorner::TopRight,
            offset: point(px(-6.), px(-6.)),
        }
    }
    pub fn corner(mut self, corner: BadgeCorner) -> Self {
        self.corner = corner;
        self
    }
    /// Insets from the selected edges. Negative values move the badge outward.
    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = offset;
        self
    }
}
impl RenderOnce for Badged {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let marker = div().absolute().child(self.badge);
        let marker = match self.corner {
            BadgeCorner::TopLeft => marker.top(self.offset.y).left(self.offset.x),
            BadgeCorner::TopRight => marker.top(self.offset.y).right(self.offset.x),
            BadgeCorner::BottomLeft => marker.bottom(self.offset.y).left(self.offset.x),
            BadgeCorner::BottomRight => marker.bottom(self.offset.y).right(self.offset.x),
        };
        div()
            .relative()
            .flex()
            .flex_shrink_0()
            .child(self.content)
            .child(marker)
    }
}

#[cfg(test)]
mod tests;
