//! Provides a [calloop] event source from [XDG Desktop Portal] events
//!
//! This module uses the [ashpd] crate

use ashpd::desktop::settings::{ColorScheme, Settings};
use ashpd::zvariant::OwnedValue;
use calloop::channel::Channel;
use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use smol::stream::StreamExt;
use std::time::Duration;

use gpui::{BackgroundExecutor, WindowAppearance};

pub enum Event {
    DoubleClickInterval(Duration),
    WindowAppearance(WindowAppearance),
    #[cfg_attr(feature = "x11", allow(dead_code))]
    CursorTheme(String),
    #[cfg_attr(feature = "x11", allow(dead_code))]
    CursorSize(u32),
    ButtonLayout(String),
}

pub struct XDPEventSource {
    channel: Channel<Event>,
}

impl XDPEventSource {
    pub fn new(executor: &BackgroundExecutor) -> Self {
        let (sender, channel) = calloop::channel::channel();

        let background = executor.clone();

        executor
            .spawn(async move {
                let settings = Settings::new().await?;

                let click_sender = sender.clone();
                background
                    .spawn(async move { watch_double_click_interval(click_sender).await })
                    .detach();

                if let Ok(initial_appearance) = settings.color_scheme().await {
                    sender.send(Event::WindowAppearance(
                        window_appearance_from_color_scheme(initial_appearance),
                    ))?;
                }
                if let Ok(initial_theme) = settings
                    .read::<String>("org.gnome.desktop.interface", "cursor-theme")
                    .await
                {
                    sender.send(Event::CursorTheme(initial_theme))?;
                }

                // If u32 is used here, it throws invalid type error
                if let Ok(initial_size) = settings
                    .read::<i32>("org.gnome.desktop.interface", "cursor-size")
                    .await
                {
                    sender.send(Event::CursorSize(initial_size as u32))?;
                }

                if let Ok(initial_layout) = settings
                    .read::<String>("org.gnome.desktop.wm.preferences", "button-layout")
                    .await
                {
                    sender.send(Event::ButtonLayout(initial_layout))?;
                }

                if let Ok(mut cursor_theme_changed) = settings
                    .receive_setting_changed_with_args(
                        "org.gnome.desktop.interface",
                        "cursor-theme",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(theme) = cursor_theme_changed.next().await {
                                let theme = theme?;
                                sender.send(Event::CursorTheme(theme))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                if let Ok(mut cursor_size_changed) = settings
                    .receive_setting_changed_with_args::<i32>(
                        "org.gnome.desktop.interface",
                        "cursor-size",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(size) = cursor_size_changed.next().await {
                                let size = size?;
                                sender.send(Event::CursorSize(size as u32))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                if let Ok(mut button_layout_changed) = settings
                    .receive_setting_changed_with_args(
                        "org.gnome.desktop.wm.preferences",
                        "button-layout",
                    )
                    .await
                {
                    let sender = sender.clone();
                    background
                        .spawn(async move {
                            while let Some(layout) = button_layout_changed.next().await {
                                let layout = layout?;
                                sender.send(Event::ButtonLayout(layout))?;
                            }
                            anyhow::Ok(())
                        })
                        .detach();
                }

                let mut appearance_changed = settings.receive_color_scheme_changed().await?;
                while let Some(scheme) = appearance_changed.next().await {
                    sender.send(Event::WindowAppearance(
                        window_appearance_from_color_scheme(scheme),
                    ))?;
                }

                anyhow::Ok(())
            })
            .detach();

        Self { channel }
    }
}

struct ClickInterval(Duration);

impl TryFrom<OwnedValue> for ClickInterval {
    type Error = ashpd::zvariant::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        let millis = value
            .downcast_ref::<i32>()
            .ok()
            .or_else(|| {
                value
                    .downcast_ref::<&str>()
                    .ok()?
                    .trim()
                    .parse::<i32>()
                    .ok()
            })
            .filter(|millis| *millis > 0)
            .ok_or(ashpd::zvariant::Error::IncorrectType)?;
        Ok(Self(Duration::from_millis(millis as u64)))
    }
}

async fn watch_double_click_interval(
    sender: calloop::channel::Sender<Event>,
) -> anyhow::Result<()> {
    let settings = Settings::new().await?;
    // Subscribe before reading so changes during initialization remain queued.
    let changes = settings.receive_setting_changed().await.ok();
    let mut keys = [
        ("org.gnome.desktop.peripherals.mouse", "double-click"),
        ("org.kde.kdeglobals.KDE", "DoubleClickInterval"),
    ];
    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|desktop| desktop.eq_ignore_ascii_case("KDE"))
    {
        keys.swap(0, 1);
    }
    let mut intervals = [None, None];
    for (index, (namespace, key)) in keys.iter().enumerate() {
        intervals[index] = settings
            .read::<ClickInterval>(namespace, key)
            .await
            .ok()
            .map(|value| value.0);
    }
    let mut current = intervals
        .into_iter()
        .flatten()
        .next()
        .unwrap_or(super::platform::DOUBLE_CLICK_INTERVAL);
    sender.send(Event::DoubleClickInterval(current))?;
    if let Some(mut changes) = changes {
        while let Some(setting) = changes.next().await {
            let Some(index) = keys.iter().position(|(namespace, key)| {
                *namespace == setting.namespace() && *key == setting.key()
            }) else {
                continue;
            };
            intervals[index] = setting
                .value()
                .try_clone()
                .ok()
                .and_then(|value| ClickInterval::try_from(value).ok())
                .map(|value| value.0);
            let interval = intervals
                .into_iter()
                .flatten()
                .next()
                .unwrap_or(super::platform::DOUBLE_CLICK_INTERVAL);
            if interval != current {
                current = interval;
                sender.send(Event::DoubleClickInterval(interval))?;
            }
        }
    }
    Ok(())
}

impl EventSource for XDPEventSource {
    type Event = Event;
    type Metadata = ();
    type Ret = ();
    type Error = anyhow::Error;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.channel.process_events(readiness, token, |evt, _| {
            if let calloop::channel::Event::Msg(msg) = evt {
                (callback)(msg, &mut ())
            }
        })?;

        Ok(PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.channel.register(poll, token_factory)?;

        Ok(())
    }

    fn reregister(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.channel.reregister(poll, token_factory)?;

        Ok(())
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        self.channel.unregister(poll)?;

        Ok(())
    }
}

fn window_appearance_from_color_scheme(cs: ColorScheme) -> WindowAppearance {
    match cs {
        ColorScheme::PreferDark => WindowAppearance::Dark,
        ColorScheme::PreferLight => WindowAppearance::Light,
        ColorScheme::NoPreference => WindowAppearance::Light,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_values_accept_portal_encodings_and_reject_invalid_intervals() {
        use ashpd::zvariant::Value;
        let gnome = ClickInterval::try_from(OwnedValue::from(750i32)).unwrap();
        let kde = ClickInterval::try_from(Value::from(" 750 ").try_to_owned().unwrap()).unwrap();
        assert_eq!(gnome.0, kde.0);
        assert_eq!(gnome.0, Duration::from_millis(750));
        for value in [
            Value::from(0i32),
            Value::from(-2i32),
            Value::from("-2"),
            Value::from("0"),
            Value::from("invalid"),
            Value::from("2147483648"),
            Value::from(true),
        ] {
            assert!(ClickInterval::try_from(value.try_to_owned().unwrap()).is_err());
        }
    }
}
