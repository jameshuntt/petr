/// A 16-bit selector split into trait / implementation / function fields.
///
/// Layout: `tttt iiii ffffffff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectorWord(u16);

impl SelectorWord {
    /// Create a selector from 4-bit trait, 4-bit impl, and 8-bit function ids.
    pub const fn new(trait_id: u8, impl_id: u8, function_id: u8) -> Self {
        Self(
            (((trait_id & 0x0F) as u16) << 12)
                | (((impl_id & 0x0F) as u16) << 8)
                | function_id as u16,
        )
    }

    /// Raw 16-bit selector.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// High 4-bit trait/interface id.
    pub const fn trait_id(self) -> u8 {
        ((self.0 >> 12) & 0x0F) as u8
    }

    /// Middle 4-bit implementation/struct id.
    pub const fn impl_id(self) -> u8 {
        ((self.0 >> 8) & 0x0F) as u8
    }

    /// Low 8-bit function selector.
    pub const fn function_id(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Use only the function selector as a 256-entry dispatch table index.
    pub const fn table_index(self) -> usize {
        self.function_id() as usize
    }
}

/// A lightweight hint about where a software scheduler should route work.
///
/// This is not literal CPU execution-port control; it is a software scheduling hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortHint {
    /// Integer-ish scalar work.
    Integer,
    /// Vector/SIMD-ish work.
    Vector,
    /// Load-heavy work.
    Load,
    /// Store-heavy work.
    Store,
    /// Branch/dispatch-heavy work.
    Dispatch,
}
