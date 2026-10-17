use core::ptr::NonNull;

use petr_core::TaggedNonNull;
use thiserror::Error;

pub const MTE_GRANULE_SIZE: usize = 16;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum MteError {
    #[error("MTE tag must fit in four bits")]
    InvalidTag,
    #[error("MTE model handle points at another allocation")]
    WrongAllocation,
    #[error("MTE model pointer tag does not match the allocation tag")]
    TagMismatch,
    #[error("MTE model handle belongs to an expired allocation generation")]
    ExpiredGeneration,
    #[error("native MTE was not compiled for Linux/AArch64")]
    NativeUnavailable,
    #[error("native MTE range must be 16-byte aligned and granule sized")]
    MisalignedRange,
    #[error("failed to enable native MTE enforcement: {0}")]
    Enforcement(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MteTag(u8);

impl MteTag {
    pub fn new(value: u8) -> Result<Self, MteError> {
        if value <= 0x0f {
            Ok(Self(value))
        } else {
            Err(MteError::InvalidTag)
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Portable model of one MTE-tagged allocation.
///
/// Retagging advances a generation as well as changing the four-bit tag so a
/// stale model handle cannot become valid again after tag wraparound.
#[derive(Debug)]
pub struct MteAllocation<T, const LOW_BITS: usize> {
    tagged: TaggedNonNull<T, LOW_BITS>,
    allocation_tag: MteTag,
    generation: u64,
}

#[derive(Debug)]
pub struct MteHandle<T, const LOW_BITS: usize> {
    tagged: TaggedNonNull<T, LOW_BITS>,
    pointer_tag: MteTag,
    generation: u64,
}

impl<T, const LOW_BITS: usize> Copy for MteHandle<T, LOW_BITS> {}
impl<T, const LOW_BITS: usize> Clone for MteHandle<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize> MteHandle<T, LOW_BITS> {
    pub fn from_parts(
        tagged: TaggedNonNull<T, LOW_BITS>,
        pointer_tag: MteTag,
        generation: u64,
    ) -> Self {
        Self {
            tagged,
            pointer_tag,
            generation,
        }
    }

    pub fn tagged(self) -> TaggedNonNull<T, LOW_BITS> {
        self.tagged
    }
    pub const fn pointer_tag(self) -> MteTag {
        self.pointer_tag
    }
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Change only the modeled pointer tag. Useful for negative tests and for
    /// representing a pointer whose address is correct but whose MTE tag is not.
    pub fn with_pointer_tag(self, pointer_tag: MteTag) -> Self {
        Self {
            pointer_tag,
            ..self
        }
    }
}

impl<T, const LOW_BITS: usize> MteAllocation<T, LOW_BITS> {
    pub fn new(tagged: TaggedNonNull<T, LOW_BITS>, allocation_tag: MteTag) -> Self {
        Self {
            tagged,
            allocation_tag,
            generation: 1,
        }
    }

    pub fn issue(&self) -> MteHandle<T, LOW_BITS> {
        MteHandle {
            tagged: self.tagged,
            pointer_tag: self.allocation_tag,
            generation: self.generation,
        }
    }

    pub fn validate(&self, handle: MteHandle<T, LOW_BITS>) -> Result<NonNull<T>, MteError> {
        if handle.tagged.clean_addr() != self.tagged.clean_addr()
            || handle.tagged.tag() != self.tagged.tag()
        {
            return Err(MteError::WrongAllocation);
        }
        if handle.generation != self.generation {
            return Err(MteError::ExpiredGeneration);
        }
        if handle.pointer_tag != self.allocation_tag {
            return Err(MteError::TagMismatch);
        }
        Ok(self.tagged.clean())
    }

    pub fn retag(&mut self, new_tag: MteTag) -> Result<(), MteError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(MteError::ExpiredGeneration)?;
        self.allocation_tag = new_tag;
        Ok(())
    }

    pub const fn allocation_tag(&self) -> MteTag {
        self.allocation_tag
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

/// Whether the native MTE instruction helpers were compiled in.
///
/// This is not a runtime CPU/kernel capability probe.
pub const fn native_mte_compiled() -> bool {
    cfg!(all(
        target_os = "linux",
        target_arch = "aarch64",
        feature = "aarch64-mte"
    ))
}

/// Insert a random architectural MTE tag into a pointer.
///
/// # Safety
/// The caller must use this only on a Linux/AArch64 MTE-enabled mapping and
/// must obey the platform's tagged-address and Rust aliasing requirements.
pub unsafe fn insert_random_tag<T>(ptr: *mut T, exclude_mask: u16) -> Result<*mut T, MteError> {
    if !native_mte_compiled() {
        return Err(MteError::NativeUnavailable);
    }
    unsafe { native_insert_random_tag(ptr, exclude_mask) }
}

/// Store the pointer's allocation tag into one MTE granule.
///
/// # Safety
/// `tagged_ptr` must designate a writable MTE-enabled 16-byte granule and the
/// process must have enabled tagged-address/MTE handling.
pub unsafe fn set_mem_tag(tagged_ptr: *mut u8) -> Result<(), MteError> {
    if !native_mte_compiled() {
        return Err(MteError::NativeUnavailable);
    }
    if (tagged_ptr as usize) % MTE_GRANULE_SIZE != 0 {
        return Err(MteError::MisalignedRange);
    }
    unsafe {
        native_set_mem_tag(tagged_ptr);
    }
    Ok(())
}

/// Tag a granule-aligned MTE region one granule at a time.
///
/// # Safety
/// The entire range must be a writable MTE-enabled mapping owned by the caller.
pub unsafe fn tag_range(tagged_ptr: *mut u8, size: usize) -> Result<(), MteError> {
    validate_native_range(tagged_ptr, size)?;
    if !native_mte_compiled() {
        return Err(MteError::NativeUnavailable);
    }
    unsafe {
        native_tag_range(tagged_ptr, size);
    }
    Ok(())
}

/// Tag a granule-aligned MTE region using paired stores when possible.
///
/// # Safety
/// The entire range must be a writable MTE-enabled mapping owned by the caller.
pub unsafe fn tag_range_fast(tagged_ptr: *mut u8, size: usize) -> Result<(), MteError> {
    validate_native_range(tagged_ptr, size)?;
    if !native_mte_compiled() {
        return Err(MteError::NativeUnavailable);
    }
    unsafe {
        native_tag_range_fast(tagged_ptr, size);
    }
    Ok(())
}

fn validate_native_range(ptr: *mut u8, size: usize) -> Result<(), MteError> {
    if (ptr as usize) % MTE_GRANULE_SIZE != 0 || size % MTE_GRANULE_SIZE != 0 {
        Err(MteError::MisalignedRange)
    } else {
        Ok(())
    }
}

/// Enable synchronous Linux MTE checking for the current process.
///
/// This does not create `PROT_MTE` mappings and does not prove runtime CPU
/// support. It is only the process-control step of a native MTE experiment.
///
/// # Safety
/// Enabling tagged-address checking changes process-wide pointer fault behavior.
pub unsafe fn enable_mte_enforcement_sync() -> Result<(), MteError> {
    if !native_mte_compiled() {
        return Err(MteError::NativeUnavailable);
    }
    unsafe { native_enable_mte_enforcement_sync() }
}

#[cfg(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte"))]
unsafe fn native_insert_random_tag<T>(ptr: *mut T, exclude_mask: u16) -> Result<*mut T, MteError> {
    let tagged_ptr: usize;
    // SAFETY: caller satisfies the native MTE preconditions documented above.
    unsafe {
        core::arch::asm!(
            "irg {out}, {in_ptr}, {mask}",
            out = out(reg) tagged_ptr,
            in_ptr = in(reg) ptr as usize,
            mask = in(reg) exclude_mask as usize,
            options(nostack),
        );
    }
    Ok(tagged_ptr as *mut T)
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte")))]
unsafe fn native_insert_random_tag<T>(
    _ptr: *mut T,
    _exclude_mask: u16,
) -> Result<*mut T, MteError> {
    Err(MteError::NativeUnavailable)
}

#[cfg(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte"))]
unsafe fn native_set_mem_tag(tagged_ptr: *mut u8) {
    // SAFETY: caller guarantees an aligned writable MTE-enabled granule.
    unsafe {
        core::arch::asm!("stg {ptr}, [{ptr}]", ptr = in(reg) tagged_ptr as usize, options(nostack));
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte")))]
unsafe fn native_set_mem_tag(_tagged_ptr: *mut u8) {}

#[cfg(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte"))]
unsafe fn native_tag_range(mut ptr: *mut u8, size: usize) {
    let end = unsafe { ptr.add(size) };
    while ptr < end {
        // SAFETY: caller guaranteed every granule in the range.
        unsafe {
            core::arch::asm!("stg {p}, [{p}]", p = in(reg) ptr as usize, options(nostack));
        }
        ptr = unsafe { ptr.add(MTE_GRANULE_SIZE) };
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte")))]
unsafe fn native_tag_range(_ptr: *mut u8, _size: usize) {}

#[cfg(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte"))]
unsafe fn native_tag_range_fast(mut ptr: *mut u8, size: usize) {
    let mut remaining = size;
    while remaining >= 32 {
        // SAFETY: caller guaranteed two writable granules beginning at `ptr`.
        unsafe {
            core::arch::asm!("st2g {p}, [{p}]", p = in(reg) ptr as usize, options(nostack));
        }
        ptr = unsafe { ptr.add(32) };
        remaining -= 32;
    }
    if remaining == MTE_GRANULE_SIZE {
        // SAFETY: the validated size leaves exactly one final granule.
        unsafe {
            core::arch::asm!("stg {p}, [{p}]", p = in(reg) ptr as usize, options(nostack));
        }
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte")))]
unsafe fn native_tag_range_fast(_ptr: *mut u8, _size: usize) {}

#[cfg(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte"))]
unsafe fn native_enable_mte_enforcement_sync() -> Result<(), MteError> {
    const PR_SET_TAGGED_ADDR_CTRL: i32 = 55;
    const PR_TAGGED_ADDR_ENABLE: usize = 1 << 0;
    const PR_MTE_TCF_SYNC: usize = 1 << 1;
    const PR_MTE_TAG_SHIFT: usize = 3;
    const PR_MTE_TAG_MASK: usize = 0xfffe << PR_MTE_TAG_SHIFT;

    unsafe extern "C" {
        fn prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i32;
    }

    let ctrl = PR_TAGGED_ADDR_ENABLE | PR_MTE_TCF_SYNC | PR_MTE_TAG_MASK;
    let rc = unsafe { prctl(PR_SET_TAGGED_ADDR_CTRL, ctrl, 0, 0, 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(MteError::Enforcement(rc))
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64", feature = "aarch64-mte")))]
unsafe fn native_enable_mte_enforcement_sync() -> Result<(), MteError> {
    Err(MteError::NativeUnavailable)
}
