use thiserror::Error;

/// Result type used by the tagged pointer workspace.
pub type TagResult<T> = Result<T, TagError>;

/// Errors returned when a pointer word, tag, layout, or keyed reveal is invalid.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TagError {
    /// Too many low bits were requested for the crate's conservative policy.
    #[error("requested LOW_BITS={bits}, but the conservative maximum is {max}")]
    LowBitsTooLarge { bits: usize, max: usize },

    /// A tag value does not fit inside the low-bit field.
    #[error("tag {tag:#x} does not fit in low mask {mask:#x}")]
    TagTooLarge { tag: usize, mask: usize },

    /// A cleaned pointer address was zero/null and cannot be represented as `NonNull`.
    #[error("clean pointer address cannot be zero/null")]
    NullAddress,

    /// A pointer address is not aligned enough for the requested low-bit count.
    #[error("address {addr:#x} is not aligned to {align} bytes")]
    Misaligned { addr: usize, align: usize },

    /// A keyed pointer was revealed with the wrong key or produced a bad address.
    #[error("keyed pointer reveal failed validation")]
    KeyRevealFailed,

    /// A residual class had a zero modulus.
    #[error("residual modulus cannot be zero")]
    ZeroModulus,

    /// A residual class residue was outside the modulus.
    #[error("residue {residue} must be lower than modulus {modulus}")]
    BadResidue { modulus: usize, residue: usize },
}
