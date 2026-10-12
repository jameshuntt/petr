use core::ptr::NonNull;

use petr_core::{TagResult, TaggedNonNull};

/// Vec-like collection whose handles are stable because each item is boxed.
#[derive(Debug)]
pub struct TaggedVec<T, const LOW_BITS: usize> {
    items: Vec<Box<T>>,
    handles: Vec<TaggedNonNull<T, LOW_BITS>>,
}

impl<T, const LOW_BITS: usize> Default for TaggedVec<T, LOW_BITS> {
    fn default() -> Self { Self::new() }
}

impl<T, const LOW_BITS: usize> TaggedVec<T, LOW_BITS> {
    /// Create an empty collection.
    pub fn new() -> Self { Self { items: Vec::new(), handles: Vec::new() } }

    /// Push an item with a tag and return its index.
    pub fn push(&mut self, value: T, tag: usize) -> TagResult<usize> {
        let mut boxed = Box::new(value);
        let ptr = NonNull::from(boxed.as_mut());
        let handle = TaggedNonNull::new(ptr, tag)?;
        let idx = self.items.len();
        self.items.push(boxed);
        self.handles.push(handle);
        Ok(idx)
    }

    /// Return a tagged handle by index.
    pub fn handle(&self, index: usize) -> Option<TaggedNonNull<T, LOW_BITS>> {
        self.handles.get(index).copied()
    }

    /// Return a shared reference by index.
    pub fn get(&self, index: usize) -> Option<&T> { self.items.get(index).map(Box::as_ref) }

    /// Return a mutable reference by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> { self.items.get_mut(index).map(Box::as_mut) }

    /// Change the tag associated with an element.
    pub fn set_tag(&mut self, index: usize, tag: usize) -> TagResult<()> {
        if let Some(handle) = self.handles.get_mut(index) {
            *handle = handle.with_tag(tag)?;
        }
        Ok(())
    }

    /// Number of elements.
    pub fn len(&self) -> usize { self.items.len() }

    /// Return true if empty.
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}
