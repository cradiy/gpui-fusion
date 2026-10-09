use super::*;
use crate::components::input;
use gpui::{Context, EntityInputHandler, Render, TestAppContext, VisualTestContext, size};

struct Example {
    select: Entity<SelectState>,
    changes: Vec<Option<SharedString>>,
    _subscription: gpui::Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            Select::new("select", &self.select)
                .w(px(240.))
                .clearable(true),
        )
    }
}
fn setup(cx: &mut TestAppContext) -> gpui::WindowHandle<Example> {
    cx.update(|cx| {
        input::init(cx);
    });
    cx.open_window(size(px(500.), px(640.)), |window, cx| {
        let select = cx.new(|cx| {
            SelectState::new(
                vec![
                    SelectOption::new("a", "Apple"),
                    SelectOption::new("b", "Banana").disabled(true),
                    SelectOption::new("c", "Cherry"),
                    SelectOption::new("d", "Date"),
                ],
                window,
                cx,
            )
        });
        let subscription = cx.subscribe(
            &select,
            |this: &mut Example, _, event: &SelectChanged, _| {
                this.changes.push(event.selected.clone())
            },
        );
        Example {
            select,
            changes: vec![],
            _subscription: subscription,
        }
    })
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}
fn open_popup(cx: &mut VisualTestContext) {
    cx.simulate_click(point(px(80.), px(36.)), Default::default());
    draw(cx);
    draw(cx);
}

#[gpui::test]
fn search_navigation_ime_and_option_updates_preserve_selection_contract(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    draw(&mut cx);
    open_popup(&mut cx);
    view.update(&mut cx.cx, |this, window, cx| {
        let state = this.select.read(cx);
        assert!(state.is_open());
        assert!(state.search.focus_handle(cx).is_focused(window));
    })
    .unwrap();
    cx.simulate_keystrokes("down enter");
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(
            this.select.read(cx).selected_id().map(|s| s.as_ref()),
            Some("c")
        );
        assert_eq!(this.changes.len(), 1);
    })
    .unwrap();
    open_popup(&mut cx);
    let search = view
        .update(&mut cx.cx, |this, _, cx| {
            this.select.read(cx).search.clone()
        })
        .unwrap();
    search.update(&mut cx.cx, |input, cx| input.set_value("date", cx));
    draw(&mut cx);
    cx.simulate_keystrokes("enter");
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert_eq!(
            this.select.read(cx).selected_id().map(|s| s.as_ref()),
            Some("d")
        )
    })
    .unwrap();
    open_popup(&mut cx);
    cx.update(|window, cx| {
        search.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx)
        })
    });
    cx.simulate_keystrokes("enter");
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert!(this.select.read(cx).is_open());
        assert_eq!(this.changes.len(), 2);
    })
    .unwrap();
    cx.update(|window, cx| {
        search.update(cx, |input, cx| {
            input.replace_text_in_range(None, "", window, cx)
        })
    });
    cx.simulate_keystrokes("escape");
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        this.select.update(cx, |state, cx| {
            state.set_options(vec![SelectOption::new("a", "Apple")], cx)
        });
        assert_eq!(this.select.read(cx).selected_id(), None);
    })
    .unwrap();
    view.update(&mut cx.cx, |this, _, _| {
        assert_eq!(this.changes, vec![Some("c".into()), Some("d".into()), None])
    })
    .unwrap();
}

#[gpui::test]
fn outside_click_and_escape_allow_reopening(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut cx = VisualTestContext::from_window(view.into(), cx);
    draw(&mut cx);
    open_popup(&mut cx);
    cx.simulate_click(point(px(460.), px(560.)), Default::default());
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert!(!this.select.read(cx).is_open());
    })
    .unwrap();
    draw(&mut cx);
    open_popup(&mut cx);
    cx.simulate_keystrokes("escape");
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert!(!this.select.read(cx).is_open());
        assert!(this.changes.is_empty());
    })
    .unwrap();
    open_popup(&mut cx);
    cx.simulate_keystrokes("down enter");
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert!(!this.select.read(cx).is_open());
        assert_eq!(
            this.select.read(cx).selected_id().map(|s| s.as_ref()),
            Some("c")
        );
    })
    .unwrap();
    open_popup(&mut cx);
    cx.simulate_click(point(px(80.), px(140.)), Default::default());
    draw(&mut cx);
    view.update(&mut cx.cx, |this, _, cx| {
        assert!(!this.select.read(cx).is_open());
        assert_eq!(
            this.select.read(cx).selected_id().map(|id| id.as_ref()),
            Some("a")
        );
    })
    .unwrap();
}
