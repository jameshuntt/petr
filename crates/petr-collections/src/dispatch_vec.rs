use core::ptr::NonNull;

use petr_core::{ProphecyBits, SelectorWord, TagResult, TaggedNonNull};
use petr_dispatch::{DispatchResult, OpFrame, OperationTable};

/// One Vec item plus selector/prophecy metadata.
#[derive(Debug)]
pub struct DispatchHandle<T, const LOW_BITS: usize> {
    /// Pointer with low-bit state tag.
    pub ptr: TaggedNonNull<T, LOW_BITS>,
    /// Trait/impl/function selector.
    pub selector: SelectorWord,
    /// Prediction bits.
    pub prophecy: ProphecyBits,
}

impl<T, const LOW_BITS: usize> Copy for DispatchHandle<T, LOW_BITS> {}
impl<T, const LOW_BITS: usize> Clone for DispatchHandle<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Vec-like structure that stores data and a selector for what operation to run.
#[derive(Debug)]
pub struct DispatchVec<T, const LOW_BITS: usize> {
    items: Vec<Box<T>>,
    handles: Vec<DispatchHandle<T, LOW_BITS>>,
}

impl<T, const LOW_BITS: usize> Default for DispatchVec<T, LOW_BITS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const LOW_BITS: usize> DispatchVec<T, LOW_BITS> {
    /// Create an empty dispatch Vec.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            handles: Vec::new(),
        }
    }

    /// Push a value with low tag, selector, and prophecy.
    pub fn push(
        &mut self,
        value: T,
        low_tag: usize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> TagResult<usize> {
        let mut boxed = Box::new(value);
        let ptr = TaggedNonNull::new(NonNull::from(boxed.as_mut()), low_tag)?;
        let idx = self.items.len();
        self.items.push(boxed);
        self.handles.push(DispatchHandle {
            ptr,
            selector,
            prophecy,
        });
        Ok(idx)
    }

    /// Iterate over handles.
    pub fn handles(&self) -> impl Iterator<Item = &DispatchHandle<T, LOW_BITS>> + '_ {
        self.handles.iter()
    }

    /// Run each handle's selector against an operation table.
    pub fn dispatch_all(
        &self,
        table: &OperationTable,
        seed_frame: OpFrame,
    ) -> DispatchResult<Vec<OpFrame>> {
        let mut out = Vec::with_capacity(self.handles.len());
        for h in &self.handles {
            if h.prophecy.skip_hint {
                out.push(seed_frame.clone());
            } else {
                out.push(table.execute_word(h.selector, seed_frame.clone())?);
            }
        }
        Ok(out)
    }
}
