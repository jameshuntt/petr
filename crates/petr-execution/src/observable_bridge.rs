//! Optional bridge that expresses execution eligibility as an observable wake key.
//!
//! `ob-serve-a-bowl` remains the observer runtime. PETR only supplies semantic
//! fields (reason, QoS, logical phase and an execution-event field bit).

use crate::ExecutionProfile;
use ob_serve_a_bowl::{WakeKey64, WakeProfileCode, WakeReasonCode};
use petr_temporal::Tick;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionWakeKind {
    Admitted,
    TemporalEligible,
    ProfileTransition,
    ResourcePressure,
}

impl ExecutionWakeKind {
    pub const fn field_mask(self) -> u16 {
        match self {
            Self::Admitted => 1 << 0,
            Self::TemporalEligible => 1 << 1,
            Self::ProfileTransition => 1 << 2,
            Self::ResourcePressure => 1 << 3,
        }
    }

    pub const fn reason(self) -> WakeReasonCode {
        match self {
            Self::Admitted => WakeReasonCode::Spawn,
            Self::TemporalEligible => WakeReasonCode::Timer,
            Self::ProfileTransition => WakeReasonCode::Qos,
            Self::ResourcePressure => WakeReasonCode::MemoryPressure,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ObservableBridgeError {
    #[error("execution wake key could not be encoded")]
    Encode,
}

pub fn execution_wake_key(
    kind: ExecutionWakeKind,
    profile: ExecutionProfile,
    tick: Tick,
) -> Result<WakeKey64, ObservableBridgeError> {
    ob_serve_a_bowl::api::wake_key()
        .reason(kind.reason())
        .profile(WakeProfileCode::AtomicEpoch)
        .qos(profile.qos.priority())
        .field_mask(kind.field_mask())
        .phase((tick.0 & 0xff) as u8)
        .hot()
        .build()
        .map_err(|_| ObservableBridgeError::Encode)
}
