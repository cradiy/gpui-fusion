#![cfg_attr(target_family = "wasm", no_main)]

fn run_example() {
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    example::main();

    #[cfg(not(all(target_os = "linux", feature = "wayland")))]
    panic!("This example requires the `wayland` feature and a linux system.");
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod example {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use gpui::{
        App, Bounds, Context, FocusHandle, FontWeight, Size, Window, WindowBackgroundAppearance,
        WindowBounds, WindowKind, WindowOptions, div, layer_shell::*, point, prelude::*, px, rems,
        rgba, white,
    };
    use gpui_platform::application;

    struct LayerShellExample {
        mode: KeyboardInteractivity,
        focus: FocusHandle,
        keys: usize,
    }

    impl LayerShellExample {
        fn new(cx: &mut Context<Self>) -> Self {
            cx.spawn(async move |this, cx| {
                loop {
                    let _ = this.update(cx, |_, cx| cx.notify());
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                }
            })
            .detach();

            LayerShellExample {
                mode: KeyboardInteractivity::None,
                focus: cx.focus_handle(),
                keys: 0,
            }
        }
    }

    impl Render for LayerShellExample {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let hours = (now / 3600) % 24;
            let minutes = (now / 60) % 60;
            let seconds = now % 60;

            div()
                .size_full()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|this, _, _, cx| {
                    this.keys += 1;
                    cx.notify();
                }))
                .flex()
                .flex_col()
                .gap_2()
                .items_center()
                .justify_center()
                .text_size(rems(4.5))
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(white())
                .bg(rgba(0x0000044))
                .rounded_xl()
                .child(format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .text_size(px(14.))
                        .children(
                            [
                                KeyboardInteractivity::None,
                                KeyboardInteractivity::OnDemand,
                                KeyboardInteractivity::Exclusive,
                            ]
                            .into_iter()
                            .enumerate()
                            .map(|(index, mode)| {
                                div()
                                    .id(("keyboard-mode", index))
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgba(if self.mode == mode {
                                        0x3974aaff
                                    } else {
                                        0x263344ff
                                    }))
                                    .cursor_pointer()
                                    .child(format!("{mode:?}"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.mode = mode;
                                        this.focus.focus(window, cx);
                                        window.set_keyboard_interactivity(mode);
                                        cx.notify();
                                    }))
                            }),
                        )
                        .child(
                            div()
                                .id("remap")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgba(0x263344ff))
                                .cursor_pointer()
                                .child("Hide for 1s")
                                .on_click(cx.listener(|_, _, window, cx| {
                                    if let Err(error) = window.set_mapped(false) {
                                        eprintln!("Cannot hide layer surface: {error}");
                                        return;
                                    }
                                    cx.spawn_in(window, async move |_, cx| {
                                        cx.background_executor()
                                            .timer(Duration::from_secs(1))
                                            .await;
                                        if let Err(error) = cx
                                            .update(|window, _| window.set_mapped(true))
                                            .and_then(|result| result)
                                        {
                                            eprintln!("Cannot show layer surface: {error}");
                                        }
                                    })
                                    .detach();
                                })),
                        )
                        .child(
                            div()
                                .id("close")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgba(0x263344ff))
                                .cursor_pointer()
                                .child("Close")
                                .on_click(|_, window, _| window.remove_window()),
                        ),
                )
                .child(div().text_size(px(14.)).child(format!(
                    "Requested: {:?} · Focused: {} · Keys received: {}",
                    self.mode,
                    window.is_window_active(),
                    self.keys
                )))
        }
    }

    pub fn main() {
        application().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: Size::new(px(500.), px(200.)),
                    })),
                    app_id: Some("gpui-layer-shell-example".to_string()),
                    window_background: WindowBackgroundAppearance::Transparent,
                    kind: WindowKind::LayerShell(LayerShellOptions {
                        namespace: "gpui".to_string(),
                        anchor: Anchor::LEFT | Anchor::RIGHT | Anchor::BOTTOM,
                        margin: Some((px(0.), px(0.), px(40.), px(0.))),
                        keyboard_interactivity: KeyboardInteractivity::None,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(LayerShellExample::new),
            )
            .unwrap();
        });
    }
}
