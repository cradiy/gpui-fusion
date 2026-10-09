use super::*;
use gpui::{Context, Render, TestAppContext, VisualTestContext, size};

struct Example {
    selected: usize,
    changes: Vec<usize>,
    width: f32,
    disabled: bool,
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut tabs = Tabs::new("test-tabs", self.selected)
            .w(px(self.width))
            .disabled(self.disabled)
            .on_change(move |value, _, cx| {
                entity.update(cx, |this, cx| {
                    this.selected = value;
                    this.changes.push(value);
                    cx.notify();
                })
            });
        for index in 0..6 {
            let label = div().w(px(80.)).child(format!("Tab {index}"));
            tabs = if index == 1 {
                tabs.disabled_tab(index, label)
            } else {
                tabs.tab(index, label)
            };
        }
        tabs
    }
}

#[gpui::test]
fn keyboard_skips_disabled_tabs_and_reveals_selection_without_fighting_scroll(
    cx: &mut TestAppContext,
) {
    let view = cx.open_window(size(px(800.), px(200.)), |_, _| Example {
        selected: 0,
        changes: vec![],
        width: 260.,
        disabled: false,
    });
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    cx.update(|window, cx| window.draw(cx).clear());
    let first = cx.debug_bounds("uic-tab-0").unwrap();
    cx.simulate_click(first.center(), Default::default());
    let disabled = cx.debug_bounds("uic-tab-1").unwrap();
    cx.simulate_click(disabled.center(), Default::default());
    view.update(&mut cx.cx, |this, _, _| assert!(this.changes.is_empty()))
        .unwrap();
    cx.simulate_keystrokes("right");
    view.update(&mut cx.cx, |this, _, _| assert_eq!(this.selected, 2))
        .unwrap();
    cx.simulate_keystrokes("end");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let last = cx.debug_bounds("uic-tab-5").unwrap();
    let bar = cx.debug_bounds("uic-tabs").unwrap();
    assert!(
        last.left() >= bar.left() && last.right() <= bar.right(),
        "{last:?} vs {bar:?}"
    );
    cx.simulate_keystrokes("right left home");
    view.update(&mut cx.cx, |this, _, _| {
        assert_eq!(this.changes, [2, 5, 0, 5, 0]);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
                position: gpui::point(px(100.), px(20.)),
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(-100.13), px(0.))),
                ..Default::default()
            }),
            cx,
        );
        window.draw(cx).clear();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let scrolled = cx.debug_bounds("uic-tab-0").unwrap();
    assert!((scrolled.left() - (first.left() - px(100.13))).abs() < px(0.5));
    view.update(&mut cx.cx, |this, _, cx| {
        this.selected = 5;
        this.width = 190.;
        cx.notify();
    })
    .unwrap();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let last = cx.debug_bounds("uic-tab-5").unwrap();
    let bar = cx.debug_bounds("uic-tabs").unwrap();
    assert!(last.left() >= bar.left() && last.right() <= bar.right());
    view.update(&mut cx.cx, |this, _, cx| {
        this.disabled = true;
        cx.notify();
    })
    .unwrap();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("home");
    view.update(&mut cx.cx, |this, _, _| assert_eq!(this.selected, 5))
        .unwrap();
}
