use crate::{DispatchError, DispatchResult, OpArg, OpFrame, OperationTable};

/// No-op operation.
pub fn op_nop(frame: OpFrame, _arg: &OpArg) -> DispatchResult<OpFrame> { Ok(frame) }

/// Prefix text.
pub fn op_add_prefix(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    frame.text = format!("{}{}", arg.text, frame.text);
    Ok(frame)
}

/// Suffix text.
pub fn op_add_suffix(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    frame.text = format!("{}{}", frame.text, arg.text);
    Ok(frame)
}

/// Add to accumulator.
pub fn op_acc_add(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    frame.acc += arg.n;
    Ok(frame)
}

/// Multiply accumulator.
pub fn op_acc_mul(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    frame.acc *= arg.n;
    Ok(frame)
}

/// Validate and multiply an octal string.
pub fn op_octal_multiply(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    if !frame.text.chars().all(|c| c.is_digit(8)) {
        return Err(DispatchError::InvalidInput(frame.text));
    }
    let value = u32::from_str_radix(&frame.text, 8).map_err(|_| DispatchError::InvalidInput(frame.text.clone()))?;
    frame.text = format!("{:o}", value.saturating_mul(arg.n as u32));
    Ok(frame)
}

/// Validate and add to an octal string.
pub fn op_octal_add(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    if !frame.text.chars().all(|c| c.is_digit(8)) {
        return Err(DispatchError::InvalidInput(frame.text));
    }
    let value = u32::from_str_radix(&frame.text, 8).map_err(|_| DispatchError::InvalidInput(frame.text.clone()))?;
    frame.text = format!("{:o}", value.saturating_add(arg.n as u32));
    Ok(frame)
}

/// Validate and normalize a hex/octet string.
pub fn op_octet_encode(mut frame: OpFrame, _arg: &OpArg) -> DispatchResult<OpFrame> {
    if !frame.text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DispatchError::InvalidInput(frame.text));
    }
    let value = u32::from_str_radix(&frame.text, 16).map_err(|_| DispatchError::InvalidInput(frame.text.clone()))?;
    frame.text = format!("{:X}", value);
    Ok(frame)
}

/// Validate and add to a hex/octet string.
pub fn op_octet_add(mut frame: OpFrame, arg: &OpArg) -> DispatchResult<OpFrame> {
    if !frame.text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DispatchError::InvalidInput(frame.text));
    }
    let value = u32::from_str_radix(&frame.text, 16).map_err(|_| DispatchError::InvalidInput(frame.text.clone()))?;
    frame.text = format!("{:X}", value.saturating_add(arg.n as u32));
    Ok(frame)
}

/// Install a useful default selector table.
pub fn default_table() -> OperationTable {
    let mut table = OperationTable::new();
    table.register(0x00, "nop", op_nop, OpArg::default());
    table.register(0x01, "add_prefix", op_add_prefix, OpArg::text("Prefix_"));
    table.register(0x02, "add_suffix", op_add_suffix, OpArg::text("_Suffix"));
    table.register(0x10, "octal_multiply", op_octal_multiply, OpArg::n(2));
    table.register(0x11, "octal_add", op_octal_add, OpArg::n(1));
    table.register(0x20, "octet_encode", op_octet_encode, OpArg::default());
    table.register(0x21, "octet_add", op_octet_add, OpArg::n(1));
    table.register(0x30, "acc_add", op_acc_add, OpArg::n(1));
    table.register(0x31, "acc_mul", op_acc_mul, OpArg::n(2));
    table
}
