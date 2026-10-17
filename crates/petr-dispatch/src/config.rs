use std::collections::HashMap;

use serde::Deserialize;

use crate::{
    DispatchError, DispatchResult, OpArg, OperationTable, op_acc_add, op_acc_mul, op_add_prefix,
    op_add_suffix, op_nop, op_octal_add, op_octal_multiply, op_octet_add, op_octet_encode,
};

/// Top-level operation TOML config.
#[derive(Debug, Deserialize)]
pub struct OperationConfig {
    /// Operation declarations keyed by hex or decimal selector string.
    pub operations: HashMap<String, OperationDefinition>,
}

/// One operation declaration.
#[derive(Debug, Deserialize)]
pub struct OperationDefinition {
    /// Operation kind.
    #[serde(rename = "type")]
    pub op_type: String,
    /// Optional prefix string.
    pub prefix: Option<String>,
    /// Optional suffix string.
    pub suffix: Option<String>,
    /// Optional multiplier.
    pub multiplier: Option<i64>,
    /// Optional numeric value.
    pub value: Option<i64>,
}

/// Parse a selector string like `0x10`, `10`, or `32`.
pub fn parse_selector(text: &str) -> DispatchResult<u8> {
    let trimmed = text.trim();
    let parsed = if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u8::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<u8>()
    };
    parsed.map_err(|_| DispatchError::InvalidInput(format!("bad selector `{text}`")))
}

/// Build an operation table from TOML text.
pub fn table_from_toml(text: &str) -> DispatchResult<OperationTable> {
    let config: OperationConfig =
        toml::from_str(text).map_err(|e| DispatchError::InvalidInput(e.to_string()))?;
    table_from_config(config)
}

/// Build an operation table from a parsed config.
pub fn table_from_config(config: OperationConfig) -> DispatchResult<OperationTable> {
    let mut table = OperationTable::new();
    for (selector_text, op) in config.operations {
        let selector = parse_selector(&selector_text)?;
        let name = op.op_type.clone();
        match op.op_type.as_str() {
            "nop" => table.register(selector, name, op_nop, OpArg::default()),
            "add_prefix" => {
                let prefix = op.prefix.ok_or_else(|| DispatchError::MissingParam {
                    op: name.clone(),
                    param: "prefix",
                })?;
                table.register(selector, name, op_add_prefix, OpArg::text(prefix));
            }
            "add_suffix" => {
                let suffix = op.suffix.ok_or_else(|| DispatchError::MissingParam {
                    op: name.clone(),
                    param: "suffix",
                })?;
                table.register(selector, name, op_add_suffix, OpArg::text(suffix));
            }
            "octal_multiply" => {
                let n = op.multiplier.ok_or_else(|| DispatchError::MissingParam {
                    op: name.clone(),
                    param: "multiplier",
                })?;
                table.register(selector, name, op_octal_multiply, OpArg::n(n));
            }
            "octal_add" => {
                let n = op.value.ok_or_else(|| DispatchError::MissingParam {
                    op: name.clone(),
                    param: "value",
                })?;
                table.register(selector, name, op_octal_add, OpArg::n(n));
            }
            "octet_encode" => table.register(selector, name, op_octet_encode, OpArg::default()),
            "octet_add" => {
                let n = op.value.ok_or_else(|| DispatchError::MissingParam {
                    op: name.clone(),
                    param: "value",
                })?;
                table.register(selector, name, op_octet_add, OpArg::n(n));
            }
            "acc_add" => {
                table.register(selector, name, op_acc_add, OpArg::n(op.value.unwrap_or(1)))
            }
            "acc_mul" => table.register(
                selector,
                name,
                op_acc_mul,
                OpArg::n(op.multiplier.unwrap_or(2)),
            ),
            other => return Err(DispatchError::UnknownOperationType(other.to_string())),
        }
    }
    Ok(table)
}
