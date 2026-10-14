//! The structured input of the natural display.
//!
//! An expression is a [`Row`] of [`Item`]s. Most items are single [`Token`]s
//! (a digit, an operator, `sin(`), which DEL removes as a unit. A
//! [`Template`] has slots that hold rows of their own, such as a fraction's
//! numerator and denominator.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::matrix::MatId;
use crate::stats::StatVar;

/// Functions that are entered as a name plus an opening parenthesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
    Log,
    Ln,
    Pol,
    Rec,
    RanInt,
    Rnd,
    /// LineIO forms of the √, ∛ and Abs templates.
    Sqrt,
    Cbrt,
    Abs,
    /// CMPLX: the argument (angle) of a complex number.
    Arg,
    /// CMPLX: the complex conjugate.
    Conjg,
    /// MATRIX: the determinant.
    Det,
    /// MATRIX: the transpose.
    Trn,
}

impl Func {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sin => "sin(",
            Self::Cos => "cos(",
            Self::Tan => "tan(",
            Self::Asin => "sin⁻¹(",
            Self::Acos => "cos⁻¹(",
            Self::Atan => "tan⁻¹(",
            Self::Sinh => "sinh(",
            Self::Cosh => "cosh(",
            Self::Tanh => "tanh(",
            Self::Asinh => "sinh⁻¹(",
            Self::Acosh => "cosh⁻¹(",
            Self::Atanh => "tanh⁻¹(",
            Self::Log => "log(",
            Self::Ln => "ln(",
            Self::Pol => "Pol(",
            Self::Rec => "Rec(",
            Self::RanInt => "RanInt#(",
            Self::Rnd => "Rnd(",
            Self::Sqrt => "√(",
            Self::Cbrt => "∛(",
            Self::Abs => "Abs(",
            Self::Arg => "arg(",
            Self::Conjg => "Conjg(",
            Self::Det => "det(",
            Self::Trn => "Trn(",
        }
    }
}

/// Memory locations that can appear in an expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Var {
    A,
    B,
    C,
    D,
    E,
    F,
    X,
    Y,
    M,
}

impl Var {
    pub fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::X => "X",
            Self::Y => "Y",
            Self::M => "M",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Token {
    Digit(u8),
    Point,
    /// The ×10^x key, written `E`.
    Exp10,
    Plus,
    Minus,
    Times,
    Divide,
    /// The (−) key, distinct from subtraction.
    Neg,
    LParen,
    RParen,
    Comma,
    Func(Func),
    Square,
    Cube,
    Recip,
    Factorial,
    Percent,
    /// The ° ′ ″ key, separating degrees, minutes and seconds.
    Dms,
    /// DRG▸ unit markers.
    AngleDeg,
    AngleRad,
    AngleGrad,
    /// nPr
    Perm,
    /// nCr
    Comb,
    Pi,
    Euler,
    Ans,
    Ran,
    Var(Var),
    /// LineIO fraction separator: `a⌟b`, and `a⌟b⌟c` for mixed fractions.
    FracBar,
    /// LineIO power `^(`: the base comes before it.
    PowOpen,
    /// LineIO root `ˣ√(`: the index comes before it.
    RootOpen,
    /// A STAT value such as x̄ or Σx.
    Stat(StatVar),
    /// Regression estimates, written after their argument: x̂ and ŷ.
    EstX,
    EstY,
    /// The imaginary unit i (CMPLX mode).
    Imag,
    /// ∠ between a modulus and an angle (CMPLX mode).
    Angle,
    /// Show the result in polar form, ▸r∠θ (CMPLX mode, at the end of the input).
    ToPolar,
    /// Show the result in rectangular form, ▸a+bi (CMPLX mode, at the end of the input).
    ToRect,
    /// A matrix variable (MATRIX mode).
    Mat(MatId),
}

impl Token {
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Digit(_) | Self::Point)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Digit(d) => return write!(f, "{d}"),
            Self::Point => ".",
            Self::Exp10 => "E",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Times => "×",
            Self::Divide => "÷",
            Self::Neg => "−",
            Self::LParen => "(",
            Self::RParen => ")",
            Self::Comma => ",",
            Self::Func(func) => func.name(),
            Self::Square => "²",
            Self::Cube => "³",
            Self::Recip => "⁻¹",
            Self::Factorial => "!",
            Self::Percent => "%",
            Self::Dms | Self::AngleDeg => "°",
            Self::AngleRad => "ʳ",
            Self::AngleGrad => "ᵍ",
            Self::Perm => "P",
            Self::Comb => "C",
            Self::Pi => "π",
            Self::Euler => "e",
            Self::Ans => "Ans",
            Self::Ran => "Ran#",
            Self::Var(v) => v.name(),
            Self::FracBar => "⌟",
            Self::PowOpen => "^(",
            Self::RootOpen => "ˣ√(",
            Self::Stat(s) => s.name(),
            Self::EstX => "x̂",
            Self::EstY => "ŷ",
            Self::Imag => "i",
            Self::Angle => "∠",
            Self::ToPolar => "▸r∠θ",
            Self::ToRect => "▸a+bi",
            Self::Mat(m) => m.name(),
        };
        f.write_str(text)
    }
}

/// The kinds of template, with their slots in cursor order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateKind {
    /// numerator, denominator
    Fraction,
    /// whole, numerator, denominator
    MixedFraction,
    /// exponent; the base is the item before the template
    Power,
    /// exponent of 10^■
    Pow10,
    /// exponent of e^■
    ExpE,
    /// radicand
    Sqrt,
    /// radicand
    CubeRoot,
    /// index, radicand
    Root,
    /// base, argument
    LogBase,
    /// argument
    Abs,
}

impl TemplateKind {
    pub fn slot_count(self) -> usize {
        match self {
            Self::MixedFraction => 3,
            Self::Fraction | Self::Root | Self::LogBase => 2,
            Self::Power | Self::Pow10 | Self::ExpE | Self::Sqrt | Self::CubeRoot | Self::Abs => 1,
        }
    }

    /// Whether a number typed just before this template becomes its first slot.
    pub fn absorbs_number(self) -> bool {
        matches!(self, Self::Fraction | Self::MixedFraction)
    }

    /// For the ▲/▼ keys: the slot directly above or below `slot`, if any.
    pub fn vertical_neighbor(self, slot: usize, down: bool) -> Option<usize> {
        match (self, slot, down) {
            (Self::Fraction, 0, true) | (Self::MixedFraction, 1, true) => Some(slot + 1),
            (Self::Fraction, 1, false) | (Self::MixedFraction, 2, false) => Some(slot - 1),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub items: Vec<Item>,
}

impl Row {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Items in this row and every nested slot; each template counts once
    /// itself, plus its contents.
    pub fn total_items(&self) -> usize {
        self.items
            .iter()
            .map(|item| match item {
                Item::Token(_) => 1,
                Item::Template(t) => 1 + t.slots.iter().map(Row::total_items).sum::<usize>(),
            })
            .sum()
    }
}

impl From<Vec<Item>> for Row {
    fn from(items: Vec<Item>) -> Self {
        Self { items }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Token(Token),
    Template(Template),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TemplateRepr")]
pub struct Template {
    pub kind: TemplateKind,
    pub slots: Vec<Row>,
}

/// Loaded form of a [`Template`], checked so its slots match its kind.
#[derive(Deserialize)]
struct TemplateRepr {
    kind: TemplateKind,
    slots: Vec<Row>,
}

impl TryFrom<TemplateRepr> for Template {
    type Error = &'static str;

    fn try_from(t: TemplateRepr) -> Result<Self, Self::Error> {
        if t.slots.len() != t.kind.slot_count() {
            return Err("wrong number of template slots");
        }
        Ok(Self { kind: t.kind, slots: t.slots })
    }
}

impl Template {
    pub fn new(kind: TemplateKind) -> Self {
        Self { kind, slots: vec![Row::default(); kind.slot_count()] }
    }
}
