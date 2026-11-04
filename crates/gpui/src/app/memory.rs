use super::*;

impl App {
    /// Schedules the return of unused allocator pages to the OS after a delay.
    /// The guard runs on the foreground thread when the delay expires; reclamation
    /// runs in the background only if it returns true. A newer request replaces
    /// a pending request. Reclamation already running cannot be cancelled.
    /// Returns false without scheduling work when the platform is unsupported.
    /// Live application data and caches are retained.
    pub fn release_unused_memory_after(
        &mut self,
        delay: Duration,
        guard: impl FnOnce(&mut App) -> bool + 'static,
    ) -> bool {
        let Some(reclaim) = self.platform.memory_reclaimer() else {
            return false;
        };
        let background = self.background_executor.clone();
        self.memory_release_task = Some(self.spawn(async move |cx| {
            background.timer(delay).await;
            if cx.update(guard) {
                background.spawn(async move { reclaim() }).await;
            }
        }));
        true
    }
}

#[derive(Default)]
pub(super) struct ElementArenaMemory {
    window_usage: FxHashMap<WindowId, usize>,
    last_activity: Option<Instant>,
    task: Option<Task<()>>,
}

impl ElementArenaMemory {
    fn retained_capacity(&self) -> usize {
        self.window_usage
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_mul(2)
    }
}

impl App {
    pub(crate) fn record_element_arena_usage(&mut self, window: WindowId) {
        let used = self.element_arena.borrow().used_capacity();
        self.element_arena_memory.window_usage.insert(window, used);
        self.schedule_element_arena_reclaim();
    }

    pub(crate) fn forget_element_arena_usage(&mut self, window: WindowId) {
        self.element_arena_memory.window_usage.remove(&window);
        self.schedule_element_arena_reclaim();
    }

    fn schedule_element_arena_reclaim(&mut self) {
        const IDLE_DELAY: Duration = Duration::from_secs(2);
        self.element_arena_memory.last_activity = Some(self.background_executor.now());
        let retained = self.element_arena_memory.retained_capacity();
        if !self.element_arena.borrow().needs_idle_reclaim(retained) {
            self.element_arena_memory.task = None;
            return;
        }
        if self.element_arena_memory.task.is_some() {
            return;
        }
        self.element_arena_memory.task = Some(self.spawn(async move |cx| {
            let mut delay = IDLE_DELAY;
            loop {
                cx.background_executor().timer(delay).await;
                let (remaining, reclaimer) = cx.update(|cx| {
                    let elapsed = cx.background_executor.now()
                        - cx.element_arena_memory.last_activity.unwrap();
                    if elapsed < IDLE_DELAY {
                        return (Some(IDLE_DELAY - elapsed), None);
                    }
                    let retained = cx.element_arena_memory.retained_capacity();
                    let reclaimed = cx.element_arena.borrow_mut().reclaim_idle_memory(retained);
                    cx.element_arena_memory.task = None;
                    (
                        None,
                        reclaimed.then(|| cx.platform.memory_reclaimer()).flatten(),
                    )
                });
                if let Some(remaining) = remaining {
                    delay = remaining;
                } else {
                    if let Some(reclaim) = reclaimer {
                        cx.background_executor()
                            .spawn(async move { reclaim() })
                            .detach();
                    }
                    break;
                }
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, IntoElement, ParentElement, Styled, TestAppContext, div, px};

    struct Content(usize);

    impl Render for Content {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .children((0..self.0).map(|_| div().absolute().size(px(1.))))
        }
    }

    fn draw(window: WindowHandle<Content>, count: usize, cx: &mut TestAppContext) {
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
    fn arena_idle_reclaim_tracks_all_windows_and_activity(cx: &mut TestAppContext) {
        let large = cx.add_window(|_, _| Content(6000));
        let small = cx.add_window(|_, _| Content(0));
        draw(large, 6000, cx);
        draw(small, 0, cx);
        let peak = cx.update(|cx| cx.element_arena.borrow().capacity());
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(cx.element_arena.borrow().capacity(), peak));
        draw(large, 0, cx);
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        draw(small, 0, cx);
        cx.executor().advance_clock(Duration::from_millis(1500));
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(cx.element_arena.borrow().capacity(), peak));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        cx.update(|cx| assert!(cx.element_arena.borrow().capacity() < peak / 2));
        draw(large, 6000, cx);
        cx.update(|cx| assert!(cx.element_arena.borrow().capacity() >= peak));
        large
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(cx.element_arena.borrow().capacity() < peak / 2);
            assert!(
                !cx.element_arena_memory
                    .window_usage
                    .contains_key(&large.window_id())
            );
        });
    }
}
