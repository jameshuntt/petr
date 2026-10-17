//! Portable protection policy shared by PETR execution, hardware experiments,
//! and memory bridges.
//!
//! This crate deliberately contains no pointer manipulation and no allocator.
//! It only names what a consumer requires and what a backend can provide.

#![deny(unsafe_code)]

use thiserror::Error;

/// Protection semantics requested by a handler or allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProtectionRequirement {
    /// No authentication or memory-tagging requirement.
    #[default]
    None,
    /// Pointer/authentication proof must be available.
    Authentication,
    /// Allocation/pointer memory-tag validation must be available.
    MemoryTagging,
    /// Both authentication and memory-tag validation are required.
    AuthenticationAndMemoryTagging,
}

impl ProtectionRequirement {
    pub const fn requires_authentication(self) -> bool {
        matches!(
            self,
            Self::Authentication | Self::AuthenticationAndMemoryTagging
        )
    }

    pub const fn requires_memory_tagging(self) -> bool {
        matches!(
            self,
            Self::MemoryTagging | Self::AuthenticationAndMemoryTagging
        )
    }

    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}

/// Capabilities supplied by a concrete protection backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ProtectionCapabilities {
    authentication: bool,
    memory_tagging: bool,
}

impl ProtectionCapabilities {
    pub const NONE: Self = Self::new(false, false);
    pub const AUTHENTICATION: Self = Self::new(true, false);
    pub const MEMORY_TAGGING: Self = Self::new(false, true);
    pub const ALL: Self = Self::new(true, true);

    pub const fn new(authentication: bool, memory_tagging: bool) -> Self {
        Self {
            authentication,
            memory_tagging,
        }
    }

    pub const fn authentication(self) -> bool {
        self.authentication
    }
    pub const fn memory_tagging(self) -> bool {
        self.memory_tagging
    }

    pub const fn supports(self, requirement: ProtectionRequirement) -> bool {
        (!requirement.requires_authentication() || self.authentication)
            && (!requirement.requires_memory_tagging() || self.memory_tagging)
    }

    pub const fn union(self, other: Self) -> Self {
        Self::new(
            self.authentication || other.authentication,
            self.memory_tagging || other.memory_tagging,
        )
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self::new(
            self.authentication && other.authentication,
            self.memory_tagging && other.memory_tagging,
        )
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionError {
    #[error(
        "protection requirement {requirement:?} is not supported by capabilities {capabilities:?}"
    )]
    Unsupported {
        requirement: ProtectionRequirement,
        capabilities: ProtectionCapabilities,
    },
}

pub fn require(
    capabilities: ProtectionCapabilities,
    requirement: ProtectionRequirement,
) -> Result<(), ProtectionError> {
    if capabilities.supports(requirement) {
        Ok(())
    } else {
        Err(ProtectionError::Unsupported {
            requirement,
            capabilities,
        })
    }
}
