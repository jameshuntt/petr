use petr_dispatch::{DispatchResult, OpFrame, OperationTable};
use petr_core::{Key, ProphecyBits, SelectorWord, TagResult};

use crate::{KeyedArena, KeyedArenaError, KeyedArenaHandle, KeyedArenaResult};

/// Opaque handle for a keyed dispatch vector entry.
///
/// This is the same hardened handle type used by [`KeyedArena`]. The dispatch
/// vector is intentionally a policy wrapper over the arena rather than a second
/// independent implementation of handle validation.
pub type KeyedDispatchHandle<T, const LOW_BITS: usize> = KeyedArenaHandle<T, LOW_BITS>;

/// Error returned when a keyed dispatch handle does not validate.
pub type KeyedDispatchError = KeyedArenaError;

/// Result type for keyed dispatch collections.
pub type KeyedDispatchResult<T> = KeyedArenaResult<T>;

/// A Vec-like policy layer over [`KeyedArena`].
///
/// The arena owns the storage and performs all validation. This wrapper keeps
/// the API shape from the earlier `KeyedDispatchVec` while avoiding duplicate
/// pointer/key/generation logic.
#[derive(Debug)]
pub struct KeyedDispatchVec<T, const LOW_BITS: usize> {
    arena: KeyedArena<T, LOW_BITS>,
}

impl<T, const LOW_BITS: usize> Default for KeyedDispatchVec<T, LOW_BITS> {
    fn default() -> Self { Self::new_randomized() }
}

impl<T, const LOW_BITS: usize> KeyedDispatchVec<T, LOW_BITS> {
    /// Create with an OS-random key when available.
    pub fn new_randomized() -> Self { Self { arena: KeyedArena::new_randomized() } }

    /// Create with a deterministic key, mostly useful for tests.
    pub fn with_key(key: Key<LOW_BITS>) -> Self { Self { arena: KeyedArena::with_key(key) } }

    /// Return the collection key value.
    pub const fn key(&self) -> Key<LOW_BITS> { self.arena.key() }

    /// Number of live entries.
    pub const fn len(&self) -> usize { self.arena.len() }

    /// Return true when there are no live entries.
    pub const fn is_empty(&self) -> bool { self.arena.is_empty() }

    /// Number of allocated slots, including vacant reusable slots.
    pub fn slot_capacity(&self) -> usize { self.arena.slot_capacity() }

    /// Number of vacant reusable slots.
    pub fn free_len(&self) -> usize { self.arena.free_len() }

    /// Borrow the underlying arena.
    pub const fn arena(&self) -> &KeyedArena<T, LOW_BITS> { &self.arena }

    /// Mutably borrow the underlying arena.
    pub fn arena_mut(&mut self) -> &mut KeyedArena<T, LOW_BITS> { &mut self.arena }

    /// Push a value and return an opaque generation/capability handle.
    pub fn push(
        &mut self,
        value: T,
        tag: usize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> TagResult<KeyedDispatchHandle<T, LOW_BITS>> {
        self.arena.insert(value, tag, selector, prophecy)
    }

    /// Validate a handle and return the live slot index.
    pub fn validate_handle(&self, handle: KeyedDispatchHandle<T, LOW_BITS>) -> KeyedDispatchResult<usize> {
        self.arena.validate_handle(handle)
    }

    /// Return true if the handle currently resolves to a live entry.
    pub fn contains(&self, handle: KeyedDispatchHandle<T, LOW_BITS>) -> bool { self.arena.contains(handle) }

    /// Return a shared reference through a validated handle.
    pub fn get(&self, handle: KeyedDispatchHandle<T, LOW_BITS>) -> KeyedDispatchResult<&T> {
        self.arena.get(handle)
    }

    /// Return a mutable reference through a validated handle.
    pub fn get_mut(&mut self, handle: KeyedDispatchHandle<T, LOW_BITS>) -> KeyedDispatchResult<&mut T> {
        self.arena.get_mut(handle)
    }

    /// Return the current handle for an index if it is live.
    pub fn handle_at(&self, index: usize) -> Option<KeyedDispatchHandle<T, LOW_BITS>> {
        self.arena.handle_at(index)
    }

    /// Return all current live handles in slot order.
    pub fn live_handles(&self) -> Vec<KeyedDispatchHandle<T, LOW_BITS>> { self.arena.live_handles() }

    /// Set a low tag and refresh the check word, returning the updated handle.
    pub fn set_tag(
        &mut self,
        handle: KeyedDispatchHandle<T, LOW_BITS>,
        tag: usize,
    ) -> KeyedDispatchResult<KeyedDispatchHandle<T, LOW_BITS>> {
        self.arena.set_tag(handle, tag)
    }

    /// Set a selector and return the updated handle.
    pub fn set_selector(
        &mut self,
        handle: KeyedDispatchHandle<T, LOW_BITS>,
        selector: SelectorWord,
    ) -> KeyedDispatchResult<KeyedDispatchHandle<T, LOW_BITS>> {
        self.arena.set_selector(handle, selector)
    }

    /// Set prophecy bits and return the updated handle.
    pub fn set_prophecy(
        &mut self,
        handle: KeyedDispatchHandle<T, LOW_BITS>,
        prophecy: ProphecyBits,
    ) -> KeyedDispatchResult<KeyedDispatchHandle<T, LOW_BITS>> {
        self.arena.set_prophecy(handle, prophecy)
    }

    /// Remove a value. The old handle becomes stale immediately.
    pub fn remove(&mut self, handle: KeyedDispatchHandle<T, LOW_BITS>) -> KeyedDispatchResult<T> {
        self.arena.remove(handle)
    }

    /// Dispatch a single validated handle's selector.
    pub fn dispatch_handle(
        &self,
        table: &OperationTable,
        handle: KeyedDispatchHandle<T, LOW_BITS>,
        seed_frame: OpFrame,
    ) -> KeyedDispatchResult<DispatchResult<OpFrame>> {
        self.arena.dispatch_handle(table, handle, seed_frame)
    }

    /// Dispatch all live entries in slot order.
    pub fn dispatch_all(&self, table: &OperationTable, seed_frame: OpFrame) -> KeyedDispatchResult<Vec<DispatchResult<OpFrame>>> {
        self.arena.dispatch_all(table, seed_frame)
    }

    /// Remove all live entries.
    pub fn clear(&mut self) { self.arena.clear(); }
}
