use crate::bridge::Host;
use gpui::{PlatformDispatcher, Priority, RunnableVariant, queue::PriorityQueueReceiver};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, ThreadId},
    time::Duration,
};

pub(crate) struct AndroidDispatcher {
    owner: ThreadId,
    host: Arc<Host>,
    pending: Mutex<HashMap<u64, RunnableVariant>>,
    next: AtomicU64,
    closed: AtomicBool,
    background: gpui::queue::PriorityQueueSender<Option<RunnableVariant>>,
    worker_count: usize,
}

impl AndroidDispatcher {
    pub fn new(host: Arc<Host>) -> Arc<Self> {
        let (background, receiver) = PriorityQueueReceiver::<Option<RunnableVariant>>::new();
        let worker_count = thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4));
        for index in 0..worker_count {
            let receiver = receiver.clone();
            thread::Builder::new()
                .name(format!("gpui-worker-{index}"))
                .spawn(move || {
                    for runnable in receiver.iter() {
                        let Some(runnable) = runnable else {
                            break;
                        };
                        runnable.run();
                    }
                })
                .expect("failed to start GPUI worker");
        }
        Arc::new(Self {
            owner: thread::current().id(),
            host,
            pending: Mutex::default(),
            next: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            background,
            worker_count,
        })
    }

    fn post(&self, runnable: RunnableVariant, delay: Duration) {
        let mut pending = self.pending.lock();
        if self.closed.load(Ordering::Acquire) {
            // A foreground future may be !Send; a late worker wake must not
            // destroy it on the worker after its owning session has closed.
            std::mem::forget(runnable);
            return;
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        pending.insert(id, runnable);
        drop(pending);
        self.host.schedule(id, delay);
    }

    pub fn run(&self, id: u64) {
        assert!(self.is_main_thread());
        let task = self.pending.lock().remove(&id);
        if let Some(task) = task {
            task.run();
        }
    }

    pub fn close(&self) {
        assert!(self.is_main_thread());
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.stop_workers();
        let pending = std::mem::take(&mut *self.pending.lock());
        drop(pending);
    }

    fn stop_workers(&self) {
        // Explicit wakeups also release workers already sleeping in queue.pop().
        for _ in 0..self.worker_count {
            let _ = self.background.send(Priority::High, None);
        }
    }
}

impl Drop for AndroidDispatcher {
    fn drop(&mut self) {
        if !self.closed.load(Ordering::Acquire) {
            self.stop_workers();
        }
    }
}

impl PlatformDispatcher for AndroidDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.owner
    }
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        if priority == Priority::RealtimeAudio {
            self.spawn_realtime(Box::new(move || {
                runnable.run();
            }));
        } else {
            let _ = self.background.send(priority, Some(runnable));
        }
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, _: Priority) {
        self.post(runnable, Duration::ZERO);
    }
    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        self.post(runnable, duration);
    }
    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        thread::spawn(f);
    }
}
