use std::sync::atomic::{AtomicUsize, Ordering};

/// Atomic storage for tagged pointer words.
///
/// This type intentionally stores raw words only. Reconstructing a pointer from
/// an atomic word is an unsafe-edge operation that must validate lifetime and ownership.
#[derive(Debug)]
pub struct AtomicTaggedWord {
    word: AtomicUsize,
}

impl AtomicTaggedWord {
    /// Create a new atomic word.
    pub const fn new(word: usize) -> Self {
        Self {
            word: AtomicUsize::new(word),
        }
    }

    /// Load the word.
    pub fn load(&self, ordering: Ordering) -> usize {
        self.word.load(ordering)
    }

    /// Store the word.
    pub fn store(&self, word: usize, ordering: Ordering) {
        self.word.store(word, ordering);
    }

    /// Compare-and-exchange the word.
    pub fn compare_exchange(
        &self,
        current: usize,
        new: usize,
        success: Ordering,
        failure: Ordering,
    ) -> Result<usize, usize> {
        self.word.compare_exchange(current, new, success, failure)
    }
}
