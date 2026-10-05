use super::*;
use gpui::{
    AppContext, Context, MouseDownEvent, MouseUpEvent, PlatformInput, Render, TestAppContext,
    WindowHandle, canvas, point, prelude::FluentBuilder, px, rgb, rgba, size,
};
use std::{cell::Cell, rc::Rc};

struct Preview {
    active: bool,
    explicit: bool,
    enabled: bool,
    clicks: usize,
    color: Rc<Cell<Hsla>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = self.color.clone();
        let bounds = self.bounds.clone();
        div().text_color(rgb(0x00ff00)).child(
            animated_style("button")
                .w(px(120.))
                .h(px(50.))
                .rounded(px(if self.active { 20. } else { 8. }))
                .bg(rgb(if self.active { 0xffffff } else { 0x000000 }))
                .when(self.explicit, |item| {
                    item.text_color(rgb(if self.active { 0xffffff } else { 0x000000 }))
                })
                .duration(Duration::from_secs(1))
                .enabled(self.enabled)
                .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
                .child(
                    canvas(
                        move |region, window, _| {
                            bounds.set(region);
                            color.set(window.text_style().color);
                        },
                        |_, _, _, _| {},
                    )
                    .size_full(),
                ),
        )
    }
}

fn draw(handle: WindowHandle<Preview>, cx: &mut TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
    })
    .unwrap();
}

#[gpui::test]
fn animated_style_retargets_preserves_input_and_restores_inheritance(cx: &mut TestAppContext) {
    // Transparent RGB must not tint an alpha fade.
    let mixed = Rgba::from(mix_color(
        rgba(0xff000000).into(),
        rgb(0x0000ff).into(),
        0.5,
    ));
    assert_eq!(
        mixed,
        Rgba {
            r: 0.,
            g: 0.,
            b: 1.,
            a: 0.5
        }
    );
    let color = Rc::new(Cell::new(Hsla::default()));
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let handle = cx.open_window(size(px(300.), px(150.)), |_, _| Preview {
        active: false,
        explicit: true,
        enabled: true,
        clicks: 0,
        color: color.clone(),
        bounds: bounds.clone(),
    });
    draw(handle, cx);
    assert_eq!(Rgba::from(color.get()), rgb(0x000000));
    let original_bounds = bounds.get();
    handle
        .update(cx, |view, _, cx| {
            view.active = true;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(Rgba::from(color.get()), rgb(0x000000));
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(handle, cx);
    let halfway = color.get();
    assert!((Rgba::from(halfway).r - 0.5).abs() < 0.001);
    assert_eq!(
        bounds.get(),
        original_bounds,
        "paint transitions must not alter layout"
    );
    cx.update_window(handle.into(), |_, window, cx| {
        for event in [
            PlatformInput::MouseDown(MouseDownEvent {
                position: point(px(20.), px(20.)),
                ..Default::default()
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                position: point(px(20.), px(20.)),
                ..Default::default()
            }),
        ] {
            window.dispatch_event(event, cx);
        }
    })
    .unwrap();
    handle
        .update(cx, |view, _, cx| {
            assert_eq!(view.clicks, 1);
            view.active = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        color.get(),
        halfway,
        "interruption must preserve the displayed color"
    );
    handle
        .update(cx, |view, _, cx| {
            view.explicit = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(
        Rgba::from(color.get()),
        rgb(0x00ff00),
        "removed explicit color must restore inheritance immediately"
    );
    handle
        .update(cx, |view, _, cx| {
            view.explicit = true;
            view.active = true;
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(handle, cx);
    assert_eq!(Rgba::from(color.get()), rgb(0xffffff));
}
