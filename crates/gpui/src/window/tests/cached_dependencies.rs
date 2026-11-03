use crate::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, Window, canvas, deferred, div, fill, px, rgb,
};

#[crate::test]
fn cached_reads_do_not_restore_released_entity_subscriptions(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};

    struct Reader {
        optional: crate::WeakEntity<f32>,
        live: Entity<f32>,
        renders: Rc<Cell<usize>>,
    }
    impl Render for Reader {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let opacity = self.optional.upgrade().map_or(1., |value| *value.read(cx));
            div()
                .size_full()
                .bg(rgb(0x457890))
                .opacity(opacity * *self.live.read(cx))
        }
    }
    struct Root(Entity<Reader>);
    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.0
                .clone()
                .cached(crate::StyleRefinement::default().size(px(64.)))
        }
    }

    let value = cx.new(|_| 0.5f32);
    let released_id = value.entity_id();
    let weak = value.downgrade();
    let live = cx.new(|_| 1.0f32);
    let renders = Rc::new(Cell::new(0));
    let child = cx.new(|_| Reader {
        optional: weak.clone(),
        live: live.clone(),
        renders: renders.clone(),
    });
    let handle = cx.add_window(|_, _| Root(child));
    cx.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear();
        assert!(cx.window_invalidators_by_entity.contains_key(&released_id));
    })
    .unwrap();
    drop(value);
    cx.update(|_| ());
    assert!(weak.upgrade().is_none());
    cx.update(|cx| assert!(!cx.window_invalidators_by_entity.contains_key(&released_id)));

    let previous_renders = renders.get();
    for _ in 0..3 {
        cx.update_window(handle.into(), |_, window, cx| {
            window.invalidator.set_dirty(true);
            window.draw(cx).clear();
            assert_eq!(
                renders.get(),
                previous_renders,
                "the child must reuse its cache"
            );
            assert!(!cx.window_invalidators_by_entity.contains_key(&released_id));
            assert!(!cx.current_window_by_entity.contains_key(&released_id));
            assert!(!cx.tracked_entities[&handle.window_id()].contains(&released_id));
        })
        .unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!window.invalidator.is_dirty());
        live.update(cx, |value, cx| {
            *value = 0.25;
            cx.notify();
        });
        assert!(window.invalidator.is_dirty());
    })
    .unwrap();
}

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
