/// A 64-bit word that can hold either a small immediate or an external handle.
///
/// This is not a dereferenceable pointer. It is useful for VM/operator examples
/// where some values can be carried inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueWord(u64);

impl ValueWord {
    const IMMEDIATE_BIT: u64 = 1;

    /// Encode a signed 31-bit immediate.
    pub const fn immediate_i31(value: i32) -> Self {
        Self(((value as i64 as u64) << 1) | Self::IMMEDIATE_BIT)
    }

    /// Encode an external handle word. The low immediate bit is cleared.
    pub const fn handle(raw: u64) -> Self {
        Self(raw & !Self::IMMEDIATE_BIT)
    }

    /// Return true when this word is an immediate value.
    pub const fn is_immediate(self) -> bool { self.0 & Self::IMMEDIATE_BIT != 0 }

    /// Return the raw word.
    pub const fn raw(self) -> u64 { self.0 }

    /// Decode the immediate as a small signed value.
    pub const fn as_i31(self) -> Option<i32> {
        if self.is_immediate() {
            Some((self.0 as i64 >> 1) as i32)
        } else {
            None
        }
    }
}
