//! One outstanding compositor callback per surface, including across forced draws.

pub(super) struct PendingFrameCallback<T> {
    pending: Option<T>,
}

impl<T> Default for PendingFrameCallback<T> {
    fn default() -> Self {
        Self { pending: None }
    }
}

impl<T: PartialEq> PendingFrameCallback<T> {
    pub(super) fn request(&mut self, request: impl FnOnce() -> T) -> bool {
        if self.pending.is_none() {
            self.pending = Some(request());
            true
        } else {
            false
        }
    }

    pub(super) fn complete(&mut self, callback: &T) -> bool {
        if self.pending.as_ref() != Some(callback) {
            return false;
        }
        self.pending = None;
        true
    }

    pub(super) fn clear(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::PendingFrameCallback;

    #[test]
    fn resize_bursts_keep_a_single_frame_callback_chain() {
        let mut frame = PendingFrameCallback::default();
        let mut issued = 0;
        for refresh in 1..=64 {
            // A resize/recovery can draw immediately while the compositor's
            // callback for the previous frame is still outstanding.
            for request in 0..32 {
                let needs_commit = frame.request(|| {
                    issued += 1;
                    issued
                });
                assert_eq!(needs_commit, request == 0);
            }
            assert_eq!(issued, refresh);
            assert!(frame.complete(&refresh));
            assert!(!frame.complete(&refresh));
        }
    }

    #[test]
    fn callbacks_from_before_unmap_cannot_restart_or_replace_the_chain() {
        let mut frame = PendingFrameCallback::default();
        assert!(frame.request(|| 1));
        frame.clear();
        assert!(!frame.complete(&1));

        assert!(frame.request(|| 2));
        assert!(!frame.complete(&1));
        assert!(!frame.request(|| panic!("remap already has an outstanding callback")));
        assert!(frame.complete(&2));
        assert!(frame.request(|| 3));
        assert!(frame.complete(&3));
    }
}
