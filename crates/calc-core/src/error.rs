use thiserror::Error;

/// The error screens of the real device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CalcError {
    /// Out-of-range results or domain errors (division by zero, √ of a negative, ...).
    #[error("Math ERROR")]
    Math,
    /// Malformed input.
    #[error("Syntax ERROR")]
    Syntax,
    /// Expression too deeply nested.
    #[error("Stack ERROR")]
    Stack,
    /// Invalid argument to a function or command.
    #[error("Argument ERROR")]
    Argument,
    /// Too much data, such as a TABLE with more than 30 rows.
    #[error("Insufficient MEM")]
    Memory,
    /// Matrix sizes that don't fit the operation, such as adding 2×2 and 3×1.
    #[error("Dimension ERROR")]
    Dimension,
}

pub type CalcResult<T> = Result<T, CalcError>;
