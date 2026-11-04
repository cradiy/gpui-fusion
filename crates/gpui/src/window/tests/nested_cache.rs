use crate::{
    Context, Entity, IntoElement, Render, TestAppContext, Window, WindowHandle, div, prelude::*,
    px, rgb,
};
use std::{cell::Cell, rc::Rc};

struct Leaf {
    renders: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
}
impl Render for Leaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let clicks = self.clicks.clone();
        div()
            .on_mouse_down(crate::MouseButton::Left, move |_, _, _| {
                clicks.set(clicks.get() + 1)
            })
            .debug_selector(|| "cached-leaf".into())
            .size_full()
            .bg(rgb(0x285a73))
            .child("Aurora atlas")
    }
}
struct Branch {
    leaf: Entity<Leaf>,
    prefix: usize,
    visible: bool,
    offset: f32,
}
impl Render for Branch {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .overflow_hidden()
            .children((0..self.prefix).map(|index| {
                div()
                    .absolute()
                    .left(px(index as f32))
                    .size(px(1.))
                    .bg(rgb(0x568371))
                    .child("North")
                    .on_mouse_down(crate::MouseButton::Left, |_, _, _| {})
            }))
            .when(self.visible, |parent| {
                parent.child(
                    self.leaf.clone().cached(
                        div()
                            .absolute()
                            .left(px(self.offset))
                            .top(px(20.))
                            .size(px(60.))
                            .style()
                            .clone(),
                    ),
                )
            })
    }
}
struct Host {
    branch: Entity<Branch>,
    prefix: usize,
}
impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children((0..self.prefix).map(|index| {
                div()
                    .absolute()
                    .left(px(index as f32))
                    .size(px(1.))
                    .bg(rgb(0x714538))
                    .child("South")
                    .on_mouse_down(crate::MouseButton::Left, |_, _, _| {})
            }))
            .child(
                self.branch
                    .clone()
                    .cached(div().absolute().size(px(200.)).style().clone()),
            )
    }
}
fn draw(window: WindowHandle<Host>, cx: &mut TestAppContext) {
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
        .unwrap();
}

#[crate::test]
fn nested_cache_survives_parent_updates_and_ancestor_replays(cx: &mut TestAppContext) {
    check_nested_cache(cx);
}

fn check_nested_cache(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let leaf = cx.new(|_| Leaf {
        renders: renders.clone(),
        clicks: clicks.clone(),
    });
    let branch = cx.new(|_| Branch {
        leaf: leaf.clone(),
        prefix: 0,
        visible: true,
        offset: 10.,
    });
    let window = cx.add_window(|_, _| Host {
        branch: branch.clone(),
        prefix: 0,
    });
    draw(window, cx);
    let initial = renders.get();
    for prefix in [4, 1, 7, 0] {
        branch.update(cx, |branch, cx| {
            branch.prefix = prefix;
            cx.notify();
        });
        draw(window, cx);
        assert_eq!(
            renders.get(),
            initial,
            "a dirty parent must preserve a clean child"
        );
        for outside in [6, 0, 3] {
            window
                .update(cx, |host, _, cx| {
                    host.prefix = outside;
                    cx.notify();
                })
                .unwrap();
            draw(window, cx);
        }
        assert_eq!(renders.get(), initial);
        cx.update_window(window.into(), |_, window, cx| {
            let bounds = window
                .rendered_frame
                .debug_bounds
                .get("cached-leaf")
                .unwrap();
            assert_eq!(bounds.origin, crate::point(px(10.), px(20.)));
            let before = clicks.get();
            window.dispatch_event(
                crate::PlatformInput::MouseDown(crate::MouseDownEvent {
                    position: crate::point(px(25.), px(35.)),
                    ..Default::default()
                }),
                cx,
            );
            assert_eq!(clicks.get(), before + 1);
            window.mouse_position = crate::point(px(500.), px(500.));
        })
        .unwrap();
    }
    branch.update(cx, |branch, cx| {
        branch.visible = false;
        cx.notify();
    });
    draw(window, cx);
    branch.update(cx, |branch, cx| {
        branch.visible = true;
        cx.notify();
    });
    draw(window, cx);
    assert_eq!(renders.get(), initial + 1);
    branch.update(cx, |branch, cx| {
        branch.offset = 35.;
        cx.notify();
    });
    draw(window, cx);
    assert_eq!(renders.get(), initial + 2);
    leaf.update(cx, |_, cx| cx.notify());
    draw(window, cx);
    assert_eq!(renders.get(), initial + 3);
    window.update(cx, |_, window, _| window.refresh()).unwrap();
    draw(window, cx);
    assert_eq!(renders.get(), initial + 4);
}

struct TransformedHost {
    branch: Entity<Branch>,
    matrix: crate::TransformationMatrix,
    retry_prepaint: bool,
}
impl Render for TransformedHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let branch = self.branch.clone();
        let retry_prepaint = self.retry_prepaint;
        let mapping = crate::PointerTransform::affine(self.matrix).unwrap();
        let paint_mapping = mapping.clone();
        crate::canvas(
            move |bounds, window, cx| {
                window.with_pointer_transform(bounds, mapping, |window| {
                    let mut prepaint = |window: &mut Window| {
                        let mut child = branch
                            .clone()
                            .cached(div().size(px(200.)).style().clone())
                            .cache_across_transforms()
                            .into_any_element();
                        child.prepaint_as_root(
                            Default::default(),
                            crate::size(px(200.), px(200.)).into(),
                            window,
                            cx,
                        );
                        child
                    };
                    if retry_prepaint {
                        let _: Result<(), ()> = window.transact(|window| {
                            let _ = prepaint(window);
                            Err(())
                        });
                    }
                    prepaint(window)
                })
            },
            move |bounds, mut child, window, cx| {
                window.with_pointer_transform(bounds, paint_mapping, |window| {
                    child.paint(window, cx)
                });
            },
        )
        .size(px(400.))
    }
}

#[crate::test]
fn nested_cache_rebuilds_after_ancestor_input_remap(cx: &mut TestAppContext) {
    check_transformed_nested(cx, false);
}
#[crate::test]
fn nested_cache_survives_speculative_prepaint_rollback(cx: &mut TestAppContext) {
    check_transformed_nested(cx, true);
}
fn check_transformed_nested(cx: &mut TestAppContext, retry_prepaint: bool) {
    let renders = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let leaf = cx.new(|_| Leaf {
        renders: renders.clone(),
        clicks: clicks.clone(),
    });
    let branch = cx.new(|_| Branch {
        leaf,
        prefix: 0,
        visible: true,
        offset: 10.,
    });
    let window = cx.add_window(|_, _| TransformedHost {
        branch: branch.clone(),
        matrix: Default::default(),
        retry_prepaint,
    });
    let initial = renders.get();
    for scale in [1.5, 2., 1.] {
        window
            .update(cx, |host, _, cx| {
                host.matrix = crate::TransformationMatrix {
                    rotation_scale: [[scale, 0.], [0., scale]],
                    translation: [40., 0.],
                };
                cx.notify();
            })
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
            .unwrap();
        let before = renders.get();
        branch.update(cx, |branch, cx| {
            branch.prefix += 1;
            cx.notify();
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear();
            let clicked = clicks.get();
            window.dispatch_event(
                crate::PlatformInput::MouseDown(crate::MouseDownEvent {
                    position: crate::point(px(40. + 25. * scale), px(35. * scale)),
                    ..Default::default()
                }),
                cx,
            );
            assert_eq!(clicks.get(), clicked + 1);
            window.mouse_position = crate::point(px(500.), px(500.));
        })
        .unwrap();
        assert_eq!(renders.get(), before + if retry_prepaint { 2 } else { 1 });
    }
    assert!(renders.get() > initial);
}

struct ScrolledHost {
    branch: Entity<Branch>,
    scroll: crate::ScrollHandle,
}
impl Render for ScrolledHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("cache-viewport")
            .size(px(100.))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div().h(px(600.)).w(px(200.)).child(
                    self.branch
                        .clone()
                        .cached(div().size(px(200.)).style().clone()),
                ),
            )
    }
}
#[crate::test]
fn nested_cache_tracks_scrolling_and_clip_changes(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let leaf = cx.new(|_| Leaf {
        renders: renders.clone(),
        clicks: Rc::new(Cell::new(0)),
    });
    let branch = cx.new(|_| Branch {
        leaf,
        prefix: 0,
        visible: true,
        offset: 10.,
    });
    let scroll = crate::ScrollHandle::new();
    let window = cx.add_window(|_, _| ScrolledHost {
        branch,
        scroll: scroll.clone(),
    });
    for offset in [0., 10., 30., 0.] {
        scroll.set_offset(crate::point(px(0.), px(-offset)));
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear();
            let bounds = window
                .rendered_frame
                .debug_bounds
                .get("cached-leaf")
                .unwrap();
            assert_eq!(bounds.origin.y, px(20. - offset));
            window.draw(cx).clear();
        })
        .unwrap();
    }
}
