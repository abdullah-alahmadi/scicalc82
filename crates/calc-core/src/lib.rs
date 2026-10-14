//! UI-independent engine for a school-style scientific calculator.

pub mod error;
pub mod format;
pub mod value;

pub use error::{CalcError, CalcResult};
pub use value::Value;
