use super::Window;
use crate::{App, Scene};
use std::{mem::size_of, time::Duration};

const IDLE_DELAY: Duration = Duration::from_secs(2);
const MIN_RECLAIM_BYTES: usize = 4 * 1024 * 1024;
const RETAIN_BYTES: usize = 64 * 1024;

fn target_capacity<T>(values: &Vec<T>, live_len: usize) -> Option<usize> {
    let stride = size_of::<T>().max(1);
    let target = live_len
        .max(values.len())
        .checked_next_power_of_two()
        .unwrap_or(usize::MAX)
        .max(RETAIN_BYTES / stride);
    (values
        .capacity()
        .saturating_sub(target)
        .saturating_mul(stride)
        >= MIN_RECLAIM_BYTES)
        .then_some(target)
}

fn needs_reclaim(scene: &Scene, live: &Scene) -> bool {
    target_capacity(&scene.quads, live.quads.len()).is_some()
        || target_capacity(&scene.paint_operations, live.paint_operations.len()).is_some()
}

fn reclaim(scene: &mut Scene, live_quads: usize, live_operations: usize) {
    if let Some(target) = target_capacity(&scene.quads, live_quads) {
        scene.quads.shrink_to(target);
    }
    if let Some(target) = target_capacity(&scene.paint_operations, live_operations) {
        scene.paint_operations.shrink_to(target);
    }
}

impl Window {
    pub(super) fn schedule_scene_memory_reclaim(&mut self, cx: &App) {
        self.last_scene_draw = cx.background_executor().now();
        let live = &self.rendered_frame.scene;
        if !needs_reclaim(live, live) && !needs_reclaim(&self.next_frame.scene, live) {
            self.scene_reclaim_task = None;
            return;
        }
        if self.scene_reclaim_task.is_some() {
            return;
        }
        self.scene_reclaim_task = Some(self.spawn(cx, async move |cx| {
            let mut delay = IDLE_DELAY;
            loop {
                cx.background_executor().timer(delay).await;
                let remaining = cx
                    .update(|window, cx| {
                        let elapsed = cx.background_executor().now() - window.last_scene_draw;
                        if elapsed < IDLE_DELAY {
                            return Some(IDLE_DELAY - elapsed);
                        }
                        let quads = window.rendered_frame.scene.quads.len();
                        let operations = window.rendered_frame.scene.paint_operations.len();
                        reclaim(&mut window.rendered_frame.scene, quads, operations);
                        reclaim(&mut window.next_frame.scene, quads, operations);
                        window.scene_reclaim_task = None;
                        None
                    })
                    .ok()
                    .flatten();
                let Some(remaining) = remaining else {
                    break;
                };
                delay = remaining;
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _,
        TestAppContext, canvas, div, fill, point, px, rgb, size,
    };

    struct Content(usize);
    impl Render for Content {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let count = self.0;
            div().size_full().child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        for index in 0..count {
                            window.paint_quad(fill(
                                Bounds::new(
                                    bounds.origin
                                        + point(px((index % 150) as f32), px((index / 150) as f32)),
                                    size(px(1.), px(1.)),
                                ),
                                rgb(0x638fa1),
                            ));
                        }
                    },
                )
                .size_full(),
            )
        }
    }

    fn draw(window: crate::WindowHandle<Content>, count: usize, cx: &mut TestAppContext) {
        window
            .update(cx, |content, _, cx| {
                content.0 = count;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
            .unwrap();
    }

    #[crate::test]
    fn idle_reclaim_waits_for_quiet_and_preserves_scene_replay(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Content(18000));
        draw(window, 18000, cx);
        draw(window, 18000, cx);
        draw(window, 8, cx);
        draw(window, 8, cx);
        let before = window
            .update(cx, |_, window, _| {
                assert!(window.scene_reclaim_task.is_some());
                window.rendered_frame.scene.paint_operations.capacity()
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        draw(window, 8, cx);
        cx.executor().advance_clock(Duration::from_millis(1500));
        cx.run_until_parked();
        window
            .update(cx, |_, window, _| {
                assert!(window.scene_reclaim_task.is_some())
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        window
            .update(cx, |_, window, _| {
                assert!(window.scene_reclaim_task.is_none());
                let scene = &window.rendered_frame.scene;
                assert!(scene.paint_operations.capacity() < before / 2);
                assert_eq!(scene.quads.len(), 8);
                let mut replay = Scene::default();
                replay.replay(0..scene.len(), scene);
                assert_eq!(replay.quads.len(), scene.quads.len());
                for (actual, expected) in replay.quads.iter().zip(&scene.quads) {
                    assert_eq!(actual.bounds, expected.bounds);
                }
                assert!(window.next_frame.scene.paint_operations.capacity() < before / 2);
            })
            .unwrap();
    }

    #[crate::test]
    fn returning_to_large_scene_cancels_pending_reclaim(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, _| Content(18000));
        draw(window, 18000, cx);
        draw(window, 18000, cx);
        draw(window, 0, cx);
        window
            .update(cx, |_, window, _| {
                assert!(window.scene_reclaim_task.is_some())
            })
            .unwrap();
        draw(window, 18000, cx);
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        window
            .update(cx, |_, window, _| {
                assert!(window.scene_reclaim_task.is_none());
                assert_eq!(window.rendered_frame.scene.quads.len(), 18000);
                assert!(window.next_frame.scene.quads.capacity() >= 18000);
                assert!(
                    window.next_frame.scene.paint_operations.capacity()
                        >= window.rendered_frame.scene.paint_operations.len()
                );
            })
            .unwrap();
    }
}
