//! `petr`: tagged pointer words and the tooling around them.
//!
//! This is the facade. Each piece is its own crate and is re-exported here
//! under a module name: [`core`], [`dispatch`], [`collections`], [`resource`],
//! [`protection`], [`temporal`], [`affinity`], [`execution`], [`math`], and [`spatial`].
//! With feature `hardware-experiments`, [`hw`] exposes optional PAC/MTE adapters.
//! The core items are also re-exported at the top level.

pub use petr_affinity as affinity;
pub use petr_collections as collections;
pub use petr_core as core;
pub use petr_dispatch as dispatch;
pub use petr_execution as execution;
#[cfg(feature = "hardware-experiments")]
pub use petr_hw as hw;
pub use petr_math as math;
pub use petr_protection as protection;
pub use petr_resource as resource;
pub use petr_spatial as spatial;
pub use petr_temporal as temporal;

pub use petr_core::*;
