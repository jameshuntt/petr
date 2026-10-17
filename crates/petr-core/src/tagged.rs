use core::ptr::{self, NonNull};

use crate::{LowTagLayout, TagError, TagResult};

/// A non-owning pointer plus metadata packed into low alignment bits.
///
/// This wrapper keeps the original `NonNull<T>` provenance when it is created
/// from an actual pointer. The full tagged word is still available for logging,
/// comparison, atomics, or handle/check construction, but dereference paths use
/// the stored clean pointer rather than reconstructing a pointer from an integer.
///
/// `from_word` is intentionally weaker: it validates the shape of a raw word and
/// creates a pointer with no exposed allocation provenance. Such values are fine
/// for address/tag inspection, but should not be dereferenced unless the caller
/// has separately re-established validity.
#[derive(Debug)]
pub struct TaggedNonNull<T: ?Sized, const LOW_BITS: usize> {
    ptr: NonNull<T>,
    tag: usize,
}

impl<T: ?Sized, const LOW_BITS: usize> Copy for TaggedNonNull<T, LOW_BITS> {}
impl<T: ?Sized, const LOW_BITS: usize> Clone for TaggedNonNull<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize> TaggedNonNull<T, LOW_BITS> {
    /// Create a tagged pointer from a clean pointer and a low tag.
    pub fn new(ptr: NonNull<T>, tag: usize) -> TagResult<Self> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        LowTagLayout::<LOW_BITS>::validate_addr(ptr.as_ptr() as usize)?;
        Ok(Self { ptr, tag })
    }

    /// Create a tagged pointer from a mutable reference.
    pub fn from_mut(value: &mut T, tag: usize) -> TagResult<Self> {
        Self::new(NonNull::from(value), tag)
    }

    /// Create from a raw word.
    ///
    /// This validates the raw address/tag shape, but it cannot recover Rust
    /// pointer provenance from the integer. The resulting pointer is suitable
    /// for address/tag inspection. Prefer [`TaggedNonNull::new`] whenever an
    /// actual pointer is available.
    pub fn from_word(word: usize) -> TagResult<Self> {
        let tag = LowTagLayout::<LOW_BITS>::low_tag(word);
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        let clean = LowTagLayout::<LOW_BITS>::clean_word(word);
        if clean == 0 {
            return Err(TagError::NullAddress);
        }
        LowTagLayout::<LOW_BITS>::validate_addr(clean)?;
        let raw = ptr::without_provenance_mut::<T>(clean);
        let ptr = NonNull::new(raw).ok_or(TagError::NullAddress)?;
        Ok(Self { ptr, tag })
    }

    /// Return the full tagged word.
    pub fn word(self) -> usize {
        self.clean_addr() | self.tag
    }

    /// Return the low tag.
    pub const fn tag(self) -> usize {
        self.tag
    }

    /// Return the cleaned address word.
    pub fn clean_addr(self) -> usize {
        self.ptr.as_ptr() as usize
    }

    /// Return the cleaned pointer while preserving provenance when available.
    pub const fn clean(self) -> NonNull<T> {
        self.ptr
    }

    /// Return a copy with a different low tag.
    pub fn with_tag(self, tag: usize) -> TagResult<Self> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        Ok(Self { ptr: self.ptr, tag })
    }

    /// Shared reference through the cleaned pointer.
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid for shared access. In
    /// particular, values constructed with `from_word` do not recover allocation
    /// provenance and should not be dereferenced unless the caller has an
    /// independent validity proof.
    pub unsafe fn as_ref<'a>(self) -> &'a T {
        // SAFETY: caller guarantees lifetime, validity, and aliasing.
        unsafe { self.ptr.as_ref() }
    }

    /// Mutable reference through the cleaned pointer.
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid and uniquely borrowed. Values
    /// constructed with `from_word` require an independent validity proof before
    /// dereference.
    pub unsafe fn as_mut<'a>(self) -> &'a mut T {
        let mut ptr = self.ptr;
        // SAFETY: caller guarantees lifetime, validity, and unique access.
        unsafe { ptr.as_mut() }
    }
}
