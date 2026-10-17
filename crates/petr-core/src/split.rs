use core::ptr::{self, NonNull};

use crate::{LowTagLayout, TagError, TagResult};

/// A software pointer wrapper with both a top-byte/head tag and a low-bit/tail tag.
///
/// This keeps the original clean `NonNull<T>` when constructed from a pointer.
/// The top byte is represented only in the software word returned by [`Self::word`];
/// dereference paths use the stored clean pointer. This avoids passing top-tagged
/// addresses to code that may dereference them.
///
/// `from_word` validates a raw word but cannot recover Rust pointer provenance.
/// Prefer [`SplitTaggedNonNull::new`] for values that may be dereferenced.
#[derive(Debug)]
pub struct SplitTaggedNonNull<T: ?Sized, const LOW_BITS: usize> {
    ptr: NonNull<T>,
    top: u8,
    low: usize,
}

impl<T: ?Sized, const LOW_BITS: usize> Copy for SplitTaggedNonNull<T, LOW_BITS> {}
impl<T: ?Sized, const LOW_BITS: usize> Clone for SplitTaggedNonNull<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize> SplitTaggedNonNull<T, LOW_BITS> {
    /// Bit shift of the top/head byte on pointer-width platforms.
    pub const TOP_SHIFT: usize = usize::BITS as usize - 8;

    /// Mask of the top/head byte.
    pub const TOP_MASK: usize = 0xFFusize << Self::TOP_SHIFT;

    /// Mask for an address after both top/head and low/tail tags are removed.
    pub const CLEAN_MASK: usize = !(Self::TOP_MASK | LowTagLayout::<LOW_BITS>::LOW_MASK);

    /// Create a split-tagged pointer from a real pointer.
    pub fn new(ptr: NonNull<T>, top: u8, low: usize) -> TagResult<Self> {
        LowTagLayout::<LOW_BITS>::validate_tag(low)?;
        LowTagLayout::<LOW_BITS>::validate_addr(ptr.as_ptr() as usize)?;
        Ok(Self { ptr, top, low })
    }

    /// Create from a raw software word.
    ///
    /// This validates the word's shape, but it cannot recover allocation
    /// provenance from the integer address. Prefer [`Self::new`] when possible.
    pub fn from_word(word: usize) -> TagResult<Self> {
        let low = word & LowTagLayout::<LOW_BITS>::LOW_MASK;
        LowTagLayout::<LOW_BITS>::validate_tag(low)?;
        let clean = word & Self::CLEAN_MASK;
        if clean == 0 {
            return Err(TagError::NullAddress);
        }
        LowTagLayout::<LOW_BITS>::validate_addr(clean)?;
        let raw = ptr::without_provenance_mut::<T>(clean);
        let ptr = NonNull::new(raw).ok_or(TagError::NullAddress)?;
        let top = ((word & Self::TOP_MASK) >> Self::TOP_SHIFT) as u8;
        Ok(Self { ptr, top, low })
    }

    /// Return the full software word.
    pub fn word(self) -> usize {
        ((self.top as usize) << Self::TOP_SHIFT) | self.clean_addr() | self.low
    }

    /// Return the top/head byte.
    pub const fn top(self) -> u8 {
        self.top
    }

    /// Return the low/tail tag.
    pub const fn low(self) -> usize {
        self.low
    }

    /// Return the clean address with top and low tags removed.
    pub fn clean_addr(self) -> usize {
        self.ptr.as_ptr() as usize
    }

    /// Return the cleaned pointer while preserving provenance when available.
    pub const fn clean(self) -> NonNull<T> {
        self.ptr
    }

    /// Change the top/head tag.
    pub fn with_top(self, top: u8) -> TagResult<Self> {
        Ok(Self {
            ptr: self.ptr,
            top,
            low: self.low,
        })
    }

    /// Change the low/tail tag.
    pub fn with_low(self, low: usize) -> TagResult<Self> {
        LowTagLayout::<LOW_BITS>::validate_tag(low)?;
        Ok(Self {
            ptr: self.ptr,
            top: self.top,
            low,
        })
    }
}
