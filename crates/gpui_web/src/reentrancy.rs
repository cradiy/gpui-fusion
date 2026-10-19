//! Guards against re-entrant platform callbacks.
//!
//! DOM listeners run synchronously and can re-enter GPUI: `autofill.js` forwards
//! form events onto the canvas, and a handler may synchronously focus an input,
//! resize the canvas, or request a frame, which fires another listener while the
//! first one is still on the stack.
//!
//! Callbacks are stored in a `RefCell`, so the naive
//! `let mut cbs = callbacks.borrow_mut(); cbs.input.as_mut().map(...)` panics with
//! "RefCell already borrowed" as soon as that happens. On wasm32 a panic aborts
//! rather than unwinds, so the `RefMut` drop guard never runs and the borrow flag
//! stays set forever: every later event panics too, which is the endless console
//! spam users see.
//!
//! So a callback is never invoked while the borrow is held: it is moved out, the
//! borrow is released, the callback runs, and it is moved back. A re-entrant call
//! then finds the slot empty and is dropped, which is the right outcome for input
//! and status callbacks because the inner event is a consequence of the event
//! already being handled.
//!
//! Resize is delivered through `window::reentrant_resize`, which drops a nested
//! notification instead of queueing it: replaying from inside the callback can
//! re-trigger the observer forever.
/// Invokes `$cell.$field` without holding the `RefCell` borrow, dropping the call
/// when the callback is already running.
///
/// Expands to the callback's return value wrapped in `Option`, which is `None`
/// when the slot is empty.
macro_rules! invoke_callback {
    ($cell:expr, $field:ident, $callback:ident, $body:block) => {{
        let cell: &_ = &$cell;
        let mut slot = cell.borrow_mut().$field.take();
        let result = slot.as_mut().map(|$callback| $body);
        if let Some(callback) = slot {
            cell.borrow_mut().$field = Some(callback);
        }
        result
    }};
}

pub(crate) use invoke_callback;

/// Mutates a window state cell without panicking when it is already borrowed.
///
/// `WebWindowMutableState` wraps the renderer and is borrowed across every draw,
/// so a DOM listener that fires mid-frame (pointermove, visibilitychange,
/// resize) would otherwise panic with "RefCell already borrowed" and, on wasm32,
/// leave the cell borrowed forever. The renderer reads these fields at the start
/// of the next frame, so dropping the write is harmless: the following event
/// carries the same information.
macro_rules! update_state {
    ($cell:expr, |$state:ident| $body:block) => {{
        if let Ok(mut $state) = $cell.try_borrow_mut() {
            $body
        }
    }};
}

pub(crate) use update_state;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Callbacks {
        counted: Option<Box<dyn FnMut(i32) -> i32>>,
    }

    fn callbacks() -> Rc<RefCell<Callbacks>> {
        Rc::new(RefCell::new(Callbacks::default()))
    }

    #[test]
    fn invoke_restores_the_callback() {
        let cell = callbacks();
        cell.borrow_mut().counted = Some(Box::new(|value| value * 2));

        assert_eq!(
            invoke_callback!(cell, counted, callback, { callback(2) }),
            Some(4)
        );
        assert_eq!(
            invoke_callback!(cell, counted, callback, { callback(3) }),
            Some(6)
        );
    }

    #[test]
    fn invoke_reports_an_empty_slot() {
        let cell = callbacks();
        assert_eq!(
            invoke_callback!(cell, counted, callback, { callback(2) }),
            None
        );
    }

    #[test]
    fn invoke_drops_a_reentrant_call_instead_of_panicking() {
        // The whole point: no "RefCell already borrowed" panic, and the callback
        // is still installed for the next event.
        let cell = callbacks();
        let inner = Rc::clone(&cell);
        cell.borrow_mut().counted = Some(Box::new(move |value| {
            assert_eq!(
                invoke_callback!(inner, counted, callback, { callback(value) }),
                None
            );
            value * 2
        }));

        assert_eq!(
            invoke_callback!(cell, counted, callback, { callback(2) }),
            Some(4)
        );
        assert_eq!(
            invoke_callback!(cell, counted, callback, { callback(4) }),
            Some(8)
        );
    }

    #[test]
    fn invoke_does_not_hold_the_borrow() {
        // A callback that needs to read another field must not deadlock on the
        // cell it was taken out of.
        let cell = callbacks();
        let inner = Rc::clone(&cell);
        cell.borrow_mut().counted = Some(Box::new(move |value| {
            assert!(inner.try_borrow_mut().is_ok());
            value
        }));

        assert_eq!(
            invoke_callback!(cell, counted, callback, { callback(7) }),
            Some(7)
        );
    }
}
