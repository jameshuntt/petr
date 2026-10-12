use petr_core::SelectorWord;

use crate::{DispatchError, DispatchResult};

/// Small value frame passed through operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpFrame {
    /// Text payload for octal/octet/string examples.
    pub text: String,
    /// Integer accumulator for numeric examples.
    pub acc: i64,
}

impl OpFrame {
    /// Create a new frame from text.
    pub fn text(text: impl Into<String>) -> Self { Self { text: text.into(), acc: 0 } }

    /// Create a new frame from an accumulator value.
    pub fn acc(acc: i64) -> Self { Self { text: String::new(), acc } }
}

/// Dynamic operation argument.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpArg {
    /// Optional numeric parameter.
    pub n: i64,
    /// Optional string parameter.
    pub text: String,
}

impl OpArg {
    /// Build a numeric argument.
    pub fn n(n: i64) -> Self { Self { n, text: String::new() } }

    /// Build a text argument.
    pub fn text(text: impl Into<String>) -> Self { Self { n: 0, text: text.into() } }
}

/// Function pointer used by the dispatch table.
pub type OpFn = fn(OpFrame, &OpArg) -> DispatchResult<OpFrame>;

/// One dispatch table entry.
#[derive(Clone)]
pub struct OpEntry {
    /// Function to execute.
    pub func: OpFn,
    /// Bound argument.
    pub arg: OpArg,
    /// Human-readable operation name.
    pub name: String,
}

impl core::fmt::Debug for OpEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OpEntry").field("name", &self.name).field("arg", &self.arg).finish()
    }
}

/// 256-entry selector dispatch table.
#[derive(Debug, Clone)]
pub struct OperationTable {
    entries: Vec<Option<OpEntry>>,
}

impl Default for OperationTable {
    fn default() -> Self { Self::new() }
}

impl OperationTable {
    /// Create an empty 256-entry table.
    pub fn new() -> Self { Self { entries: vec![None; 256] } }

    /// Register an operation at a raw byte selector.
    pub fn register(&mut self, selector: u8, name: impl Into<String>, func: OpFn, arg: OpArg) {
        self.entries[selector as usize] = Some(OpEntry { func, arg, name: name.into() });
    }

    /// Register using a 4/4/8 selector word.
    pub fn register_word(&mut self, selector: SelectorWord, name: impl Into<String>, func: OpFn, arg: OpArg) {
        self.register(selector.function_id(), name, func, arg);
    }

    /// Execute by raw selector.
    pub fn execute(&self, selector: u8, frame: OpFrame) -> DispatchResult<OpFrame> {
        match &self.entries[selector as usize] {
            Some(entry) => (entry.func)(frame, &entry.arg),
            None => Err(DispatchError::OperationNotFound(selector)),
        }
    }

    /// Execute by selector word. Runtime uses the low 8 function bits as the table index.
    pub fn execute_word(&self, selector: SelectorWord, frame: OpFrame) -> DispatchResult<OpFrame> {
        self.execute(selector.function_id(), frame)
    }

    /// Execute a selector without a branch in user code by falling back to NOP for missing entries.
    ///
    /// This still uses an internal Option branch; use it as an ergonomic hot-loop shape, not a
    /// hard side-channel guarantee.
    pub fn execute_or_nop(&self, selector: u8, frame: OpFrame) -> OpFrame {
        if let Some(entry) = &self.entries[selector as usize] {
            (entry.func)(frame, &entry.arg).unwrap_or_else(|_| OpFrame::text("<op-error>"))
        } else {
            frame
        }
    }

    /// Return true when an entry is registered.
    pub fn contains(&self, selector: u8) -> bool { self.entries[selector as usize].is_some() }
}
