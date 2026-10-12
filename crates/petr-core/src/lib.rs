#![deny(unsafe_op_in_unsafe_fn)]
//! Core pointer-word primitives for the tagged pointer workspace.
//!
//! The portable core is low-bit tagging: if an allocation is aligned to `2^N`,
//! the lowest `N` address bits are known to be zero and can be used as metadata
//! while the pointer is stored as a word. Always clean the pointer before deref.

pub mod atomic_word;
pub mod error;
pub mod header;
pub mod immediate;
pub mod keyed;
pub mod layout;
pub mod prophecy;
pub mod residual;
pub mod split;
pub mod tagged;

pub use atomic_word::*;
pub use error::*;
pub use header::*;
pub use immediate::*;
pub use keyed::*;
pub use layout::*;
pub use prophecy::*;
pub use residual::*;
pub use split::*;
pub use tagged::*;
