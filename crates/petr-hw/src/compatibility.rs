use petr_core::{SplitTaggedNonNull, TaggedNonNull};

/// A decomposed PETR split-tagged pointer suitable for crossing a hardware
/// tagging boundary without reusing its logical top byte as hardware metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitHardwareView {
    pub clean_addr: usize,
    pub software_top: u8,
    pub low_tag: usize,
}

/// Inspect a split-tagged pointer without manufacturing a hardware-tagged raw
/// pointer.
pub fn split_hardware_view<T, const LOW_BITS: usize>(
    ptr: SplitTaggedNonNull<T, LOW_BITS>,
) -> SplitHardwareView {
    SplitHardwareView {
        clean_addr: ptr.clean_addr(),
        software_top: ptr.top(),
        low_tag: ptr.low(),
    }
}

/// Convert a split-tagged PETR pointer into the low-tag-only view used by PAC
/// and MTE adapters while preserving the original `NonNull<T>` provenance.
///
/// The software top byte is intentionally discarded from the hardware-facing
/// view. It remains available from the original `SplitTaggedNonNull` handle.
pub fn hardware_base<T, const LOW_BITS: usize>(
    ptr: SplitTaggedNonNull<T, LOW_BITS>,
) -> TaggedNonNull<T, LOW_BITS> {
    // `ptr.low()` already satisfied the same LowTagLayout when the split value
    // was constructed, and `ptr.clean()` preserves the original provenance.
    TaggedNonNull::new(ptr.clean(), ptr.low()).expect("validated split-tagged pointer")
}
