use crate::{PortHint, SelectorWord};

/// Small summary bits that predict how a value should be handled before deref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProphecyBits {
    /// Value is known/expected to be zero-like.
    pub maybe_zero: bool,
    /// Value is known/expected to be empty-like.
    pub maybe_empty: bool,
    /// Value should be treated as hot.
    pub hot: bool,
    /// Value should be skipped if the operation supports skip-fast behavior.
    pub skip_hint: bool,
    /// Software lane hint.
    pub port_hint: PortHint,
}

impl ProphecyBits {
    /// Pack the prophecy into one byte.
    pub const fn pack(self) -> u8 {
        (self.maybe_zero as u8)
            | ((self.maybe_empty as u8) << 1)
            | ((self.hot as u8) << 2)
            | ((self.skip_hint as u8) << 3)
            | ((match self.port_hint {
                PortHint::Integer => 0,
                PortHint::Vector => 1,
                PortHint::Load => 2,
                PortHint::Store => 3,
                PortHint::Dispatch => 4,
            }) << 4)
    }

    /// Unpack from one byte.
    pub const fn unpack(byte: u8) -> Self {
        let lane = (byte >> 4) & 0x07;
        Self {
            maybe_zero: byte & 1 != 0,
            maybe_empty: byte & 2 != 0,
            hot: byte & 4 != 0,
            skip_hint: byte & 8 != 0,
            port_hint: match lane {
                1 => PortHint::Vector,
                2 => PortHint::Load,
                3 => PortHint::Store,
                4 => PortHint::Dispatch,
                _ => PortHint::Integer,
            },
        }
    }
}

/// Selector plus prophecy metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProphecyHeader {
    /// Trait/impl/function selector.
    pub selector: SelectorWord,
    /// Prediction bits.
    pub prophecy: ProphecyBits,
}
