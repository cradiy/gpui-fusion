use super::*;
use crate::components::input;
use gpui::{Context, EntityInputHandler, Render, TestAppContext, VisualTestContext, size};

struct Example {
    state: Entity<SelectState>,
    changes: Vec<Vec<SharedString>>,
    _subscription: gpui::Subscription,
}
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(
            MultiSelect::new("choices", &self.state)
                .w(px(240.))
                .clearable(true),
        )
    }
}
fn setup(cx: &mut TestAppContext) -> gpui::WindowHandle<Example> {
    cx.update(input::init);
    cx.open_window(size(px(500.), px(640.)), |window, cx| {
        let state = cx.new(|cx| {
            SelectState::multiple(
                vec![
                    SelectOption::new("a", "Apple"),
                    SelectOption::new("b", "Banana").disabled(true),
                    SelectOption::new("c", "Cherry"),
                    SelectOption::new("d", "Date"),
                ],
                window,
                cx,
            )
            .max_selected(2)
        });
        let subscription = cx.subscribe(
            &state,
            |this: &mut Example, _, event: &MultiSelectChanged, cx| {
                this.changes.push(event.selected.clone());
                cx.notify();
            },
        );
        Example {
            state,
            changes: Vec::new(),
            _subscription: subscription,
        }
    })
}
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear());
}

#[gpui::test]
fn selection_limits_and_option_replacement_preserve_ids_atomically(cx: &mut TestAppContext) {
    let view = setup(cx);
    view.update(cx, |this, _, cx| {
        this.state.update(cx, |state, cx| {
            assert!(state.select("a", cx));
            assert!(state.select("a", cx));
            assert!(!state.select("b", cx));
            assert!(state.select("c", cx));
            assert!(!state.select("d", cx));
            for ids in [
                vec!["a", "a"],
                vec!["b"],
                vec!["missing"],
                vec!["a", "c", "d"],
            ] {
                assert!(
                    !state.set_selected_ids(ids.into_iter().map(SharedString::from).collect(), cx)
                );
                assert_eq!(state.selected_ids(), &[SharedString::from("a"), "c".into()]);
            }
            state.set_options(
                vec![
                    SelectOption::new("a", "Renamed").disabled(true),
                    SelectOption::new("d", "Date"),
                ],
                cx,
            );
            assert_eq!(state.selected_ids(), &[SharedString::from("a")]);
            assert_eq!(state.selected_option().unwrap().label.as_ref(), "Renamed");
            assert!(state.deselect("a", cx));
            assert!(!state.deselect("a", cx));
        })
    })
    .unwrap();
    view.update(cx, |this, _, _| {
        assert_eq!(
            this.changes,
            vec![
                vec![SharedString::from("a")],
                vec!["a".into(), "c".into()],
                vec!["a".into()],
                vec![]
            ]
        );
    })
    .unwrap();
}

#[gpui::test]
fn toggles_stay_open_and_search_composition_does_not_select(cx: &mut TestAppContext) {
    let view = setup(cx);
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    draw(&mut visual);
    visual.simulate_click(point(px(80.), px(36.)), Default::default());
    draw(&mut visual);
    draw(&mut visual);
    visual.simulate_keystrokes("enter down enter");
    draw(&mut visual);
    let search = view
        .update(&mut visual.cx, |this, _, cx| {
            let data = this.state.read(cx);
            assert!(data.is_open());
            assert_eq!(data.selected_ids(), &[SharedString::from("a"), "c".into()]);
            assert!(data.unavailable(&data.options[3]));
            data.search.clone()
        })
        .unwrap();
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    search.update(&mut visual.cx, |input, cx| input.set_value("date", cx));
    draw(&mut visual);
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(this.state.read(cx).is_open());
        assert_eq!(
            this.state.read(cx).selected_ids(),
            &[SharedString::from("a"), "d".into()]
        );
    })
    .unwrap();
    visual.update(|window, cx| {
        search.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx)
        })
    });
    visual.simulate_keystrokes("enter");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, _| {
        assert_eq!(this.changes.len(), 4)
    })
    .unwrap();
    visual.update(|window, cx| {
        search.update(cx, |input, cx| {
            input.replace_text_in_range(None, "", window, cx)
        })
    });
    visual.simulate_keystrokes("escape backspace");
    draw(&mut visual);
    view.update(&mut visual.cx, |this, _, cx| {
        assert!(!this.state.read(cx).is_open());
        assert_eq!(
            this.state.read(cx).selected_ids(),
            &[SharedString::from("a")]
        );
    })
    .unwrap();
}
