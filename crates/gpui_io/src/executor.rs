use anyhow::Result;
use futures::{channel::oneshot, future::LocalBoxFuture};
use std::sync::Arc;

/// Injected dispatcher for blocking I/O. The callback must enqueue work off the UI thread.
#[derive(Clone)]
pub struct IoExecutor(Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>);

impl IoExecutor {
    /// Use an application's existing worker pool without depending on its async runtime.
    pub fn new(dispatch: impl Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static) -> Self {
        Self(Arc::new(dispatch))
    }

    /// Run blocking work. Once dispatched, cancellation does not interrupt the operation.
    pub fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> LocalBoxFuture<'static, Result<T>> {
        let executor = self.clone();
        Box::pin(async move {
            let (tx, rx) = oneshot::channel();
            executor.dispatch(move || {
                let _ = tx.send(work());
            });
            rx.await
                .map_err(|_| std::io::Error::other("I/O worker stopped"))?
        })
    }

    /// Enqueue cleanup that must outlive the caller's future.
    pub fn dispatch(&self, work: impl FnOnce() + Send + 'static) {
        (self.0)(Box::new(work));
    }
}
