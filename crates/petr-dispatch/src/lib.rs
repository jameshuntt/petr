//! Selector-driven operation dispatch, TOML configuration, shuffled bytecode maps,
//! and continuation-style programs.

pub mod builtins;
pub mod chain;
pub mod config;
pub mod dense;
pub mod error;
pub mod shuffle;
pub mod table;
pub mod typed;

pub use builtins::*;
pub use chain::*;
pub use config::*;
pub use dense::*;
pub use error::*;
pub use shuffle::*;
pub use table::*;
pub use typed::*;
