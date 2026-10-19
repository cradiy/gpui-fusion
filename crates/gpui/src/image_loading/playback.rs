use crate::{BackgroundExecutor, ImageAnimation, ImageCacheError, ImageId, RenderImage, Task};
use futures::FutureExt;
use scheduler::Instant;
use std::{sync::Arc, time::Duration};

pub(crate) type PendingFrame =
    futures::future::Shared<Task<Result<Arc<RenderImage>, ImageCacheError>>>;

pub(crate) struct AnimationPlayback {
    pub image_id: ImageId,
    source: Arc<ImageAnimation>,
    current: Arc<RenderImage>,
    index: usize,
    started_at: Option<Instant>,
    pending: Option<PendingFrame>,
    ready: Option<Arc<RenderImage>>,
    error: Option<ImageCacheError>,
    completed_plays: u32,
    finished: bool,
}

impl AnimationPlayback {
    pub fn new(poster: Arc<RenderImage>, source: Arc<ImageAnimation>) -> Self {
        Self {
            image_id: poster.id,
            source,
            current: poster,
            index: 0,
            started_at: None,
            pending: None,
            ready: None,
            error: None,
            completed_plays: 0,
            finished: false,
        }
    }

    pub fn update(
        &mut self,
        now: Instant,
        active: bool,
        executor: &BackgroundExecutor,
    ) -> Result<Arc<RenderImage>, ImageCacheError> {
        if let Some(result) = self.pending.as_mut().and_then(|task| task.now_or_never()) {
            self.pending = None;
            match result {
                Ok(image) => self.ready = Some(image),
                Err(error) => self.error = Some(error),
            }
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if !active || self.finished {
            self.started_at = None;
            return Ok(self.current.clone());
        }
        let started_at = *self.started_at.get_or_insert(now);
        let delay = Duration::from(self.current.delay(0)).max(Duration::from_millis(10));
        let final_frame = self.index + 1 == self.source.frame_count()
            && self
                .source
                .loop_count()
                .finished_after(self.completed_plays.saturating_add(1));
        if final_frame {
            if now - started_at >= delay {
                self.finished = true;
            }
            return Ok(self.current.clone());
        }
        if now - started_at >= delay
            && let Some(image) = self.ready.take()
        {
            self.current = image;
            self.index = (self.index + 1) % self.source.frame_count();
            if self.index == 0 {
                self.completed_plays = self.completed_plays.saturating_add(1);
            }
            self.started_at = Some(now);
        }
        if self.pending.is_none()
            && self.ready.is_none()
            && !(self.index + 1 == self.source.frame_count()
                && self
                    .source
                    .loop_count()
                    .finished_after(self.completed_plays.saturating_add(1)))
        {
            self.pending = Some(
                self.source
                    .frame((self.index + 1) % self.source.frame_count(), executor)
                    .shared(),
            );
        }
        Ok(self.current.clone())
    }

    pub fn wakeup(&self) -> Option<(Instant, Option<PendingFrame>)> {
        if self.finished || self.error.is_some() {
            return None;
        }
        let deadline =
            self.started_at? + Duration::from(self.current.delay(0)).max(Duration::from_millis(10));
        Some((deadline, self.pending.clone()))
    }
}
