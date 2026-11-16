use super::*;
use crate::{default_theme, style::Styled as FusionStyled, theme::ThemeStore};
use gpui::{
    EntityInputHandler, IntoElement, KeyUpEvent, Keystroke, MouseButton, Render, Styled,
    TestAppContext, VisualTestContext, div, prelude::*, size,
};
use std::cell::RefCell;

struct Harness {
    select: Entity<Select>,
    theme: Entity<ThemeRuntime>,
    outside: FocusHandle,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let outside = self.outside.clone();
        div()
            .p_4()
            .flex()
            .flex_col()
            .items_start()
            .child(self.select.clone())
            .child(
                div()
                    .id("outside")
                    .debug_selector(|| "select-outside".into())
                    .absolute()
                    .left(px(310.))
                    .top(px(16.))
                    .w(px(90.))
                    .h(px(30.))
                    .track_focus(&self.outside)
                    .tab_stop(true)
                    .child("Continue")
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        outside.focus(window, cx)
                    }),
            )
    }
}
fn theme(cx: &mut App) -> Entity<ThemeRuntime> {
    ThemeRuntime::install(
        ThemeStore::new(default_theme(), "light").unwrap(),
        "neutral",
        cx,
    )
    .unwrap()
}
fn options() -> [SelectOption; 4] {
    [
        SelectOption::new("flac", "FLAC").keywords("lossless archive"),
        SelectOption::new("wav", "WAV").disabled(true),
        SelectOption::new("opus", "Opus").keywords("speech"),
        SelectOption::new("aac", "AAC"),
    ]
}
fn press(visual: &mut VisualTestContext, key: &str) {
    visual.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse(key).unwrap(),
    });
    visual.update(|window, cx| window.draw(cx).clear());
}
#[gpui::test]
fn selection_keyboard_clear_and_outside_dismissal(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let received = events.clone();
    let view = cx.open_window(size(px(440.), px(360.)), |window, cx| {
        let theme = theme(cx);
        let select = cx.new(|cx| {
            Select::new("Encoding", options(), window, cx)
                .unwrap()
                .clearable(true)
                .transition_duration(Duration::ZERO)
                .on_change(move |value, _, _| received.borrow_mut().push(value))
        });
        Harness {
            select,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let trigger = visual
        .debug_bounds("fusion-select-trigger")
        .unwrap()
        .center();
    visual.simulate_click(trigger, Default::default());
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-select-popup").is_some());
    let popup = visual.debug_bounds("fusion-select-popup").unwrap();
    let row = visual.debug_bounds("fusion-select-option-0").unwrap();
    assert!(row.size.width > popup.size.width * 0.9);
    assert!((row.left() - popup.left() - (popup.right() - row.right())).abs() < px(1.));
    let disabled = visual
        .debug_bounds("fusion-select-option-1")
        .unwrap()
        .center();
    visual.simulate_click(disabled, Default::default());
    assert!(events.borrow().is_empty());
    press(&mut visual, "down");
    press(&mut visual, "enter");
    assert_eq!(&*events.borrow(), &[Some(SharedString::from("opus"))]);
    view.update(&mut visual.cx, |h, window, cx| {
        assert!(!h.select.read(cx).open);
        assert!(h.select.focus_handle(cx).is_focused(window));
    })
    .unwrap();
    press(&mut visual, "down");
    let row = visual.debug_bounds("fusion-select-option-2").unwrap();
    visual.simulate_click(
        gpui::point(row.right() - px(2.), row.center().y),
        Default::default(),
    );
    view.update(&mut visual.cx, |h, _, cx| assert!(!h.select.read(cx).open))
        .unwrap();
    assert_eq!(events.borrow().len(), 1);
    visual.update(|window, cx| window.draw(cx).clear());
    let clear = visual.debug_bounds("fusion-select-clear").unwrap().center();
    visual.simulate_click(clear, Default::default());
    assert_eq!(events.borrow().last(), Some(&None));
    view.update(&mut visual.cx, |h, _, cx| assert!(!h.select.read(cx).open))
        .unwrap();
    press(&mut visual, "down");
    press(&mut visual, "end");
    press(&mut visual, "escape");
    assert_eq!(events.borrow().len(), 2);
    press(&mut visual, "down");
    let outside = visual.debug_bounds("select-outside").unwrap().center();
    visual.simulate_click(outside, Default::default());
    view.update(&mut visual.cx, |h, window, cx| {
        assert!(!h.select.read(cx).open);
        assert!(
            h.outside.is_focused(window),
            "focused {:?}, outside {:?}, trigger {:?}, panel {:?}, scope {:?}",
            window.focused(cx),
            h.outside,
            h.select.read(cx).focus,
            h.select.read(cx).panel_focus,
            h.select.read(cx).scope_focus
        );
        h.select
            .update(cx, |s, cx| s.set_disabled(true, window, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_click(trigger, Default::default());
    view.update(&mut visual.cx, |h, _, cx| assert!(!h.select.read(cx).open))
        .unwrap();
}
#[gpui::test]
fn search_ime_theme_reload_and_empty_results_preserve_selection(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(440.), px(400.)), |window, cx| {
        let theme = theme(cx);
        let select = cx.new(|cx| {
            Select::new("Encoding", options(), window, cx)
                .unwrap()
                .searchable(true)
                .transition_duration(Duration::ZERO)
        });
        Harness {
            select,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    view.update(&mut visual.cx, |h, window, cx| {
        window.activate_window();
        h.select.update(cx, |s, cx| {
            s.set_selected(Some("aac"), cx).unwrap();
            s.open(window, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_input("archive");
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, window, cx| {
        let search = h.select.read(cx).search.as_ref().unwrap().editor.clone();
        assert_eq!(h.select.read(cx).filtered, [0]);
        search.update(cx, |s, cx| {
            s.replace_and_mark_text_in_range(None, "kana", Some(0..4), window, cx)
        });
        h.theme.update(cx, |t, cx| {
            t.set_mode("dark", cx).unwrap();
            t.set_material_mode(MaterialMode::Glass, cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    press(&mut visual, "enter");
    view.update(&mut visual.cx, |h, window, cx| {
        let s = h.select.read(cx);
        assert!(s.open);
        assert_eq!(s.selected().map(|s| s.as_ref()), Some("aac"));
        assert!(s.composing(cx));
        let search = s.search.as_ref().unwrap().editor.clone();
        assert!(search.focus_handle(cx).is_focused(window));
        search.update(cx, |s, cx| s.set_value("unmatched query", cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-select-empty").is_some());
    press(&mut visual, "enter");
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.select.read(cx).open);
        let editor = h.select.read(cx).search.as_ref().unwrap().editor.clone();
        editor.update(cx, |s, cx| s.set_value("speech", cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    press(&mut visual, "enter");
    view.update(&mut visual.cx, |h, _, cx| {
        assert_eq!(
            h.select.read(cx).selected().map(|s| s.as_ref()),
            Some("opus")
        );
        assert!(!h.select.read(cx).open);
    })
    .unwrap();
}
#[gpui::test]
fn virtual_list_reveals_distant_items_fits_viewport_and_tabs_out(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(440.), px(260.)), |window, cx| {
        let theme = theme(cx);
        let select = cx.new(|cx| {
            Select::new(
                "Record",
                (0..5000).map(|i| SelectOption::new(format!("record-{i}"), format!("Record {i}"))),
                window,
                cx,
            )
            .unwrap()
            .transition_duration(Duration::ZERO)
            .part(SelectPart::Item, |s| s.h(px(28.)))
        });
        Harness {
            select,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    view.update(&mut visual.cx, |h, window, cx| {
        h.select.update(cx, |s, cx| s.open(window, cx))
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert!(visual.debug_bounds("fusion-select-option-0").is_some());
    assert!(visual.debug_bounds("fusion-select-option-2500").is_none());
    press(&mut visual, "end");
    let panel = visual.debug_bounds("fusion-select-popup").unwrap();
    let last = visual.debug_bounds("fusion-select-option-4999").unwrap();
    assert!(last.top() >= panel.top() && last.bottom() <= panel.bottom());
    assert!(panel.top() >= px(0.) && panel.bottom() <= px(260.));
    assert!(visual.debug_bounds("fusion-select-option-0").is_none());
    press(&mut visual, "tab");
    view.update(&mut visual.cx, |h, window, cx| {
        assert!(!h.select.read(cx).open);
        assert!(
            h.outside.is_focused(window),
            "focused {:?}, outside {:?}, trigger {:?}, panel {:?}, scope {:?}",
            window.focused(cx),
            h.outside,
            h.select.read(cx).focus,
            h.select.read(cx).panel_focus,
            h.select.read(cx).scope_focus
        );
    })
    .unwrap();
}
#[gpui::test]
fn option_replacement_is_atomic_and_reconciles_selected_values(cx: &mut TestAppContext) {
    cx.open_window(size(px(440.), px(260.)), |window, cx| {
        let theme = theme(cx);
        let select = cx.new(|cx| Select::new("Encoding", options(), window, cx).unwrap());
        select.update(cx, |s, cx| {
            s.set_selected(Some("opus"), cx).unwrap();
            assert!(s.set_selected(Some("wav"), cx).is_err());
            assert_eq!(s.selected().map(|s| s.as_ref()), Some("opus"));
            assert!(
                s.set_options(
                    [
                        SelectOption::new("dup", "First"),
                        SelectOption::new("dup", "Second")
                    ],
                    cx
                )
                .is_err()
            );
            assert_eq!(s.selected().map(|s| s.as_ref()), Some("opus"));
            s.set_options(
                [
                    SelectOption::new("opus", "Renamed"),
                    SelectOption::new("other", "Other"),
                ],
                cx,
            )
            .unwrap();
            assert_eq!(s.selected, Some(0));
            s.set_options([SelectOption::new("opus", "Renamed").disabled(true)], cx)
                .unwrap();
            assert!(s.selected().is_none());
            s.set_options([], cx).unwrap();
            s.open(window, cx);
            assert!(s.active.is_none());
        });
        Harness {
            select,
            theme,
            outside: cx.focus_handle().tab_stop(true),
        }
    });
}
