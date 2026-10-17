use core::{
    num::NonZeroUsize,
    ptr::{self, NonNull},
};
use std::time::{SystemTime, UNIX_EPOCH};

use zeroize::Zeroize;

use crate::{LowTagLayout, TagError, TagResult};

/// Per-collection or per-process XOR key used to hide/check clean pointer addresses.
///
/// The key clears the low tag bits so `stored = (addr ^ key) | tag` preserves the
/// tag field. This is a lightweight capability/check primitive, not a substitute
/// for cryptographic authentication or Rust aliasing/lifetime rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key<const LOW_BITS: usize> {
    mask: usize,
}

impl<const LOW_BITS: usize> Key<LOW_BITS> {
    /// Build a key from a deterministic seed. Low tag bits are cleared so low tags survive ORing.
    pub fn from_seed(seed: u64) -> Self {
        let mut x = seed as usize;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let mask = x & LowTagLayout::<LOW_BITS>::CLEAN_MASK;
        Self {
            mask: if mask == 0 {
                LowTagLayout::<LOW_BITS>::ALIGN << 7
            } else {
                mask
            },
        }
    }

    /// Build a key from OS entropy, falling back to process-local mixing only if OS entropy fails.
    pub fn random() -> Self {
        let mut bytes = [0u8; core::mem::size_of::<usize>()];
        if getrandom::getrandom(&mut bytes).is_ok() {
            let mut x = usize::from_ne_bytes(bytes);
            x &= LowTagLayout::<LOW_BITS>::CLEAN_MASK;
            if x == 0 {
                x = LowTagLayout::<LOW_BITS>::ALIGN << 9;
            }
            bytes.zeroize();
            return Self { mask: x };
        }
        Self::process_local_fallback()
    }

    /// Build a process-local-ish key from time and a stack address.
    ///
    /// Prefer [`Key::random`]. This exists as a no-std-hostile fallback shape for demos.
    pub fn process_local() -> Self {
        Self::random()
    }

    /// Build a deterministic process-local fallback when OS entropy is unavailable.
    pub fn process_local_fallback() -> Self {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let stack_probe = 0usize;
        let addr = (&stack_probe as *const usize as usize) as u64;
        Self::from_seed(t ^ addr.rotate_left(17))
    }

    /// Return the raw mask.
    pub const fn mask(self) -> usize {
        self.mask
    }

    /// Mask a clean aligned address and attach a low tag.
    pub fn mask_addr(self, addr: usize, tag: usize) -> TagResult<NonZeroUsize> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        LowTagLayout::<LOW_BITS>::validate_addr(addr)?;
        let word = (addr ^ self.mask) | tag;
        NonZeroUsize::new(word).ok_or(TagError::NullAddress)
    }

    /// Reveal a clean address from a masked word.
    pub fn reveal_addr(self, stored: NonZeroUsize) -> TagResult<usize> {
        let masked_clean = stored.get() & LowTagLayout::<LOW_BITS>::CLEAN_MASK;
        let addr = masked_clean ^ self.mask;
        if addr == 0 {
            return Err(TagError::NullAddress);
        }
        LowTagLayout::<LOW_BITS>::validate_addr(addr)?;
        Ok(addr)
    }
}

/// Secret key material that zeroizes its mask on drop.
///
/// Use this when a component owns a key for a long-lived arena and should avoid
/// accidentally keeping the raw mask in memory after teardown. Convert to `Key`
/// for fast copyable hot-path operations.
#[derive(Debug, PartialEq, Eq)]
pub struct KeyMaterial<const LOW_BITS: usize> {
    mask: usize,
}

impl<const LOW_BITS: usize> KeyMaterial<LOW_BITS> {
    /// Create key material from OS entropy.
    pub fn random() -> Self {
        Self {
            mask: Key::<LOW_BITS>::random().mask(),
        }
    }

    /// Create deterministic key material for tests.
    pub fn from_seed(seed: u64) -> Self {
        Self {
            mask: Key::<LOW_BITS>::from_seed(seed).mask(),
        }
    }

    /// Return the copyable key view.
    pub const fn key(&self) -> Key<LOW_BITS> {
        Key { mask: self.mask }
    }
}

impl<const LOW_BITS: usize> Drop for KeyMaterial<LOW_BITS> {
    fn drop(&mut self) {
        self.mask.zeroize();
    }
}

/// A pointer word whose clean address is XOR-masked by a `Key`.
///
/// This type is useful for validation/check words. Hardened owning collections
/// should still keep the original `NonNull<T>` internally and use the masked word
/// as a capability check rather than as the sole provenance source.
#[derive(Debug)]
pub struct KeyedTaggedNonNull<T: ?Sized, const LOW_BITS: usize> {
    stored: NonZeroUsize,
    provenance: Option<NonNull<T>>,
}

impl<T: ?Sized, const LOW_BITS: usize> Copy for KeyedTaggedNonNull<T, LOW_BITS> {}
impl<T: ?Sized, const LOW_BITS: usize> Clone for KeyedTaggedNonNull<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize> KeyedTaggedNonNull<T, LOW_BITS> {
    /// Mask a pointer address with `key` and attach a low tag.
    pub fn new(ptr: NonNull<T>, tag: usize, key: Key<LOW_BITS>) -> TagResult<Self> {
        let addr = ptr.as_ptr() as usize;
        let stored = key.mask_addr(addr, tag)?;
        Ok(Self {
            stored,
            provenance: Some(ptr),
        })
    }

    /// Create from a stored nonzero word.
    ///
    /// This validates nothing and carries no allocation provenance. Use it for
    /// stored/check words, not for direct dereference.
    pub const fn from_stored_word(stored: NonZeroUsize) -> Self {
        Self {
            stored,
            provenance: None,
        }
    }

    /// Return the stored masked word.
    pub const fn stored_word(self) -> usize {
        self.stored.get()
    }

    /// Return the stored masked nonzero word.
    pub const fn stored_nonzero(self) -> NonZeroUsize {
        self.stored
    }

    /// Return the low tag without revealing the address.
    pub const fn tag(self) -> usize {
        self.stored.get() & LowTagLayout::<LOW_BITS>::LOW_MASK
    }

    /// Reveal the clean address using `key`.
    pub fn reveal_addr(self, key: Key<LOW_BITS>) -> TagResult<usize> {
        key.reveal_addr(self.stored)
    }

    /// Reveal the clean pointer using `key`.
    ///
    /// Prefer collection-owned deref paths that preserve original pointer provenance.
    pub fn reveal(self, key: Key<LOW_BITS>) -> TagResult<NonNull<T>> {
        let addr = self.reveal_addr(key)?;
        if let Some(ptr) = self.provenance
            && ptr.as_ptr() as usize == addr
        {
            return Ok(ptr);
        }
        let raw = ptr::without_provenance_mut::<T>(addr);
        NonNull::new(raw).ok_or(TagError::NullAddress)
    }
}
