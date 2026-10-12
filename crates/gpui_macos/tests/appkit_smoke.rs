//! Main-thread AppKit integration test. Run with `GPUI_MACOS_SMOKE=1 cargo test
//! -p gpui_macos --features font-kit --test appkit_smoke` in a desktop session.

#[cfg(target_os = "macos")]
fn main() {
    use gpui::{
        App, Application, Bounds, Context, Menu, MenuItem, MouseButton, NoAction, Window,
        WindowBounds, WindowOptions, div, prelude::*, px, size,
    };
    use gpui_macos::MacPlatform;
    use std::{cell::Cell, rc::Rc, time::Duration};

    if std::env::var_os("GPUI_MACOS_SMOKE").is_none() {
        println!("appkit_smoke skipped; set GPUI_MACOS_SMOKE=1 to open a test window");
        return;
    }

    struct View(Rc<Cell<usize>>, Rc<Cell<usize>>);
    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.0.set(self.0.get() + 1);
            let clicks = self.1.clone();
            div()
                .size_full()
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    clicks.set(clicks.get() + 1);
                })
                .child("AppKit smoke test")
        }
    }

    Application::with_platform(Rc::new(MacPlatform::new(false))).run(|cx: &mut App| {
        cx.set_menus([Menu::new("GPUI Test").items([
            MenuItem::action("Checked", NoAction).checked(true),
            MenuItem::separator(),
            MenuItem::submenu(
                Menu::new("Submenu").items([MenuItem::action("Disabled", NoAction).disabled(true)]),
            ),
        ])]);
        let frames = Rc::new(Cell::new(0));
        let rendered_frames = frames.clone();
        let clicks = Rc::new(Cell::new(0));
        let observed_clicks = clicks.clone();
        let bounds = Bounds::centered(None, size(px(320.), px(240.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("GPUI AppKit Test");
                    cx.new(|_| View(rendered_frames, observed_clicks))
                },
            )
            .unwrap();
        window
            .update(cx, |_, window, _| {
                assert!(window.scale_factor() > 0.);
                window.resize(size(px(400.), px(300.)));
                window.refresh();
            })
            .unwrap();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            assert!(
                frames.get() > 0,
                "the window must render at least one frame"
            );
            objc2::rc::autoreleasepool(|_| {
                use objc2::{MainThreadMarker, msg_send, rc::Retained};
                use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
                use objc2_foundation::NSPoint;
                let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
                let native_window = app.windows().firstObject().unwrap();
                for kind in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
                    let event: Retained<NSEvent> = unsafe {
                        msg_send![
                            objc2::class!(NSEvent),
                            mouseEventWithType: kind,
                            location: NSPoint::new(20., 20.),
                            modifierFlags: NSEventModifierFlags::empty(),
                            timestamp: 1.0f64,
                            windowNumber: native_window.windowNumber(),
                            context: std::ptr::null_mut::<objc2::runtime::AnyObject>(),
                            eventNumber: 1isize,
                            clickCount: 1isize,
                            pressure: 1.0f32,
                        ]
                    };
                    native_window.sendEvent(&event);
                }
            });
            assert_eq!(clicks.get(), 1, "native mouse input must reach GPUI");
            window
                .update(cx, |_, window, _| {
                    assert_eq!(window.viewport_size(), size(px(400.), px(300.)));
                    window.remove_window();
                })
                .unwrap();
            cx.update(|cx| {
                println!(
                    "appkit_smoke passed: menus, window creation, resize, draw, mouse input, close"
                );
                cx.quit();
            });
        })
        .detach();
    });
}

#[cfg(not(target_os = "macos"))]
fn main() {}
