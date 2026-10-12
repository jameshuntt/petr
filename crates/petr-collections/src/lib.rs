//! Collections that store tagged/keyed/selector-bearing handles.

pub mod dispatch_vec;
pub mod encoded_handle;
pub mod engine;
pub mod keyed_vec;
pub mod keyed_arena;
pub mod paged_keyed_arena;
pub mod keyed_dispatch_vec;
pub mod slab;
pub mod tagged_vec;

pub use dispatch_vec::*;
pub use encoded_handle::*;
pub use engine::*;
pub use keyed_vec::*;
pub use keyed_arena::*;
pub use paged_keyed_arena::*;
pub use keyed_dispatch_vec::*;
pub use slab::*;
pub use tagged_vec::*;
