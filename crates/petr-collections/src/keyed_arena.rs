use core::{marker::PhantomData, num::NonZeroUsize, ptr::NonNull};

use petr_dispatch::{DispatchResult, OpFrame, OperationTable};
use petr_core::{Key, LowTagLayout, ProphecyBits, SelectorWord, TagResult};

/// Opaque handle for a live [`KeyedArena`] entry.
///
/// The handle is intentionally copyable and inspectable, but it is not a raw
/// pointer. It only becomes useful when validated by the arena that created it.
/// The arena checks index, generation, keyed check word, selector, prophecy, and
/// revealed address before returning access to the stored value.
#[derive(Debug, PartialEq, Eq)]
pub struct KeyedArenaHandle<T, const LOW_BITS: usize> {
    index: u32,
    generation: u32,
    check: NonZeroUsize,
    selector: SelectorWord,
    prophecy: ProphecyBits,
    _marker: PhantomData<fn() -> T>,
}

impl<T, const LOW_BITS: usize> Copy for KeyedArenaHandle<T, LOW_BITS> {}

impl<T, const LOW_BITS: usize> Clone for KeyedArenaHandle<T, LOW_BITS> {
    fn clone(&self) -> Self { *self }
}

impl<T, const LOW_BITS: usize> KeyedArenaHandle<T, LOW_BITS> {
    /// Slot index carried by this handle.
    pub const fn index(self) -> u32 { self.index }

    /// Generation carried by this handle.
    pub const fn generation(self) -> u32 { self.generation }

    /// Masked nonzero verification word. This is not a raw pointer authority.
    pub const fn check_word(self) -> NonZeroUsize { self.check }

    /// Trait/implementation/function selector attached to this handle.
    pub const fn selector(self) -> SelectorWord { self.selector }

    /// Prophecy/hint bits attached to this handle.
    pub const fn prophecy(self) -> ProphecyBits { self.prophecy }

    /// Low state tag visible in the check word.
    pub const fn tag(self) -> usize { self.check.get() & LowTagLayout::<LOW_BITS>::LOW_MASK }

    /// Build a forged/test handle by changing only the check word.
    ///
    /// This is intentionally safe because validation must reject forged handles.
    pub const fn with_check_word(self, check: NonZeroUsize) -> Self { Self { check, ..self } }

    /// Build a forged/test handle by changing only the generation.
    pub const fn with_generation(self, generation: u32) -> Self { Self { generation, ..self } }

    /// Build a forged/test handle by changing only the selector.
    pub const fn with_selector(self, selector: SelectorWord) -> Self { Self { selector, ..self } }

    /// Build a forged/test handle by changing only the prophecy bits.
    pub const fn with_prophecy(self, prophecy: ProphecyBits) -> Self { Self { prophecy, ..self } }

    /// Rebuild a handle from opaque validation parts.
    ///
    /// This constructor is intentionally safe because a handle produced from
    /// serialized/user-controlled data still has no authority by itself. The
    /// owning arena must validate index, generation, check word, selector,
    /// prophecy bits, and revealed address before access is granted.
    pub const fn from_validation_parts(
        index: u32,
        generation: u32,
        check: NonZeroUsize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> Self {
        Self { index, generation, check, selector, prophecy, _marker: PhantomData }
    }
}

#[derive(Debug)]
struct ArenaEntry<T, const LOW_BITS: usize> {
    value: Box<T>,
    ptr: NonNull<T>,
    generation: u32,
    check: NonZeroUsize,
    selector: SelectorWord,
    prophecy: ProphecyBits,
}

impl<T, const LOW_BITS: usize> ArenaEntry<T, LOW_BITS> {
    fn handle(&self, index: usize) -> KeyedArenaHandle<T, LOW_BITS> {
        KeyedArenaHandle {
            index: index as u32,
            generation: self.generation,
            check: self.check,
            selector: self.selector,
            prophecy: self.prophecy,
            _marker: PhantomData,
        }
    }
}

/// Error returned when a keyed arena handle does not validate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyedArenaError {
    /// The handle index is outside the arena.
    #[error("arena handle index {index} is outside the arena")]
    BadIndex { index: u32 },

    /// The slot has been removed.
    #[error("arena slot {index} is vacant")]
    Vacant { index: u32 },

    /// The handle generation does not match the live slot generation.
    #[error("stale arena handle at index {index}: handle generation {handle_generation}, live generation {live_generation}")]
    StaleGeneration { index: u32, handle_generation: u32, live_generation: u32 },

    /// The check word does not match the live masked pointer/tag word.
    #[error("arena capability/check word mismatch at index {index}")]
    CheckMismatch { index: u32 },

    /// The handle selector does not match the live selector.
    #[error("arena selector mismatch at index {index}")]
    SelectorMismatch { index: u32 },

    /// The handle prophecy bits do not match the live prophecy bits.
    #[error("arena prophecy mismatch at index {index}")]
    ProphecyMismatch { index: u32 },

    /// The masked word did not reveal to the live pointer address.
    #[error("arena keyed reveal did not match the live pointer at index {index}")]
    RevealMismatch { index: u32 },

    /// The arena cannot represent or allocate another flat slot index.
    #[error("arena capacity exceeded")]
    CapacityExceeded,

    /// Underlying tag/key validation failed.
    #[error(transparent)]
    Tag(#[from] petr_core::TagError),
}

/// Result type for keyed arena operations.
pub type KeyedArenaResult<T> = Result<T, KeyedArenaError>;

/// A provenance-preserving, keyed, generation-checked arena.
///
/// This is the hardened general-purpose form behind the workspace's collection
/// ideas. Values are owned by stable `Box<T>` allocations. The arena keeps the
/// real `NonNull<T>` internally for provenance-preserving access and stores a
/// masked check word only as a validation/capability word.
///
/// Each handle carries:
///
/// - slot index
/// - generation
/// - keyed check word
/// - low tag
/// - 4/4/8 selector word
/// - prophecy bits
///
/// A handle cannot access anything unless the owning arena validates all of that
/// metadata against the live slot.
#[derive(Debug)]
pub struct KeyedArena<T, const LOW_BITS: usize> {
    key: Key<LOW_BITS>,
    entries: Vec<Option<ArenaEntry<T, LOW_BITS>>>,
    free: Vec<usize>,
    live: usize,
    next_generation: u32,
}

impl<T, const LOW_BITS: usize> Default for KeyedArena<T, LOW_BITS> {
    fn default() -> Self { Self::new_randomized() }
}

impl<T, const LOW_BITS: usize> KeyedArena<T, LOW_BITS> {
    /// Create an arena with an OS-random key when available.
    pub fn new_randomized() -> Self { Self::with_key(Key::random()) }

    /// Create an arena with a deterministic key, mostly useful for tests.
    pub fn with_key(key: Key<LOW_BITS>) -> Self {
        Self { key, entries: Vec::new(), free: Vec::new(), live: 0, next_generation: 1 }
    }

    /// Return the arena key value.
    pub const fn key(&self) -> Key<LOW_BITS> { self.key }

    /// Number of live entries.
    pub const fn len(&self) -> usize { self.live }

    /// Return true when the arena has no live entries.
    pub const fn is_empty(&self) -> bool { self.live == 0 }

    /// Number of allocated slots, including vacant reusable slots.
    pub fn slot_capacity(&self) -> usize { self.entries.len() }

    /// Number of vacant reusable slots.
    pub fn free_len(&self) -> usize { self.free.len() }

    /// Reserve backing slot capacity.
    pub fn reserve_slots(&mut self, additional: usize) { self.entries.reserve(additional); }

    /// Insert a value and return an opaque arena handle.
    pub fn insert(
        &mut self,
        value: T,
        tag: usize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> TagResult<KeyedArenaHandle<T, LOW_BITS>> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        let mut value = Box::new(value);
        let ptr = NonNull::from(value.as_mut());
        let check = self.key.mask_addr(ptr.as_ptr() as usize, tag)?;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);

        let entry = ArenaEntry { value, ptr, generation, check, selector, prophecy };
        let index = if let Some(index) = self.free.pop() {
            self.entries[index] = Some(entry);
            index
        } else {
            self.entries.push(Some(entry));
            self.entries.len() - 1
        };
        self.live += 1;
        Ok(self.entries[index].as_ref().expect("inserted arena entry exists").handle(index))
    }

    /// Validate a handle and return the live slot index.
    pub fn validate_handle(&self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<usize> {
        let index = handle.index as usize;
        let Some(slot) = self.entries.get(index) else {
            return Err(KeyedArenaError::BadIndex { index: handle.index });
        };
        let Some(entry) = slot.as_ref() else {
            return Err(KeyedArenaError::Vacant { index: handle.index });
        };
        if entry.generation != handle.generation {
            return Err(KeyedArenaError::StaleGeneration {
                index: handle.index,
                handle_generation: handle.generation,
                live_generation: entry.generation,
            });
        }
        if entry.check != handle.check {
            return Err(KeyedArenaError::CheckMismatch { index: handle.index });
        }
        if entry.selector != handle.selector {
            return Err(KeyedArenaError::SelectorMismatch { index: handle.index });
        }
        if entry.prophecy != handle.prophecy {
            return Err(KeyedArenaError::ProphecyMismatch { index: handle.index });
        }
        let revealed = self.key.reveal_addr(handle.check)?;
        if revealed != entry.ptr.as_ptr() as usize {
            return Err(KeyedArenaError::RevealMismatch { index: handle.index });
        }
        Ok(index)
    }

    /// Return true if the handle currently resolves to a live entry in this arena.
    pub fn contains(&self, handle: KeyedArenaHandle<T, LOW_BITS>) -> bool {
        self.validate_handle(handle).is_ok()
    }

    /// Return a shared reference through a validated handle.
    pub fn get(&self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<&T> {
        let index = self.validate_handle(handle)?;
        Ok(self.entries[index].as_ref().expect("validated live arena entry").value.as_ref())
    }

    /// Return a mutable reference through a validated handle.
    pub fn get_mut(&mut self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<&mut T> {
        let index = self.validate_handle(handle)?;
        Ok(self.entries[index].as_mut().expect("validated live arena entry").value.as_mut())
    }

    /// Return the current handle for an index if it is live.
    pub fn handle_at(&self, index: usize) -> Option<KeyedArenaHandle<T, LOW_BITS>> {
        self.entries.get(index)?.as_ref().map(|entry| entry.handle(index))
    }

    /// Return all current live handles in slot order.
    pub fn live_handles(&self) -> Vec<KeyedArenaHandle<T, LOW_BITS>> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|entry| entry.handle(index)))
            .collect()
    }

    /// Return the selector for a validated handle.
    pub fn selector(&self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<SelectorWord> {
        let index = self.validate_handle(handle)?;
        Ok(self.entries[index].as_ref().expect("validated live arena entry").selector)
    }

    /// Return the prophecy bits for a validated handle.
    pub fn prophecy(&self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<ProphecyBits> {
        let index = self.validate_handle(handle)?;
        Ok(self.entries[index].as_ref().expect("validated live arena entry").prophecy)
    }

    /// Set the low tag and refresh the check word, returning the updated handle.
    ///
    /// The old handle becomes invalid because its check word still contains the
    /// old tag.
    pub fn set_tag(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        tag: usize,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        let index = self.validate_handle(handle)?;
        let entry = self.entries[index].as_mut().expect("validated live arena entry");
        entry.check = self.key.mask_addr(entry.ptr.as_ptr() as usize, tag)?;
        Ok(entry.handle(index))
    }

    /// Set the selector and return the updated handle.
    ///
    /// The old handle becomes invalid because selectors are part of handle
    /// validation. This prevents accidentally dispatching with stale operation
    /// metadata.
    pub fn set_selector(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        selector: SelectorWord,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        let index = self.validate_handle(handle)?;
        let entry = self.entries[index].as_mut().expect("validated live arena entry");
        entry.selector = selector;
        Ok(entry.handle(index))
    }

    /// Set prophecy bits and return the updated handle.
    ///
    /// The old handle becomes invalid because prophecy bits are part of handle
    /// validation.
    pub fn set_prophecy(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        prophecy: ProphecyBits,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        let index = self.validate_handle(handle)?;
        let entry = self.entries[index].as_mut().expect("validated live arena entry");
        entry.prophecy = prophecy;
        Ok(entry.handle(index))
    }

    /// Refresh the generation for a live slot and return the updated handle.
    ///
    /// This is useful when a subsystem wants to explicitly invalidate borrowed or
    /// cached handles without moving or removing the value.
    pub fn refresh_generation(
        &mut self,
        handle: KeyedArenaHandle<T, LOW_BITS>,
    ) -> KeyedArenaResult<KeyedArenaHandle<T, LOW_BITS>> {
        let index = self.validate_handle(handle)?;
        let entry = self.entries[index].as_mut().expect("validated live arena entry");
        entry.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        Ok(entry.handle(index))
    }

    /// Remove a value. The old handle becomes invalid immediately.
    pub fn remove(&mut self, handle: KeyedArenaHandle<T, LOW_BITS>) -> KeyedArenaResult<T> {
        let index = self.validate_handle(handle)?;
        let entry = self.entries[index].take().expect("validated live arena entry");
        self.free.push(index);
        self.live -= 1;
        Ok(*entry.value)
    }

    /// Dispatch a validated handle's selector.
    pub fn dispatch_handle(
        &self,
        table: &OperationTable,
        handle: KeyedArenaHandle<T, LOW_BITS>,
        seed_frame: OpFrame,
    ) -> KeyedArenaResult<DispatchResult<OpFrame>> {
        let index = self.validate_handle(handle)?;
        let entry = self.entries[index].as_ref().expect("validated live arena entry");
        if entry.prophecy.skip_hint {
            Ok(Ok(seed_frame))
        } else {
            Ok(table.execute_word(entry.selector, seed_frame))
        }
    }

    /// Dispatch all live entries in slot order.
    pub fn dispatch_all(&self, table: &OperationTable, seed_frame: OpFrame) -> KeyedArenaResult<Vec<DispatchResult<OpFrame>>> {
        let mut out = Vec::with_capacity(self.live);
        for (index, entry) in self.entries.iter().enumerate() {
            let Some(entry) = entry.as_ref() else { continue; };
            let handle = entry.handle(index);
            self.validate_handle(handle)?;
            if entry.prophecy.skip_hint {
                out.push(Ok(seed_frame.clone()));
            } else {
                out.push(table.execute_word(entry.selector, seed_frame.clone()));
            }
        }
        Ok(out)
    }

    /// Remove all live entries and reset reusable slot state.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.free.clear();
        self.live = 0;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
    }
}
