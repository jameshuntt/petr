use thiserror::Error;

/// Result type for operation dispatch.
pub type DispatchResult<T> = Result<T, DispatchError>;

/// Errors returned by selector dispatch.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DispatchError {
    /// No operation was registered for the selector.
    #[error("operation not found for selector {0:#04x}")]
    OperationNotFound(u8),

    /// Input was not valid for the selected operation.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// TOML config declared an operation type this crate does not know.
    #[error("unknown operation type: {0}")]
    UnknownOperationType(String),

    /// TOML config omitted a required parameter.
    #[error("missing parameter `{param}` for operation `{op}`")]
    MissingParam { op: String, param: &'static str },

    /// Program counter stepped out of the chain.
    #[error("invalid continuation index {0}")]
    InvalidContinuation(usize),
}
