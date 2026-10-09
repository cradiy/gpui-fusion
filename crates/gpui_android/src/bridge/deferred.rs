use std::cell::RefCell;

#[derive(Default)]
struct Queue {
    depth: usize,
    callbacks: Vec<Box<dyn FnOnce()>>,
}

thread_local! {
    static QUEUE: RefCell<Queue> = RefCell::default();
}

// Return to a shallow native stack before entering Java to post asynchronous work.
// GPUI layout and paint can leave too little of Android's UI thread stack for JNI.
pub(super) struct Scope;

impl Scope {
    pub(super) fn enter() -> Self {
        QUEUE.with_borrow_mut(|queue| queue.depth += 1);
        Self
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let callbacks = QUEUE.with_borrow_mut(|queue| {
            queue.depth -= 1;
            if queue.depth == 0 {
                std::mem::take(&mut queue.callbacks)
            } else {
                Vec::new()
            }
        });
        for callback in callbacks {
            callback();
        }
    }
}

pub(super) fn post(callback: impl FnOnce() + 'static) {
    let callback = QUEUE.with_borrow_mut(|queue| {
        if queue.depth > 0 {
            queue.callbacks.push(Box::new(callback));
            None
        } else {
            Some(callback)
        }
    });
    if let Some(callback) = callback {
        callback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn callbacks_wait_for_outer_native_scope_and_can_post_more_work() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let outer = Scope::enter();
        let inner = Scope::enter();
        let captured = events.clone();
        post(move || {
            captured.borrow_mut().push(1);
            post(move || captured.borrow_mut().push(2));
        });
        drop(inner);
        assert!(events.borrow().is_empty());
        drop(outer);
        assert_eq!(*events.borrow(), [1, 2]);
        let captured = events.clone();
        post(move || captured.borrow_mut().push(3));
        assert_eq!(*events.borrow(), [1, 2, 3]);
    }
}
