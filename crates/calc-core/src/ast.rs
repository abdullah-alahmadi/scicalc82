//! Parsed expressions.

use crate::error::CalcError;
use crate::input::{Cursor, Func, Var};
use crate::stats::StatVar;
use crate::value::Value;

/// An error and the input position the device's cursor jumps to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedError {
    pub error: CalcError,
    pub at: Cursor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    /// Where errors from this node point: an operator, function or template.
    pub at: Cursor,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Number(Value),
    Pi,
    Euler,
    Var(Var),
    Ans,
    Ran,
    /// The imaginary unit i (CMPLX mode).
    Imag,
    Stat(StatVar),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Square,
    Cube,
    Recip,
    Factorial,
    Percent,
    /// DRG▸ markers: the operand is in this unit, convert it to the current one.
    FromDeg,
    FromRad,
    FromGrad,
    Sqrt,
    Cbrt,
    Abs,
    Exp,
    /// x̂: the x estimated from a y value.
    EstimateX,
    /// ŷ: the y estimated from an x value.
    EstimateY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Perm,
    Comb,
    Pow,
    /// index, radicand
    Root,
    /// base, argument
    LogBase,
    /// r∠θ: modulus, angle (CMPLX mode)
    Polar,
}
