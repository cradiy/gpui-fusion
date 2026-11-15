use gpui::{
    AppContext, Context, Entity, Focusable, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px, size,
};
use gpui_fusion::{
    default_theme,
    runtime::ThemeRuntime,
    slider::{Slider, SliderRange},
    style::{StylePatch, Styled as FusionStyled},
    theme::ThemeStore,
};
use std::{cell::RefCell, rc::Rc};

struct Harness {
    slider: Entity<Slider>,
    theme: Entity<ThemeRuntime>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(self.slider.clone())
    }
}

#[test]
fn range_rejects_unrepresentable_steps_and_limits() {
    for (min, max, step) in [
        (4., 4., None),
        (2., -2., None),
        (f64::NAN, 1., None),
        (0., f64::INFINITY, None),
        (-f64::MAX, f64::MAX, None),
        (0., 1., Some(0.)),
        (0., 1., Some(-1.)),
        (0., 1., Some(f64::NAN)),
        (0., 1., Some(f64::INFINITY)),
        (1e16, 1e17, Some(0.1)),
    ] {
        assert!(SliderRange::new(min, max, step).is_err());
    }
}

#[gpui::test]
fn slider_drag_keyboard_range_and_cancellation(cx: &mut TestAppContext) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let events = changes.clone();
    let view = cx.open_window(size(px(420.), px(200.)), |window, cx| {
        let theme =
            cx.new(|_| ThemeRuntime::new(ThemeStore::new(default_theme(), "light").unwrap()));
        let slider = cx.new(|cx| {
            Slider::new("Level", -7., theme.clone(), "neutral", window, cx)
                .range(SliderRange::new(-7., 14., Some(4.)).unwrap())
                .animated(false)
                .on_change(move |value, _, _| events.borrow_mut().push(value))
        });
        Harness { slider, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    });
    let bounds = visual.debug_bounds("fusion-slider").unwrap();
    visual.simulate_click(bounds.center(), Default::default());
    assert_eq!(&*changes.borrow(), &[5.]);
    visual.simulate_keystrokes("right");
    visual.simulate_keystrokes("end");
    assert_eq!(&*changes.borrow(), &[5., 9., 14.]);
    visual.simulate_keystrokes("left");
    assert_eq!(changes.borrow().last(), Some(&9.));
    visual.simulate_keystrokes("home");
    visual.simulate_keystrokes("pageup");
    assert_eq!(changes.borrow().last(), Some(&-3.));
    visual.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("pageup").unwrap(),
    });
    view.update(&mut visual.cx, |h, window, cx| {
        h.slider.update(cx, |s, cx| s.set_value(5., cx));
        h.theme.update(cx, |t, cx| t.set_mode("dark", cx).unwrap());
        assert!(h.slider.read(cx).focus_handle(cx).is_focused(window));
    })
    .unwrap();
    let count = changes.borrow().len();
    visual.update(|window, cx| window.draw(cx).clear());
    // Pressing the thumb at an offset must not jump the value.
    let thumb_center = gpui::point(
        bounds.left() + px(11.) + (bounds.size.width - px(22.)) * (12. / 21.),
        bounds.center().y,
    );
    visual.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: thumb_center + gpui::point(px(6.), px(0.)),
        ..Default::default()
    });
    assert_eq!(changes.borrow().len(), count);
    // Capture must work even before the next draw and beyond the control's bounds.
    let outside = gpui::point(bounds.right() + px(60.), bounds.center().y);
    visual.simulate_event(gpui::MouseMoveEvent {
        position: outside,
        pressed_button: Some(gpui::MouseButton::Left),
        ..Default::default()
    });
    assert_eq!(changes.borrow().last(), Some(&14.));
    let count = changes.borrow().len();
    visual.simulate_event(gpui::MouseMoveEvent {
        position: outside,
        pressed_button: Some(gpui::MouseButton::Left),
        ..Default::default()
    });
    assert_eq!(changes.borrow().len(), count);
    view.update(&mut visual.cx, |h, _, cx| {
        h.slider.update(cx, |s, cx| s.set_disabled(true, cx))
    })
    .unwrap();
    visual.simulate_event(gpui::MouseMoveEvent {
        position: bounds.origin,
        pressed_button: Some(gpui::MouseButton::Left),
        ..Default::default()
    });
    visual.simulate_event(gpui::MouseUpEvent {
        position: bounds.origin,
        button: gpui::MouseButton::Left,
        ..Default::default()
    });
    visual.update(|window, cx| window.draw(cx).clear());
    visual.simulate_keystrokes("home");
    visual.simulate_click(bounds.center(), Default::default());
    assert_eq!(changes.borrow().len(), count);
    view.update(&mut visual.cx, |h, _, cx| {
        h.slider.update(cx, |s, cx| {
            s.set_value(f64::NAN, cx);
            assert_eq!(s.value(), 14.);
            s.set_range(SliderRange::new(-2., 2., None).unwrap(), cx);
            assert_eq!(s.value(), 2.);
            s.set_value(0.125, cx);
            assert_eq!(s.value(), 0.125);
        })
    })
    .unwrap();
    assert_eq!(changes.borrow().len(), count);
}

#[gpui::test]
fn slider_theme_and_instance_styles_refresh_without_resetting_value(cx: &mut TestAppContext) {
    let view = cx.open_window(size(px(420.), px(200.)), |window, cx| {
        let theme =
            cx.new(|_| ThemeRuntime::new(ThemeStore::new(default_theme(), "light").unwrap()));
        let slider = cx.new(|cx| {
            Slider::new("Level", 37., theme.clone(), "neutral", window, cx)
                .variant("glass")
                .animated(false)
        });
        Harness { slider, theme }
    });
    let mut visual = VisualTestContext::from_window(view.into(), cx);
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        h.slider.update(cx, |s, cx| {
            s.set_style(StylePatch::default().w(px(310.)), cx)
        });
        h.theme.update(cx, |t, cx| t.set_mode("dark", cx).unwrap());
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        visual.debug_bounds("fusion-slider").unwrap().size.width,
        px(310.)
    );
    view.update(&mut visual.cx, |h, _, cx| {
        assert_eq!(h.slider.read(cx).value(), 37.);
        h.slider.update(cx, |s, cx| {
            s.set_style(StylePatch::default(), cx);
            s.set_variant(Some("missing".into()), cx);
        });
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.slider.read(cx).style_error().is_some());
        h.slider.update(cx, |s, cx| s.set_variant(None, cx));
    })
    .unwrap();
    visual.update(|window, cx| window.draw(cx).clear());
    view.update(&mut visual.cx, |h, _, cx| {
        assert!(h.slider.read(cx).style_error().is_none());
        assert_eq!(h.slider.read(cx).value(), 37.);
    })
    .unwrap();
}
