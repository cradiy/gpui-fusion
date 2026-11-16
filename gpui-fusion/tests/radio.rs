use gpui::{
    App, AppContext, Context, Entity, Focusable, IntoElement, KeyUpEvent, Keystroke, Render,
    SharedString, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size,
};
use gpui_fusion::{
    components::radio::{RadioGroup, RadioOption, RadioPart},
    default_theme,
    style::{MaterialMode, StylePatch, Styled as FusionStyled},
    theme::{ThemeStore, runtime::ThemeRuntime},
};
use std::{cell::RefCell, rc::Rc};

struct Harness {
    control: Entity<RadioGroup>,
    theme: Entity<ThemeRuntime>,
    after: gpui::FocusHandle,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.control.clone()).child(
            div()
                .id("after")
                .track_focus(&self.after.clone().tab_stop(true))
                .child("Continue"),
        )
    }
}
fn install(cx: &mut App) -> Entity<ThemeRuntime> {
    ThemeRuntime::install(
        ThemeStore::new(default_theme(), "light").unwrap(),
        "neutral",
        cx,
    )
    .unwrap()
}
fn options() -> [RadioOption; 4] {
    [
        RadioOption::new("dpi-150", "150 dpi"),
        RadioOption::new("dpi-300", "300 dpi").disabled(true),
        RadioOption::new("dpi-600", "600 dpi"),
        RadioOption::new("dpi-1200", "1200 dpi"),
    ]
}

#[gpui::test]
fn selection_excludes_labels_skips_disabled_and_wraps_with_one_tab_stop(cx: &mut TestAppContext) {
    let changes: Rc<RefCell<Vec<SharedString>>> = Default::default();
    let received = changes.clone();
    let handle = cx.open_window(size(px(420.), px(260.)), |window, cx| {
        let theme = install(cx);
        let control = cx.new(|cx| {
            RadioGroup::new("Raster resolution", options(), window, cx)
                .unwrap()
                .animated(false)
                .on_change(move |value, _, _| received.borrow_mut().push(value))
        });
        Harness {
            control,
            theme,
            after: cx.focus_handle(),
        }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let label = visual
        .debug_bounds("fusion-radio-label-2")
        .unwrap()
        .center();
    let disabled = visual
        .debug_bounds("fusion-radio-indicator-1")
        .unwrap()
        .center();
    let last = visual
        .debug_bounds("fusion-radio-indicator-3")
        .unwrap()
        .center();
    visual.simulate_click(label, Default::default());
    visual.simulate_click(disabled, Default::default());
    assert!(changes.borrow().is_empty());
    visual.simulate_click(last, Default::default());
    visual.simulate_click(last, Default::default());
    assert_eq!(&*changes.borrow(), &[SharedString::from("dpi-1200")]);
    visual.simulate_keystrokes("right right left end home");
    assert_eq!(
        changes
            .borrow()
            .iter()
            .map(|v| v.as_ref())
            .collect::<Vec<_>>(),
        [
            "dpi-1200", "dpi-150", "dpi-600", "dpi-150", "dpi-1200", "dpi-150"
        ]
    );
    visual.simulate_keystrokes("space");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    assert_eq!(changes.borrow().len(), 6);
    handle
        .update(&mut visual.cx, |h, window, cx| {
            assert_eq!(
                h.control.read(cx).selected().map(|s| s.as_ref()),
                Some("dpi-150")
            );
            assert!(h.control.read(cx).focus_handle(cx).is_focused(window));
            h.control.update(cx, |c, cx| {
                assert!(c.set_selected("dpi-300", cx).is_err());
                assert!(c.set_selected("absent", cx).is_err());
                c.set_disabled(true, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(last, Default::default());
    visual.simulate_keystrokes("right end");
    assert_eq!(changes.borrow().len(), 6);
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.control.update(cx, |c, cx| c.set_disabled(false, cx));
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(last, Default::default());
    visual.update(|window, cx| {
        window.draw(cx).clear();
        window.focus_next(cx);
    });
    handle
        .update(&mut visual.cx, |h, window, _| {
            // Tab leaves the selected radio rather than entering another option.
            assert!(h.after.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn theme_and_custom_parts_preserve_selection_and_focus(cx: &mut TestAppContext) {
    let handle = cx.open_window(size(px(580.), px(150.)), |window, cx| {
        let theme = install(cx);
        let control = cx.new(|cx| {
            RadioGroup::new("Raster resolution", options(), window, cx)
                .unwrap()
                .animated(false)
                .horizontal(true)
                .part(RadioPart::Indicator, |s| s.w(px(24.)).h(px(24.)))
                .part(RadioPart::Dot, |s| {
                    s.w(px(10.)).h(px(10.)).bg(gpui::rgb(0x102050))
                })
        });
        Harness {
            control,
            theme,
            after: cx.focus_handle(),
        }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let before = visual.debug_bounds("fusion-radio-indicator-2").unwrap();
    assert_eq!(before.size.width, px(24.));
    assert_eq!(
        before.origin.y,
        visual
            .debug_bounds("fusion-radio-indicator-0")
            .unwrap()
            .origin
            .y
    );
    visual.simulate_click(before.center(), Default::default());
    handle
        .update(&mut visual.cx, |h, _, cx| {
            h.theme.update(cx, |t, cx| {
                t.set_mode("dark", cx).unwrap();
                t.set_material_mode(MaterialMode::Glass, cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-radio-indicator-2").unwrap(),
        before
    );
    handle
        .update(&mut visual.cx, |h, window, cx| {
            let control = h.control.read(cx);
            assert_eq!(control.selected().map(|s| s.as_ref()), Some("dpi-600"));
            assert!(control.focus_handle(cx).is_focused(window));
            assert!(control.style_error().is_none());
            h.control.update(cx, |c, cx| {
                c.set_style(StylePatch::default(), cx);
                c.set_variant(Some("absent".into()), cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    handle
        .update(&mut visual.cx, |h, _, cx| {
            assert!(h.control.read(cx).style_error().is_some());
            h.control.update(cx, |c, cx| {
                c.set_variant(None, cx);
                c.set_material_mode(Some(MaterialMode::Plain), cx);
            });
        })
        .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual
            .debug_bounds("fusion-radio-indicator-2")
            .unwrap()
            .size
            .width,
        px(20.)
    );
    handle
        .update(&mut visual.cx, |h, _, cx| {
            assert!(h.control.read(cx).style_error().is_none())
        })
        .unwrap();
}

#[gpui::test]
fn invalid_options_are_rejected_and_unavailable_groups_have_no_selection(cx: &mut TestAppContext) {
    cx.open_window(size(px(300.), px(150.)), |window, cx| {
        let theme = install(cx);
        for options in [
            vec![RadioOption::new("", "Resolution")],
            vec![RadioOption::new("draft", " ")],
            vec![
                RadioOption::new("draft", "Draft"),
                RadioOption::new("draft", "Duplicate"),
            ],
        ] {
            cx.new(|cx| {
                assert!(RadioGroup::new("Resolution", options, window, cx).is_err());
                RadioGroup::new("Resolution", [], window, cx).unwrap()
            });
        }
        let empty = cx.new(|cx| RadioGroup::new("Resolution", [], window, cx).unwrap());
        assert!(empty.read(cx).selected().is_none());
        let control = cx.new(|cx| {
            RadioGroup::new(
                "Resolution",
                [RadioOption::new("draft", "Draft").disabled(true)],
                window,
                cx,
            )
            .unwrap()
        });
        assert!(control.read(cx).selected().is_none());
        Harness {
            control,
            theme,
            after: cx.focus_handle(),
        }
    });
}
