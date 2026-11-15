use gpui::{CaretAffinity, SharedString};
use std::{collections::VecDeque, ops::Range};

pub(super) struct Snapshot {
    pub content: SharedString,
    pub selection: Range<usize>,
    pub reversed: bool,
    pub affinity: CaretAffinity,
}

/// Both directions share a byte budget. Large edits remain usable without retaining copies.
#[derive(Default)]
pub(super) struct History {
    undo: VecDeque<Snapshot>,
    redo: VecDeque<Snapshot>,
    bytes: usize,
}
impl History {
    const BYTES: usize = 1024 * 1024;
    const ENTRIES: usize = 100;
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn commit(&mut self, snapshot: Snapshot) {
        self.bytes -= self
            .redo
            .iter()
            .map(|entry| entry.content.len())
            .sum::<usize>();
        self.redo.clear();
        self.push(snapshot, false);
    }
    pub fn travel(&mut self, current: Snapshot, redo: bool) -> Option<Snapshot> {
        let snapshot = if redo {
            self.redo.pop_back()
        } else {
            self.undo.pop_back()
        }?;
        self.bytes -= snapshot.content.len();
        self.push(current, !redo);
        Some(snapshot)
    }
    fn push(&mut self, snapshot: Snapshot, redo: bool) {
        if snapshot.content.len() > Self::BYTES {
            self.clear();
            return;
        }
        self.bytes += snapshot.content.len();
        if redo {
            self.redo.push_back(snapshot);
        } else {
            self.undo.push_back(snapshot);
        }
        while self.bytes > Self::BYTES || self.undo.len() + self.redo.len() > Self::ENTRIES {
            let removed = if redo {
                self.redo.pop_front().or_else(|| self.undo.pop_front())
            } else {
                self.undo.pop_front().or_else(|| self.redo.pop_front())
            };
            if let Some(removed) = removed {
                self.bytes -= removed.content.len();
            }
        }
    }
}
