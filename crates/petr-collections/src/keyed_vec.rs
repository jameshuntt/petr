use core::ptr::NonNull;

use petr_core::{Key, KeyedTaggedNonNull, TagResult};

/// Vec-like collection with per-collection IV/key-masked pointer handles.
#[derive(Debug)]
pub struct KeyedTaggedVec<T, const LOW_BITS: usize> {
    key: Key<LOW_BITS>,
    items: Vec<Box<T>>,
    handles: Vec<KeyedTaggedNonNull<T, LOW_BITS>>,
}

impl<T, const LOW_BITS: usize> KeyedTaggedVec<T, LOW_BITS> {
    /// Create a collection with a process-local key.
    pub fn new_randomized() -> Self { Self::with_key(Key::process_local()) }

    /// Create a collection with a deterministic key.
    pub fn with_key(key: Key<LOW_BITS>) -> Self { Self { key, items: Vec::new(), handles: Vec::new() } }

    /// Return the collection key.
    pub const fn key(&self) -> Key<LOW_BITS> { self.key }

    /// Push a value and store a keyed pointer handle.
    pub fn push(&mut self, value: T, tag: usize) -> TagResult<usize> {
        let mut boxed = Box::new(value);
        let ptr = NonNull::from(boxed.as_mut());
        let handle = KeyedTaggedNonNull::new(ptr, tag, self.key)?;
        let idx = self.items.len();
        self.items.push(boxed);
        self.handles.push(handle);
        Ok(idx)
    }

    /// Return the stored masked handle.
    pub fn handle(&self, index: usize) -> Option<KeyedTaggedNonNull<T, LOW_BITS>> {
        self.handles.get(index).copied()
    }

    /// Reveal a handle and return a shared reference if it points into this collection.
    pub fn get_by_handle(&self, handle: KeyedTaggedNonNull<T, LOW_BITS>) -> TagResult<Option<&T>> {
        let addr = handle.reveal_addr(self.key)?;
        Ok(self.items.iter().find(|item| (item.as_ref() as *const T as usize) == addr).map(Box::as_ref))
    }

    /// Number of items.
    pub fn len(&self) -> usize { self.items.len() }

    /// Return true when the collection has no items.
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}
