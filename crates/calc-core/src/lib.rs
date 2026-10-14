//! UI-independent engine for a school-style scientific calculator.

pub mod ast;
pub mod calculator;
pub mod error;
pub mod eval;
pub mod format;
pub mod input;
pub mod key;
pub mod matrix;
pub mod menu;
pub mod number;
pub mod parse;
pub mod state;
pub mod stats;
pub mod table;
pub mod value;

pub use calculator::Calculator;
pub use error::{CalcError, CalcResult};
pub use key::Key;
pub use value::Value;
