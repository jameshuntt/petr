use petr_protection::{ProtectionCapabilities, ProtectionError, ProtectionRequirement, require};

use crate::{native_mte_compiled, native_pac_compiled};

/// Which implementation family is intended to satisfy a protection policy.
///
/// The portable model is deterministic and testable on every target. Native
/// hardware is intentionally conservative: capability discovery only reports
/// mechanisms that were compiled for this target. It is not a runtime CPU/OS
/// probe and it does not imply that an arbitrary heap mapping is MTE-backed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtectionBackend {
    SoftwareModel,
    NativeHardware,
}

pub const fn backend_capabilities(backend: ProtectionBackend) -> ProtectionCapabilities {
    match backend {
        ProtectionBackend::SoftwareModel => ProtectionCapabilities::ALL,
        ProtectionBackend::NativeHardware => {
            ProtectionCapabilities::new(native_pac_compiled(), native_mte_compiled())
        }
    }
}

pub fn require_backend(
    backend: ProtectionBackend,
    requirement: ProtectionRequirement,
) -> Result<(), ProtectionError> {
    require(backend_capabilities(backend), requirement)
}
