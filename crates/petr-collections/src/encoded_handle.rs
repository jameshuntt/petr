use core::num::NonZeroUsize;

use petr_core::{ProphecyBits, SelectorWord};

use crate::{KeyedArenaHandle, PagedKeyedArenaHandle};

/// Serialized/session-local representation of a [`KeyedArenaHandle`].
///
/// This type intentionally contains only validation metadata. It does **not**
/// contain a raw pointer address and must not be treated as a dereferenceable
/// capability. It becomes meaningful only when converted back into a typed
/// handle and validated by the same owning arena/key/session that issued it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodedArenaHandle {
    /// Slot index in the issuing arena.
    pub index: u32,
    /// Slot generation in the issuing arena.
    pub generation: u32,
    /// Nonzero keyed check word. This is not a pointer authority.
    pub check: usize,
    /// Raw 16-bit selector word.
    pub selector: u16,
    /// Packed prophecy/hint byte.
    pub prophecy: u8,
    /// Low tag copied out for logs/debugging. Validation uses `check`.
    pub tag: u16,
}

impl EncodedArenaHandle {
    /// Encode a live arena handle into primitive metadata.
    pub fn from_handle<T, const LOW_BITS: usize>(handle: KeyedArenaHandle<T, LOW_BITS>) -> Self {
        Self {
            index: handle.index(),
            generation: handle.generation(),
            check: handle.check_word().get(),
            selector: handle.selector().raw(),
            prophecy: handle.prophecy().pack(),
            tag: handle.tag() as u16,
        }
    }

    /// Convert encoded metadata back into a typed validation handle.
    ///
    /// This does not validate the handle. It only rebuilds the typed envelope so
    /// the owning arena can perform validation.
    pub fn to_handle<T, const LOW_BITS: usize>(self) -> Option<KeyedArenaHandle<T, LOW_BITS>> {
        let check = NonZeroUsize::new(self.check)?;
        Some(KeyedArenaHandle::from_validation_parts(
            self.index,
            self.generation,
            check,
            SelectorWord::new(
                ((self.selector >> 12) & 0x0F) as u8,
                ((self.selector >> 8) & 0x0F) as u8,
                (self.selector & 0xFF) as u8,
            ),
            ProphecyBits::unpack(self.prophecy),
        ))
    }
}

impl<T, const LOW_BITS: usize> From<KeyedArenaHandle<T, LOW_BITS>> for EncodedArenaHandle {
    fn from(handle: KeyedArenaHandle<T, LOW_BITS>) -> Self {
        Self::from_handle(handle)
    }
}

impl EncodedArenaHandle {
    /// Encode a live paged-arena handle into primitive metadata.
    pub fn from_paged_handle<T, const LOW_BITS: usize, const PAGE_SIZE: usize>(
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> Self {
        Self {
            index: handle.index(),
            generation: handle.generation(),
            check: handle.check_word().get(),
            selector: handle.selector().raw(),
            prophecy: handle.prophecy().pack(),
            tag: handle.tag() as u16,
        }
    }

    /// Convert encoded metadata back into a typed paged-arena validation handle.
    ///
    /// This only rebuilds the envelope. The owning paged arena still has to
    /// validate every field and the revealed live address.
    pub fn to_paged_handle<T, const LOW_BITS: usize, const PAGE_SIZE: usize>(
        self,
    ) -> Option<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        let check = NonZeroUsize::new(self.check)?;
        Some(PagedKeyedArenaHandle::from_validation_parts(
            self.index,
            self.generation,
            check,
            SelectorWord::new(
                ((self.selector >> 12) & 0x0F) as u8,
                ((self.selector >> 8) & 0x0F) as u8,
                (self.selector & 0xFF) as u8,
            ),
            ProphecyBits::unpack(self.prophecy),
        ))
    }
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize>
    From<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> for EncodedArenaHandle
{
    fn from(handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>) -> Self {
        Self::from_paged_handle(handle)
    }
}
