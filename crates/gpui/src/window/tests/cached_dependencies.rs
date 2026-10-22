use crate::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, Window, canvas, deferred, div, fill, px, rgb,
};

struct PaintReader {
    value: Entity<f32>,
    deferred: bool,
}

impl Render for PaintReader {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let value = self.value.clone();
        let content = canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                window.paint_quad(fill(bounds, rgb(0x457890).opacity(*value.read(cx))));
            },
        )
        .size_full();
        if self.deferred {
            deferred(content).into_any_element()
        } else {
            content.into_any_element()
        }
    }
}

struct Host {
    child: Entity<PaintReader>,
    value: Entity<f32>,
    read_before_child: bool,
    opacity: f32,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.read_before_child {
            let _ = self.value.read(cx);
        }
        div().opacity(self.opacity).child(
            self.child
                .clone()
                .cached(crate::StyleRefinement::default().size(px(64.))),
        )
    }
}

#[crate::test]
fn cached_paint_and_deferred_reads_remain_subscribed(cx: &mut TestAppContext) {
    for deferred in [false, true] {
        let value = cx.new(|_| 1.0f32);
        let child = cx.new(|_| PaintReader {
            value: value.clone(),
            deferred,
        });
        let window = cx.add_window(|_, _| Host {
            child,
            value: value.clone(),
            read_before_child: true,
            opacity: 1.,
        });
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
            .unwrap();
        // The parent stops reading the dependency while its child reuses cached paint.
        window
            .update(cx, |host, _, cx| {
                host.read_before_child = false;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            value.update(cx, |value, cx| {
                *value = 0.25;
                cx.notify();
            });
            assert!(window.invalidator.is_dirty(), "deferred={deferred}");
        })
        .unwrap();
    }
}

#[crate::test]
fn cached_paint_tracks_inherited_opacity(cx: &mut TestAppContext) {
    let value = cx.new(|_| 1.0f32);
    let child = cx.new(|_| PaintReader {
        value: value.clone(),
        deferred: false,
    });
    let window = cx.add_window(|_, _| Host {
        child,
        value,
        read_before_child: false,
        opacity: 1.,
    });
    for opacity in [1., 0.375, 0.375, 1.] {
        window
            .update(cx, |host, _, cx| {
                host.opacity = opacity;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear();
            assert_eq!(
                window.rendered_frame.scene.quads[0].background.solid.a,
                opacity
            );
        })
        .unwrap();
    }
}

#[crate::test]
fn cached_paint_settles_after_hover_changes_without_pointer_motion(cx: &mut TestAppContext) {
    struct HoverHost {
        child: Entity<PaintReader>,
        width: f32,
    }
    impl Render for HoverHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("opacity-host")
                .w(px(self.width))
                .h(px(160.))
                .opacity(0.25)
                .hover(|style| style.opacity(0.75))
                .child(
                    self.child
                        .clone()
                        .cached(crate::StyleRefinement::default().size(px(64.))),
                )
        }
    }
    let value = cx.new(|_| 1.0f32);
    let child = cx.new(|_| PaintReader {
        value,
        deferred: false,
    });
    let handle = cx.add_window(|_, _| HoverHost { child, width: 80. });
    for (width, alpha) in [(80., 0.25), (240., 0.75), (80., 0.25)] {
        handle
            .update(cx, |host, window, cx| {
                host.width = width;
                window.mouse_position = crate::point(px(120.), px(32.));
                cx.notify();
            })
            .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.draw(cx).clear();
            for _ in 0..3 {
                let callbacks = window.next_frame_callbacks.take();
                if callbacks.is_empty() {
                    break;
                }
                for callback in callbacks {
                    callback(window, cx);
                }
                window.draw(cx).clear();
            }
            assert_eq!(
                window.rendered_frame.scene.quads[0].background.solid.a,
                alpha
            );
            assert!(window.next_frame_callbacks.borrow().is_empty());
        })
        .unwrap();
    }
}
