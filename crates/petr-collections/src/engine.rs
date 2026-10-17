use petr_core::{Key, ProphecyBits, SelectorWord, TagResult};
use petr_dispatch::{DispatchResult, OpFrame, OperationTable};

use crate::{EncodedArenaHandle, KeyedArena, KeyedArenaHandle, KeyedArenaResult};

/// A small runtime facade combining a keyed arena with a dispatch table.
///
/// `ArenaEngine` is the higher-level shape most applications should use: the
/// arena owns/validates objects and the dispatch table routes selector words.
#[derive(Debug)]
pub struct ArenaEngine<T, const LOW_BITS: usize> {
    arena: KeyedArena<T, LOW_BITS>,
    table: OperationTable,
}

impl<T, const LOW_BITS: usize> ArenaEngine<T, LOW_BITS> {
    /// Create an engine with an OS-random arena key and caller-provided table.
    pub fn new_randomized(table: OperationTable) -> Self {
        Self {
            arena: KeyedArena::new_randomized(),
            table,
        }
    }

    /// Create an engine with deterministic key and caller-provided table.
    pub fn with_key_and_table(key: Key<LOW_BITS>, table: OperationTable) -> Self {
        Self {
            arena: KeyedArena::with_key(key),
            table,
        }
    }

    /// Borrow the underlying arena.
    pub const fn arena(&self) -> &KeyedArena<T, LOW_BITS> {
        &self.arena
    }

    /// Mutably borrow the underlying arena.
    pub fn arena_mut(&mut self) -> &mut KeyedArena<T, LOW_BITS> {
        &mut self.arena
    }

    /// Borrow the dispatch table.
    pub const fn table(&self) -> &OperationTable {
        &self.table
    }

    /// Mutably borrow the dispatch table.
    pub fn table_mut(&mut self) -> &mut OperationTable {
        &mut self.table
    }

    /// Insert a value with tag, selector, and prophecy metadata.
    pub fn insert(
        &mut self,
        value: T,
        tag: usize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> TagResult<KeyedArenaHandle<T, LOW_BITS>> {
        self.arena.insert(value, tag, selector, prophecy)
    }

    /// Validate and read a value.
    pub fn get(&self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<&T> {
        self.arena.get(handle)
    }

    /// Validate and mutably read a value.
    pub fn get_mut(&mut self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<&mut T> {
        self.arena.get_mut(handle)
    }

    /// Remove a value through a validated handle.
    pub fn remove(&mut self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<T> {
        self.arena.remove(handle)
    }

    /// Dispatch one handle through the engine's table.
    pub fn dispatch(
        &self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        seed_frame: OpFrame,
    ) -> KeyedArenaResult<DispatchResult<OpFrame>> {
        self.arena.dispatch_handle(&self.table, handle, seed_frame)
    }

    /// Dispatch all live handles through the engine's table.
    pub fn dispatch_all(
        &self,
        seed_frame: OpFrame,
    ) -> KeyedArenaResult<Vec<DispatchResult<OpFrame>>> {
        self.arena.dispatch_all(&self.table, seed_frame)
    }

    /// Update the low tag and return the refreshed handle.
    pub fn set_tag(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        tag: usize,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        self.arena.set_tag(handle, tag)
    }

    /// Update the selector and return the refreshed handle.
    pub fn set_selector(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        selector: SelectorWord,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        self.arena.set_selector(handle, selector)
    }

    /// Update prophecy bits and return the refreshed handle.
    pub fn set_prophecy(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        prophecy: ProphecyBits,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        self.arena.set_prophecy(handle, prophecy)
    }

    /// Encode a handle into session-local metadata.
    pub fn encode_handle(
        &self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
    ) -> KeyedArenaResult<EncodedArenaHandle> {
        self.arena.validate_handle(handle)?;
        Ok(EncodedArenaHandle::from(handle))
    }

    /// Decode and validate a session-local handle.
    pub fn decode_handle(
        &self,
        encoded: EncodedArenaHandle,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        let handle =
            encoded
                .to_handle::<T, LOW_BITS>()
                .ok_or(crate::KeyedArenaError::CheckMismatch {
                    index: encoded.index,
                })?;
        self.arena.validate_handle(handle)?;
        Ok(handle)
    }
}
