//! Experimental hardware-backed pointer hardening for PETR.
//!
//! This crate is deliberately outside `petr-core`. The portable PETR contract
//! remains provenance-preserving software metadata. This crate maps clean PETR
//! pointers into two optional AArch64 mechanisms:
//!
//! - pointer authentication (PAC), and
//! - memory tagging (MTE).
//!
//! PAC and MTE both use architectural pointer bits. They therefore must not be
//! treated as extra storage for [`petr_core::SplitTaggedNonNull`]'s logical top
//! byte. The compatibility helpers always recover the clean, provenance-carrying
//! pointer first and keep PETR's software phase byte separate.
//!
//! The portable PAC/MTE models are useful for testing policy on any machine.
//! Native instructions are opt-in experiments and never participate in the
//! default PETR build.

#![deny(unsafe_op_in_unsafe_fn)]

pub mod capability;
pub mod compatibility;
pub mod mte;
pub mod pac;

pub use capability::*;
pub use compatibility::*;
pub use mte::*;
pub use pac::*;

pub use petr_protection::{ProtectionCapabilities, ProtectionError, ProtectionRequirement};
