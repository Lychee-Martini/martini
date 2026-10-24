use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct DebounceQueue {
    pending: HashMap<PathBuf, Instant>,
    settle_duration: Duration,
}

impl DebounceQueue {
    pub fn new(settle_duration: Duration) -> Self {
        Self {
            pending: HashMap::new(),
            settle_duration,
        }
    }

    pub fn insert_or_update(&mut self, path: PathBuf) {
        let settle_at = Instant::now() + self.settle_duration;
        self.pending.insert(path, settle_at);
    }

    pub fn pop_settled(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let mut settled = Vec::new();

        self.pending.retain(|path, &mut settle_at| {
            if now >= settle_at {
                settled.push(path.clone());
                false
            } else {
                true
            }
        });

        settled
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}
