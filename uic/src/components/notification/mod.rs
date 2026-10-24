use std::time::Duration;

use gpui::{
    App, Context, Entity, FontWeight, Global, Hsla, IntoElement, Pixels, Refineable as _, Render,
    SharedString, StyleRefinement, Styled, Window, div, prelude::*, px, rgb, svg,
};

use gpui_icons::GpuiIcons;

const DEFAULT_DURATION: Duration = Duration::from_millis(4500);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationVariant {
    #[default]
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationPlacement {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, uic_macros::Chainable)]
pub struct NotificationColors {
    pub info: Hsla,
    pub success: Hsla,
    pub warn: Hsla,
    pub error: Hsla,
}

impl NotificationColors {
    fn color(self, variant: NotificationVariant) -> Hsla {
        match variant {
            NotificationVariant::Info => self.info,
            NotificationVariant::Success => self.success,
            NotificationVariant::Warn => self.warn,
            NotificationVariant::Error => self.error,
        }
    }
}

#[derive(Clone, Debug, uic_macros::Chainable)]
pub struct NotificationAppearance {
    pub colors: NotificationColors,
    pub description_foreground: Hsla,
    pub close_foreground: Hsla,
    pub gap: Pixels,
    #[chain(skip)]
    style: StyleRefinement,
    #[chain(skip)]
    viewport_margin: Pixels,
}

impl NotificationAppearance {
    pub fn viewport_margin(mut self, margin: Pixels) -> Self {
        self.viewport_margin = margin.max(px(0.));
        self
    }
}

impl Styled for NotificationAppearance {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Default for NotificationAppearance {
    fn default() -> Self {
        Self {
            colors: NotificationColors {
                info: rgb(0x1677ff).into(),
                success: rgb(0x52c41a).into(),
                warn: rgb(0xfaad14).into(),
                error: rgb(0xff4d4f).into(),
            },
            description_foreground: rgb(0x000000).opacity(0.65).into(),
            close_foreground: rgb(0x000000).opacity(0.45).into(),
            gap: px(12.),
            style: StyleRefinement::default()
                .w(px(384.))
                .p(px(20.))
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(0x000000).opacity(0.06))
                .bg(rgb(0xffffff))
                .text_color(rgb(0x000000).opacity(0.88))
                .shadow_lg(),
            viewport_margin: px(24.),
        }
    }
}

/// A notification card shown by the global notification layer.
pub struct Notification {
    title: SharedString,
    description: SharedString,
    variant: NotificationVariant,
    placement: Option<NotificationPlacement>,
    duration: Option<Duration>,
    closable: bool,
    style: StyleRefinement,
}

impl Notification {
    pub fn new(title: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            variant: NotificationVariant::Info,
            placement: None,
            duration: Some(DEFAULT_DURATION),
            closable: true,
            style: StyleRefinement::default(),
        }
    }

    pub fn variant(mut self, variant: NotificationVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn info(self) -> Self {
        self.variant(NotificationVariant::Info)
    }

    pub fn success(self) -> Self {
        self.variant(NotificationVariant::Success)
    }

    pub fn warn(self) -> Self {
        self.variant(NotificationVariant::Warn)
    }

    pub fn error(self) -> Self {
        self.variant(NotificationVariant::Error)
    }

    pub fn placement(mut self, placement: NotificationPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn persistent(mut self) -> Self {
        self.duration = None;
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

impl Styled for Notification {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct NotificationItem {
    id: u64,
    title: SharedString,
    description: SharedString,
    variant: NotificationVariant,
    placement: NotificationPlacement,
    closable: bool,
    style: StyleRefinement,
}

pub struct NotificationManager {
    items: Vec<NotificationItem>,
    next_id: u64,
    appearance: NotificationAppearance,
    placement: NotificationPlacement,
}

impl NotificationManager {
    fn new(appearance: NotificationAppearance) -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
            appearance,
            placement: NotificationPlacement::default(),
        }
    }

    fn push(&mut self, notification: Notification, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let duration = notification.duration;
        self.items.push(NotificationItem {
            id,
            title: notification.title,
            description: notification.description,
            variant: notification.variant,
            placement: notification.placement.unwrap_or(self.placement),
            closable: notification.closable,
            style: notification.style,
        });
        cx.notify();

        if let Some(duration) = duration {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                if let Some(this) = this.upgrade() {
                    this.update(cx, |manager, cx| manager.dismiss(id, cx));
                }
            })
            .detach();
        }
    }

    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.items.retain(|item| item.id != id);
        cx.notify();
    }

    fn render_stack(&self, placement: NotificationPlacement, cx: &mut Context<Self>) -> gpui::Div {
        let appearance = &self.appearance;
        div()
            .absolute()
            .when(
                matches!(
                    placement,
                    NotificationPlacement::TopLeft | NotificationPlacement::BottomLeft
                ),
                |this| this.left(appearance.viewport_margin),
            )
            .when(
                matches!(
                    placement,
                    NotificationPlacement::TopRight | NotificationPlacement::BottomRight
                ),
                |this| this.right(appearance.viewport_margin),
            )
            .when_else(
                matches!(
                    placement,
                    NotificationPlacement::TopLeft | NotificationPlacement::TopRight
                ),
                |this| this.top(appearance.viewport_margin).flex_col(),
                |this| this.bottom(appearance.viewport_margin).flex_col_reverse(),
            )
            .flex()
            .gap(appearance.gap)
            .children(
                self.items
                    .iter()
                    .filter(move |item| item.placement == placement)
                    .map(|item| self.render_item(item, cx)),
            )
    }

    fn render_item(&self, item: &NotificationItem, cx: &mut Context<Self>) -> gpui::AnyElement {
        let appearance = &self.appearance;
        let id = item.id;
        let mut card = div()
            .id(("notification", id as usize))
            .relative()
            .flex()
            .items_start()
            .gap_3()
            .child(status_icon(
                item.variant,
                appearance.colors.color(item.variant),
            ))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .pr_6()
                            .text_sm()
                            .line_height(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(22.))
                            .text_color(appearance.description_foreground)
                            .child(item.description.clone()),
                    ),
            )
            .when(item.closable, |this| {
                this.child(
                    div()
                        .id(("notification-close", id as usize))
                        .absolute()
                        .top(px(14.))
                        .right(px(14.))
                        .size_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .text_color(appearance.close_foreground)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x000000).opacity(0.04)))
                        .on_click(cx.listener(move |manager, _, _, cx| {
                            manager.dismiss(id, cx);
                            cx.stop_propagation();
                        }))
                        .child(
                            svg()
                                .path(GpuiIcons::LucideX)
                                .size_4()
                                .text_color(appearance.close_foreground),
                        ),
                )
            });
        card.style().refine(&appearance.style);
        card.style().refine(&item.style);
        card.into_any_element()
    }
}

impl Render for NotificationManager {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().absolute().inset_0().children([
            self.render_stack(NotificationPlacement::TopLeft, cx),
            self.render_stack(NotificationPlacement::TopRight, cx),
            self.render_stack(NotificationPlacement::BottomLeft, cx),
            self.render_stack(NotificationPlacement::BottomRight, cx),
        ])
    }
}

fn status_icon(variant: NotificationVariant, color: Hsla) -> impl IntoElement {
    let icon = match variant {
        NotificationVariant::Info => GpuiIcons::LucideInfo,
        NotificationVariant::Success => GpuiIcons::LucideCircleCheck,
        NotificationVariant::Warn => GpuiIcons::LucideCircleAlert,
        NotificationVariant::Error => GpuiIcons::LucideCircleX,
    };
    svg().path(icon).size_6().flex_none().text_color(color)
}

struct GlobalNotification(Entity<NotificationManager>);

impl Global for GlobalNotification {}

pub fn init(cx: &mut App) {
    init_with_appearance(NotificationAppearance::default(), cx);
}

pub fn init_with_appearance(appearance: NotificationAppearance, cx: &mut App) {
    if !cx.has_global::<GlobalNotification>() {
        let manager = cx.new(|_| NotificationManager::new(appearance));
        cx.set_global(GlobalNotification(manager));
    } else {
        set_appearance(appearance, cx);
    }
}

pub fn set_appearance(appearance: NotificationAppearance, cx: &mut App) {
    layer(cx).update(cx, |manager, cx| {
        manager.appearance = appearance;
        cx.notify();
    });
}

pub fn set_placement(placement: NotificationPlacement, cx: &mut App) {
    layer(cx).update(cx, |manager, cx| {
        manager.placement = placement;
        cx.notify();
    });
}

pub fn layer(cx: &App) -> Entity<NotificationManager> {
    cx.global::<GlobalNotification>().0.clone()
}

pub fn show(notification: Notification, cx: &mut App) {
    layer(cx).update(cx, |manager, cx| manager.push(notification, cx));
}

pub fn info(title: impl Into<SharedString>, description: impl Into<SharedString>, cx: &mut App) {
    show(Notification::new(title, description).info(), cx);
}

pub fn success(title: impl Into<SharedString>, description: impl Into<SharedString>, cx: &mut App) {
    show(Notification::new(title, description).success(), cx);
}

pub fn warn(title: impl Into<SharedString>, description: impl Into<SharedString>, cx: &mut App) {
    show(Notification::new(title, description).warn(), cx);
}

pub fn error(title: impl Into<SharedString>, description: impl Into<SharedString>, cx: &mut App) {
    show(Notification::new(title, description).error(), cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_exposes_top_level_styled_properties() {
        let notification = Notification::new("Title", "Description")
            .w(px(420.))
            .rounded(px(12.));
        assert!(notification.style.size.width.is_some());
        assert!(notification.style.corner_radii.top_left.is_some());
    }

    #[test]
    fn persistent_notification_has_no_timer() {
        assert!(
            Notification::new("Title", "Description")
                .persistent()
                .duration
                .is_none()
        );
    }
}
