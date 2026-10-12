//! `petr`: tagged pointer words and the tooling around them.
//!
//! This is the facade. Each piece is its own crate and is re-exported here
//! under a module name: [`core`], [`dispatch`], [`collections`], [`resource`],
//! [`math`], [`spatial`]. The core items are also re-exported at the top level.

pub use petr_core as core;
pub use petr_dispatch as dispatch;
pub use petr_collections as collections;
pub use petr_resource as resource;
pub use petr_math as math;
pub use petr_spatial as spatial;

pub use petr_core::*;
