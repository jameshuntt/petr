use crate::{TagError, TagResult};

/// Compile-time layout helper for low-bit pointer tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LowTagLayout<const LOW_BITS: usize>;

impl<const LOW_BITS: usize> LowTagLayout<LOW_BITS> {
    /// Conservative maximum low bits allowed by this playground.
    ///
    /// Larger alignments can be valid, but this limit avoids accidentally
    /// making small objects waste huge amounts of memory in examples.
    pub const MAX_LOW_BITS: usize = 12;

    /// Required alignment in bytes.
    pub const ALIGN: usize = 1usize << LOW_BITS;

    /// Mask covering the low tag bits.
    pub const LOW_MASK: usize = if LOW_BITS == 0 { 0 } else { (1usize << LOW_BITS) - 1 };

    /// Mask covering the clean pointer region after low tag bits are stripped.
    pub const CLEAN_MASK: usize = !Self::LOW_MASK;

    /// Number of possible low-tag states.
    pub const TAG_STATES: usize = 1usize << LOW_BITS;

    /// Validate only the requested low-bit layout.
    pub const fn validate_layout() -> TagResult<()> {
        if LOW_BITS > Self::MAX_LOW_BITS {
            Err(TagError::LowBitsTooLarge { bits: LOW_BITS, max: Self::MAX_LOW_BITS })
        } else {
            Ok(())
        }
    }

    /// Validate that a tag fits.
    pub const fn validate_tag(tag: usize) -> TagResult<()> {
        if LOW_BITS > Self::MAX_LOW_BITS {
            return Err(TagError::LowBitsTooLarge { bits: LOW_BITS, max: Self::MAX_LOW_BITS });
        }
        if tag & !Self::LOW_MASK != 0 {
            return Err(TagError::TagTooLarge { tag, mask: Self::LOW_MASK });
        }
        Ok(())
    }

    /// Validate that an address is aligned enough for this tag layout.
    pub const fn validate_addr(addr: usize) -> TagResult<()> {
        if LOW_BITS > Self::MAX_LOW_BITS {
            return Err(TagError::LowBitsTooLarge { bits: LOW_BITS, max: Self::MAX_LOW_BITS });
        }
        if addr & Self::LOW_MASK != 0 {
            return Err(TagError::Misaligned { addr, align: Self::ALIGN });
        }
        Ok(())
    }

    /// Strip low tag bits from a word.
    pub const fn clean_word(word: usize) -> usize {
        word & Self::CLEAN_MASK
    }

    /// Extract only the low tag bits from a word.
    pub const fn low_tag(word: usize) -> usize {
        word & Self::LOW_MASK
    }
}
