use core::ptr::NonNull;

use petr_core::{TagResult, TaggedNonNull};

/// Handle into a `TaggedSlab`.
#[derive(Debug)]
pub struct SlabHandle<T, const LOW_BITS: usize> {
    /// Index into the slab slot table.
    pub index: usize,
    /// Generation counter for stale-handle detection.
    pub generation: u32,
    /// Tagged pointer for fast metadata access.
    pub tagged: TaggedNonNull<T, LOW_BITS>,
}

impl<T, const LOW_BITS: usize> Copy for SlabHandle<T, LOW_BITS> {}
impl<T, const LOW_BITS: usize> Clone for SlabHandle<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug)]
struct Slot<T> {
    value: Option<Box<T>>,
    generation: u32,
}

/// Slab with stable boxed allocations and tagged handles.
#[derive(Debug)]
pub struct TaggedSlab<T, const LOW_BITS: usize> {
    slots: Vec<Slot<T>>,
    free: Vec<usize>,
}

impl<T, const LOW_BITS: usize> Default for TaggedSlab<T, LOW_BITS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const LOW_BITS: usize> TaggedSlab<T, LOW_BITS> {
    /// Create an empty slab.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Insert a value and return a tagged handle.
    pub fn insert(&mut self, value: T, tag: usize) -> TagResult<SlabHandle<T, LOW_BITS>> {
        let mut boxed = Box::new(value);
        let tagged = TaggedNonNull::new(NonNull::from(boxed.as_mut()), tag)?;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index];
            slot.value = Some(boxed);
            Ok(SlabHandle {
                index,
                generation: slot.generation,
                tagged,
            })
        } else {
            let index = self.slots.len();
            self.slots.push(Slot {
                value: Some(boxed),
                generation: 0,
            });
            Ok(SlabHandle {
                index,
                generation: 0,
                tagged,
            })
        }
    }

    /// Get a shared reference by handle.
    pub fn get(&self, handle: SlabHandle<T, LOW_BITS>) -> Option<&T> {
        let slot = self.slots.get(handle.index)?;
        if slot.generation == handle.generation {
            slot.value.as_deref()
        } else {
            None
        }
    }

    /// Remove a value by handle.
    pub fn remove(&mut self, handle: SlabHandle<T, LOW_BITS>) -> Option<T> {
        let slot = self.slots.get_mut(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        let boxed = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(handle.index);
        Some(*boxed)
    }

    /// Number of occupied slots.
    pub fn occupied(&self) -> usize {
        self.slots.iter().filter(|s| s.value.is_some()).count()
    }
}
