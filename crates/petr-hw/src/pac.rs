use core::ptr::NonNull;

use petr_core::TaggedNonNull;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PacError {
    #[error("native PAC was not compiled for this target")]
    NativeUnavailable,
    #[error("pointer authentication failed")]
    AuthenticationFailed,
}

/// PAC modifier/context value. PETR metadata is folded into the modifier rather
/// than written into PAC-owned pointer bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PacModifier(pub u64);

impl PacModifier {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Deterministically bind domain/generation/phase style metadata into one
    /// modifier. This is a mixer, not a cryptographic KDF.
    pub fn from_components(domain: u64, generation: u64, phase: u64) -> Self {
        let x = mix64(domain ^ generation.rotate_left(21) ^ phase.rotate_left(43));
        Self(x)
    }

    fn bind_low_tag(self, low_tag: usize) -> usize {
        mix64(self.0 ^ (low_tag as u64).rotate_left(17)) as usize
    }
}

/// Deterministic key for the portable PAC policy model.
///
/// This is intentionally not a cryptographic substitute for architectural PAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoftwarePacKey(u64);

impl SoftwarePacKey {
    pub fn from_seed(seed: u64) -> Self {
        Self(mix64(seed ^ 0x5041_432d_5045_5452))
    }
}

/// Portable model token. The actual pointer stays provenance-preserving and the
/// model signature is an independent check value.
#[derive(Debug)]
pub struct SoftwarePacToken<T, const LOW_BITS: usize> {
    tagged: TaggedNonNull<T, LOW_BITS>,
    signature: u64,
}

impl<T, const LOW_BITS: usize> Copy for SoftwarePacToken<T, LOW_BITS> {}
impl<T, const LOW_BITS: usize> Clone for SoftwarePacToken<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize> SoftwarePacToken<T, LOW_BITS> {
    pub fn tagged(self) -> TaggedNonNull<T, LOW_BITS> {
        self.tagged
    }
    pub const fn signature(self) -> u64 {
        self.signature
    }
}

pub struct SoftwarePac;

impl SoftwarePac {
    pub fn sign<T, const LOW_BITS: usize>(
        tagged: TaggedNonNull<T, LOW_BITS>,
        modifier: PacModifier,
        key: SoftwarePacKey,
    ) -> SoftwarePacToken<T, LOW_BITS> {
        let signature = software_signature(tagged.clean_addr(), tagged.tag(), modifier, key);
        SoftwarePacToken { tagged, signature }
    }

    pub fn authenticate<T, const LOW_BITS: usize>(
        token: SoftwarePacToken<T, LOW_BITS>,
        modifier: PacModifier,
        key: SoftwarePacKey,
    ) -> Result<NonNull<T>, PacError> {
        let expected =
            software_signature(token.tagged.clean_addr(), token.tagged.tag(), modifier, key);
        if expected == token.signature {
            Ok(token.tagged.clean())
        } else {
            Err(PacError::AuthenticationFailed)
        }
    }
}

fn software_signature(
    addr: usize,
    low_tag: usize,
    modifier: PacModifier,
    key: SoftwarePacKey,
) -> u64 {
    mix64(
        key.0
            ^ (addr as u64).rotate_left(9)
            ^ (low_tag as u64).rotate_left(31)
            ^ modifier.0.rotate_left(47),
    )
}

/// Opaque native PAC token. Even when PAC is enabled, PETR retains the clean
/// pointer for provenance and treats the signed address as a check token rather
/// than a dereferenceable Rust pointer.
#[derive(Debug)]
pub struct NativePacToken<T, const LOW_BITS: usize> {
    tagged: TaggedNonNull<T, LOW_BITS>,
    signed_addr: usize,
}

impl<T, const LOW_BITS: usize> Copy for NativePacToken<T, LOW_BITS> {}
impl<T, const LOW_BITS: usize> Clone for NativePacToken<T, LOW_BITS> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize> NativePacToken<T, LOW_BITS> {
    pub fn sign(
        tagged: TaggedNonNull<T, LOW_BITS>,
        modifier: PacModifier,
    ) -> Result<Self, PacError> {
        if !native_pac_compiled() {
            return Err(PacError::NativeUnavailable);
        }
        let bound = modifier.bind_low_tag(tagged.tag());
        let signed_addr = native_sign_addr(tagged.clean_addr(), bound);
        Ok(Self {
            tagged,
            signed_addr,
        })
    }

    pub fn authenticate(self, modifier: PacModifier) -> Result<NonNull<T>, PacError> {
        if !native_pac_compiled() {
            return Err(PacError::NativeUnavailable);
        }
        let bound = modifier.bind_low_tag(self.tagged.tag());
        let authenticated = native_auth_addr(self.signed_addr, bound);
        if authenticated == self.tagged.clean_addr() {
            Ok(self.tagged.clean())
        } else {
            Err(PacError::AuthenticationFailed)
        }
    }

    pub const fn signed_addr(self) -> usize {
        self.signed_addr
    }
    pub fn tagged(self) -> TaggedNonNull<T, LOW_BITS> {
        self.tagged
    }
}

/// Whether this binary was compiled with native AArch64 PAC support enabled.
pub const fn native_pac_compiled() -> bool {
    cfg!(all(target_arch = "aarch64", feature = "aarch64-pac"))
}

#[cfg(all(target_arch = "aarch64", feature = "aarch64-pac"))]
#[inline(always)]
fn native_sign_addr(value: usize, context: usize) -> usize {
    let mut ptr = value;
    // SAFETY: PACIA transforms an integer address word. The result is retained
    // as an opaque check token and is not dereferenced directly.
    unsafe {
        core::arch::asm!(
            "pacia {ptr}, {ctx}",
            ptr = inout(reg) ptr,
            ctx = in(reg) context,
            options(nostack),
        );
    }
    ptr
}

#[cfg(not(all(target_arch = "aarch64", feature = "aarch64-pac")))]
#[inline(always)]
fn native_sign_addr(value: usize, _context: usize) -> usize {
    value
}

#[cfg(all(target_arch = "aarch64", feature = "aarch64-pac"))]
#[inline(always)]
fn native_auth_addr(value: usize, context: usize) -> usize {
    let mut ptr = value;
    // SAFETY: AUTIA transforms an integer address word. PETR compares the
    // result with its retained clean pointer before returning provenance.
    unsafe {
        core::arch::asm!(
            "autia {ptr}, {ctx}",
            ptr = inout(reg) ptr,
            ctx = in(reg) context,
            options(nostack),
        );
    }
    ptr
}

#[cfg(not(all(target_arch = "aarch64", feature = "aarch64-pac")))]
#[inline(always)]
fn native_auth_addr(value: usize, _context: usize) -> usize {
    value
}

const fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
