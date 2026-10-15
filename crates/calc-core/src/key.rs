//! Physical keys and what they do with SHIFT and ALPHA.
//!
//! The layout follows a classic school scientific calculator closely, but this
//! table is the one place to change if a key should behave differently.

use crate::input::{Func, TemplateKind, Token, Var};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Shift,
    Alpha,
    Mode,
    On,
    Up,
    Down,
    Left,
    Right,
    Abs,
    Cube,
    Recip,
    LogBase,
    Fraction,
    Sqrt,
    Square,
    Power,
    Log,
    Ln,
    Neg,
    Dms,
    Hyp,
    Sin,
    Cos,
    Tan,
    Rcl,
    Eng,
    LParen,
    RParen,
    SToD,
    MPlus,
    Digit(u8),
    Del,
    Ac,
    Times,
    Divide,
    Plus,
    Minus,
    Point,
    Exp10,
    Ans,
    Equals,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Modifier {
    #[default]
    None,
    Shift,
    Alpha,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Insert(Token),
    Template(TemplateKind),
    Left,
    Right,
    Up,
    Down,
    Del,
    Ac,
    Equals,
    /// SHIFT = : show the result as a decimal.
    ApproxEquals,
    SToD,
    /// ENG (+1) and SHIFT ENG (−1).
    Eng(i32),
    /// M+ (false) and M− (true).
    MemoryAdd {
        subtract: bool,
    },
    Sto,
    Rcl,
    Hyp,
    DrgMenu,
    ClrMenu,
    /// SHIFT 1: the STAT menu (STAT mode only).
    StatMenu,
    /// SHIFT 2: the CMPLX menu (CMPLX mode only).
    CmplxMenu,
    /// SHIFT 4: the MATRIX menu (MATRIX mode only).
    MatrixMenu,
    /// SHIFT 5: the VECTOR menu (VECTOR mode only).
    VectorMenu,
    Mode,
    Setup,
    Nothing,
}

pub(crate) fn action(key: Key, modifier: Modifier) -> Action {
    use Action::{Insert, Template};
    use Modifier::{Alpha, Shift};
    let func = |f| Insert(Token::Func(f));
    let var = |v| Insert(Token::Var(v));
    match (key, modifier) {
        (Key::Shift | Key::Alpha, _) => Action::Nothing,
        (Key::Mode, Shift) => Action::Setup,
        (Key::Mode, _) => Action::Mode,
        (Key::On, _) => Action::Ac,
        (Key::Up, _) => Action::Up,
        (Key::Down, _) => Action::Down,
        (Key::Left, _) => Action::Left,
        (Key::Right, _) => Action::Right,
        (Key::Abs, _) => Template(TemplateKind::Abs),
        (Key::Cube, _) => Insert(Token::Cube),
        (Key::Recip, Shift) => Insert(Token::Factorial),
        (Key::Recip, _) => Insert(Token::Recip),
        (Key::LogBase, _) => Template(TemplateKind::LogBase),
        (Key::Fraction, Shift) => Template(TemplateKind::MixedFraction),
        (Key::Fraction, _) => Template(TemplateKind::Fraction),
        (Key::Sqrt, Shift) => Template(TemplateKind::CubeRoot),
        (Key::Sqrt, _) => Template(TemplateKind::Sqrt),
        (Key::Square, _) => Insert(Token::Square),
        (Key::Power, Shift) => Template(TemplateKind::Root),
        (Key::Power, _) => Template(TemplateKind::Power),
        (Key::Log, Shift) => Template(TemplateKind::Pow10),
        (Key::Log, _) => func(Func::Log),
        (Key::Ln, Shift) => Template(TemplateKind::ExpE),
        (Key::Ln, _) => func(Func::Ln),
        (Key::Neg, Alpha) => var(Var::A),
        (Key::Neg, Shift) => Insert(Token::Angle),
        (Key::Neg, _) => Insert(Token::Neg),
        (Key::Dms, Alpha) => var(Var::B),
        (Key::Dms, _) => Insert(Token::Dms),
        (Key::Hyp, Alpha) => var(Var::C),
        (Key::Hyp, _) => Action::Hyp,
        (Key::Sin, Alpha) => var(Var::D),
        (Key::Sin, Shift) => func(Func::Asin),
        (Key::Sin, _) => func(Func::Sin),
        (Key::Cos, Alpha) => var(Var::E),
        (Key::Cos, Shift) => func(Func::Acos),
        (Key::Cos, _) => func(Func::Cos),
        (Key::Tan, Alpha) => var(Var::F),
        (Key::Tan, Shift) => func(Func::Atan),
        (Key::Tan, _) => func(Func::Tan),
        (Key::Rcl, Shift) => Action::Sto,
        (Key::Rcl, _) => Action::Rcl,
        (Key::Eng, Shift) => Action::Eng(-1),
        (Key::Eng, _) => Action::Eng(1),
        (Key::LParen, Shift) => Insert(Token::Percent),
        (Key::LParen, _) => Insert(Token::LParen),
        (Key::RParen, Shift) => Insert(Token::Comma),
        (Key::RParen, Alpha) => var(Var::X),
        (Key::RParen, _) => Insert(Token::RParen),
        (Key::SToD, Alpha) => var(Var::Y),
        (Key::SToD, _) => Action::SToD,
        (Key::MPlus, Shift) => Action::MemoryAdd { subtract: true },
        (Key::MPlus, Alpha) => var(Var::M),
        (Key::MPlus, _) => Action::MemoryAdd { subtract: false },
        (Key::Digit(9), Shift) => Action::ClrMenu,
        (Key::Digit(1), Shift) => Action::StatMenu,
        (Key::Digit(2), Shift) => Action::CmplxMenu,
        (Key::Digit(4), Shift) => Action::MatrixMenu,
        (Key::Digit(5), Shift) => Action::VectorMenu,
        (Key::Digit(0), Shift) => func(Func::Rnd),
        (Key::Digit(d), _) => Insert(Token::Digit(d)),
        (Key::Del, _) => Action::Del,
        (Key::Ac, _) => Action::Ac,
        (Key::Times, Shift) => Insert(Token::Perm),
        (Key::Times, _) => Insert(Token::Times),
        (Key::Divide, Shift) => Insert(Token::Comb),
        (Key::Divide, _) => Insert(Token::Divide),
        (Key::Plus, Shift) => func(Func::Pol),
        (Key::Plus, _) => Insert(Token::Plus),
        (Key::Minus, Shift) => func(Func::Rec),
        (Key::Minus, _) => Insert(Token::Minus),
        (Key::Point, Shift) => Insert(Token::Ran),
        (Key::Point, Alpha) => func(Func::RanInt),
        (Key::Point, _) => Insert(Token::Point),
        (Key::Exp10, Shift) => Insert(Token::Pi),
        (Key::Exp10, Alpha) => Insert(Token::Euler),
        (Key::Exp10, _) => Insert(Token::Exp10),
        (Key::Ans, Shift) => Action::DrgMenu,
        (Key::Ans, _) => Insert(Token::Ans),
        (Key::Equals, Shift) => Action::ApproxEquals,
        (Key::Equals, _) => Action::Equals,
    }
}

/// The variable a key selects after STO or RCL (its ALPHA legend).
pub(crate) fn var_for_key(key: Key) -> Option<Var> {
    Some(match key {
        Key::Neg => Var::A,
        Key::Dms => Var::B,
        Key::Hyp => Var::C,
        Key::Sin => Var::D,
        Key::Cos => Var::E,
        Key::Tan => Var::F,
        Key::RParen => Var::X,
        Key::SToD => Var::Y,
        Key::MPlus => Var::M,
        _ => return None,
    })
}

/// The function a trig key gives after hyp.
pub(crate) fn hyperbolic(key: Key, modifier: Modifier) -> Option<Func> {
    let inverse = modifier == Modifier::Shift;
    Some(match (key, inverse) {
        (Key::Sin, false) => Func::Sinh,
        (Key::Cos, false) => Func::Cosh,
        (Key::Tan, false) => Func::Tanh,
        (Key::Sin, true) => Func::Asinh,
        (Key::Cos, true) => Func::Acosh,
        (Key::Tan, true) => Func::Atanh,
        _ => return None,
    })
}
