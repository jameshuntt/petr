//! CPU placement policy for PETR.
//!
//! Selection is portable and deterministic. Actual OS thread pinning is an
//! optional adapter behind the `core-affinity` feature.

#![deny(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CpuId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSet {
    cpus: Vec<CpuId>,
}
impl CpuSet {
    pub fn new(mut cpus: Vec<CpuId>) -> Result<Self, AffinityError> {
        cpus.sort_unstable();
        cpus.dedup();
        if cpus.is_empty() {
            return Err(AffinityError::EmptyCpuSet);
        }
        Ok(Self { cpus })
    }
    pub fn from_count(count: usize) -> Result<Self, AffinityError> {
        if count == 0 {
            return Err(AffinityError::EmptyCpuSet);
        }
        Self::new((0..count).map(CpuId).collect())
    }
    pub fn system_visible() -> Result<Self, AffinityError> {
        #[cfg(feature = "core-affinity")]
        {
            let ids = core_affinity::get_core_ids().ok_or(AffinityError::DiscoveryUnavailable)?;
            return Self::new(ids.into_iter().map(|id| CpuId(id.id)).collect());
        }
        #[cfg(not(feature = "core-affinity"))]
        {
            let count = std::thread::available_parallelism()
                .map_err(|_| AffinityError::DiscoveryUnavailable)?
                .get();
            Self::from_count(count)
        }
    }
    pub fn contains(&self, cpu: CpuId) -> bool {
        self.cpus.binary_search(&cpu).is_ok()
    }
    pub fn as_slice(&self) -> &[CpuId] {
        &self.cpus
    }
    pub fn len(&self) -> usize {
        self.cpus.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cpus.is_empty()
    }
    pub fn first_free(&self, occupied: &[CpuId]) -> Option<CpuId> {
        self.cpus
            .iter()
            .copied()
            .find(|cpu| !occupied.contains(cpu))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Require(CpuId),
    Prefer(CpuId),
    Any,
}

impl Placement {
    pub fn resolve(
        self,
        allowed: &CpuSet,
        occupied: &[CpuId],
    ) -> Result<Option<CpuId>, AffinityError> {
        match self {
            Self::Require(cpu) => {
                if !allowed.contains(cpu) {
                    return Err(AffinityError::CpuNotAllowed(cpu));
                }
                if occupied.contains(&cpu) {
                    return Err(AffinityError::CpuBusy(cpu));
                }
                Ok(Some(cpu))
            }
            Self::Prefer(cpu) => {
                if allowed.contains(cpu) && !occupied.contains(&cpu) {
                    Ok(Some(cpu))
                } else {
                    Ok(allowed.first_free(occupied))
                }
            }
            Self::Any => Ok(allowed.first_free(occupied)),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AffinityError {
    #[error("CPU set cannot be empty")]
    EmptyCpuSet,
    #[error("CPU discovery is unavailable")]
    DiscoveryUnavailable,
    #[error("CPU {0:?} is outside the allowed set")]
    CpuNotAllowed(CpuId),
    #[error("CPU {0:?} is already occupied")]
    CpuBusy(CpuId),
    #[error("OS thread pinning support is not enabled")]
    PinningUnsupported,
    #[error("failed to pin current thread to CPU {0:?}")]
    PinningFailed(CpuId),
}

pub fn pin_current(cpu: CpuId) -> Result<(), AffinityError> {
    #[cfg(feature = "core-affinity")]
    {
        if core_affinity::set_for_current(core_affinity::CoreId { id: cpu.0 }) {
            Ok(())
        } else {
            Err(AffinityError::PinningFailed(cpu))
        }
    }
    #[cfg(not(feature = "core-affinity"))]
    {
        let _ = cpu;
        Err(AffinityError::PinningUnsupported)
    }
}
