use crate::{DispatchError, DispatchResult, OpFrame};

/// Copyable argument for dense dispatch entries.
///
/// This avoids heap-owned per-entry strings so the dense table can be a fixed
/// 256-entry array initialized to NOP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DenseOpArg {
    /// Numeric argument.
    pub n: i64,
    /// Static text argument.
    pub text: &'static str,
}

impl DenseOpArg {
    /// Empty argument.
    pub const ZERO: Self = Self { n: 0, text: "" };

    /// Numeric argument.
    pub const fn n(n: i64) -> Self { Self { n, text: "" } }

    /// Text argument.
    pub const fn text(text: &'static str) -> Self { Self { n: 0, text } }
}

/// Function pointer used by the dense dispatch table.
pub type DenseOpFn = fn(OpFrame, DenseOpArg) -> DispatchResult<OpFrame>;

/// One fixed dispatch entry.
#[derive(Clone, Copy)]
pub struct DenseOpEntry {
    /// Function to execute.
    pub func: DenseOpFn,
    /// Bound argument.
    pub arg: DenseOpArg,
    /// Human-readable operation name.
    pub name: &'static str,
}

impl core::fmt::Debug for DenseOpEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DenseOpEntry")
            .field("name", &self.name)
            .field("arg", &self.arg)
            .finish()
    }
}

/// NOP operation for missing selectors.
pub fn dense_nop(frame: OpFrame, _arg: DenseOpArg) -> DispatchResult<OpFrame> { Ok(frame) }

/// Prefix text with a static argument.
pub fn dense_add_prefix(mut frame: OpFrame, arg: DenseOpArg) -> DispatchResult<OpFrame> {
    frame.text = format!("{}{}", arg.text, frame.text);
    Ok(frame)
}

/// Suffix text with a static argument.
pub fn dense_add_suffix(mut frame: OpFrame, arg: DenseOpArg) -> DispatchResult<OpFrame> {
    frame.text = format!("{}{}", frame.text, arg.text);
    Ok(frame)
}

/// Add to accumulator.
pub fn dense_acc_add(mut frame: OpFrame, arg: DenseOpArg) -> DispatchResult<OpFrame> {
    frame.acc += arg.n;
    Ok(frame)
}

/// Multiply accumulator.
pub fn dense_acc_mul(mut frame: OpFrame, arg: DenseOpArg) -> DispatchResult<OpFrame> {
    frame.acc *= arg.n;
    Ok(frame)
}

/// Validate and normalize a hex/octet string.
pub fn dense_octet_encode(mut frame: OpFrame, _arg: DenseOpArg) -> DispatchResult<OpFrame> {
    if !frame.text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DispatchError::InvalidInput(frame.text));
    }
    let value = u32::from_str_radix(&frame.text, 16)
        .map_err(|_| DispatchError::InvalidInput(frame.text.clone()))?;
    frame.text = format!("{:X}", value);
    Ok(frame)
}

const NOP_ENTRY: DenseOpEntry = DenseOpEntry {
    func: dense_nop,
    arg: DenseOpArg::ZERO,
    name: "nop",
};

/// A fixed 256-slot selector table with every missing selector pre-filled as NOP.
///
/// This removes the `Option`/`match` from the hot dispatch path. It is still not
/// literally branchless at the CPU level, because calling a function pointer is
/// an indirect branch. The useful property is that selector choice is table
/// indexing rather than an `if`/`else` or `match` chain.
#[derive(Debug, Clone)]
pub struct DenseOperationTable {
    entries: [DenseOpEntry; 256],
}

impl Default for DenseOperationTable {
    fn default() -> Self { Self::new() }
}

impl DenseOperationTable {
    /// Create a dense table full of NOP entries.
    pub fn new() -> Self { Self { entries: [NOP_ENTRY; 256] } }

    /// Register a selector.
    pub fn register(&mut self, selector: u8, name: &'static str, func: DenseOpFn, arg: DenseOpArg) {
        self.entries[selector as usize] = DenseOpEntry { func, arg, name };
    }

    /// Execute the selector. Missing entries execute NOP.
    pub fn execute(&self, selector: u8, frame: OpFrame) -> DispatchResult<OpFrame> {
        let entry = self.entries[selector as usize];
        (entry.func)(frame, entry.arg)
    }

    /// Return the static operation name registered for a selector.
    pub fn name(&self, selector: u8) -> &'static str { self.entries[selector as usize].name }
}

/// Install a useful dense table.
pub fn default_dense_table() -> DenseOperationTable {
    let mut table = DenseOperationTable::new();
    table.register(0x01, "add_prefix", dense_add_prefix, DenseOpArg::text("Prefix_"));
    table.register(0x02, "add_suffix", dense_add_suffix, DenseOpArg::text("_Suffix"));
    table.register(0x20, "octet_encode", dense_octet_encode, DenseOpArg::ZERO);
    table.register(0x30, "acc_add", dense_acc_add, DenseOpArg::n(1));
    table.register(0x31, "acc_mul", dense_acc_mul, DenseOpArg::n(2));
    table
}
