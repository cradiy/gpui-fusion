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
