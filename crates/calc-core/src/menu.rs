//! The device's numbered menus: MODE, SETUP, DRG▸, CLR and the STAT menus.

use crate::format::{FractionStyle, NumberFormat};
use crate::input::{Func, Token};
use crate::matrix::MatId;
use crate::state::{AngleUnit, CalcMode};
use crate::stats::{StatKind, StatVar};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuId {
    Mode,
    StatType,
    /// SETUP, first page.
    Setup,
    /// SETUP, second page.
    Setup2,
    Fix,
    Sci,
    Norm,
    StatFreq,
    Drg,
    Clr,
    Stat,
    StatSum,
    StatVar,
    StatReg,
    StatMinMax,
    /// SHIFT 2 in CMPLX mode: arg, Conjg, ▸r∠θ, ▸a+bi.
    Cmplx,
    /// SETUP: show CMPLX results as a+bi or r∠θ.
    CmplxFormat,
    /// SHIFT 4 in MATRIX mode.
    Matrix,
    /// Which matrix to size (`define`) or edit.
    MatSelect {
        define: bool,
    },
    /// The size of a matrix; `more` is the second page (1×n sizes).
    MatDim {
        id: MatId,
        more: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Mode(CalcMode),
    Open(MenuId),
    StatType(StatKind),
    /// MathIO (true) or LineIO (false).
    Natural(bool),
    Angle(AngleUnit),
    Number(NumberFormat),
    Fraction(FractionStyle),
    StatFreq(bool),
    Insert(Token),
    /// CLR: 1 Setup, 2 Memory, 3 All.
    Clear(u8),
    StatEditor,
    /// CMPLX results in polar (true) or rectangular form.
    ComplexPolar(bool),
    /// Create a matrix of zeros with this size and edit it.
    DefineMatrix(MatId, usize, usize),
    /// Edit a matrix's cells (or size it first if it doesn't exist yet).
    EditMatrix(MatId),
}

/// An open menu as the display shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuView {
    pub title: Option<&'static str>,
    /// Numbered items, e.g. `1:COMP`. Empty for a digit prompt.
    pub items: Vec<String>,
    /// A digit prompt such as `Fix 0~9?`.
    pub prompt: Option<&'static str>,
    pub more_below: bool,
    pub more_above: bool,
}

fn stat_items(vars: &[StatVar]) -> Vec<(&'static str, MenuAction)> {
    vars.iter().map(|&v| (v.name(), MenuAction::Insert(Token::Stat(v)))).collect()
}

fn entries(id: MenuId, kind: StatKind) -> Vec<(&'static str, MenuAction)> {
    use MenuAction::*;
    let paired = kind.paired();
    match id {
        MenuId::Mode => {
            vec![
                ("COMP", Mode(CalcMode::Comp)),
                ("CMPLX", Mode(CalcMode::Cmplx)),
                ("STAT", Open(MenuId::StatType)),
                ("TABLE", Mode(CalcMode::Table)),
                ("MATRIX", Mode(CalcMode::Matrix)),
            ]
        }
        MenuId::StatType => vec![("1-VAR", StatType(StatKind::OneVar)), ("A+BX", StatType(StatKind::Linear))],
        MenuId::Setup => vec![
            ("MthIO", Natural(true)),
            ("LineIO", Natural(false)),
            ("Deg", Angle(AngleUnit::Deg)),
            ("Rad", Angle(AngleUnit::Rad)),
            ("Gra", Angle(AngleUnit::Gra)),
            ("Fix", Open(MenuId::Fix)),
            ("Sci", Open(MenuId::Sci)),
            ("Norm", Open(MenuId::Norm)),
        ],
        MenuId::Setup2 => vec![
            ("ab/c", Fraction(FractionStyle::Mixed)),
            ("d/c", Fraction(FractionStyle::Improper)),
            ("STAT", Open(MenuId::StatFreq)),
            ("CMPLX", Open(MenuId::CmplxFormat)),
        ],
        MenuId::CmplxFormat => vec![("a+bi", ComplexPolar(false)), ("r∠θ", ComplexPolar(true))],
        MenuId::Matrix => vec![
            ("Dim", Open(MenuId::MatSelect { define: true })),
            ("Data", Open(MenuId::MatSelect { define: false })),
            ("MatA", Insert(Token::Mat(MatId::A))),
            ("MatB", Insert(Token::Mat(MatId::B))),
            ("MatC", Insert(Token::Mat(MatId::C))),
            ("MatAns", Insert(Token::Mat(MatId::Ans))),
            ("det", Insert(Token::Func(Func::Det))),
            ("Trn", Insert(Token::Func(Func::Trn))),
        ],
        MenuId::MatSelect { define } => MatId::STORED
            .iter()
            .map(|&id| (id.name(), if define { Open(MenuId::MatDim { id, more: false }) } else { EditMatrix(id) }))
            .collect(),
        MenuId::MatDim { id, more: false } => vec![
            ("3×3", DefineMatrix(id, 3, 3)),
            ("3×2", DefineMatrix(id, 3, 2)),
            ("3×1", DefineMatrix(id, 3, 1)),
            ("2×3", DefineMatrix(id, 2, 3)),
            ("2×2", DefineMatrix(id, 2, 2)),
            ("2×1", DefineMatrix(id, 2, 1)),
        ],
        MenuId::MatDim { id, more: true } => {
            vec![("1×3", DefineMatrix(id, 1, 3)), ("1×2", DefineMatrix(id, 1, 2)), ("1×1", DefineMatrix(id, 1, 1))]
        }
        MenuId::Cmplx => vec![
            ("arg", Insert(Token::Func(Func::Arg))),
            ("Conjg", Insert(Token::Func(Func::Conjg))),
            ("▸r∠θ", Insert(Token::ToPolar)),
            ("▸a+bi", Insert(Token::ToRect)),
        ],
        MenuId::StatFreq => vec![("ON", StatFreq(true)), ("OFF", StatFreq(false))],
        MenuId::Drg => {
            vec![("°", Insert(Token::AngleDeg)), ("r", Insert(Token::AngleRad)), ("g", Insert(Token::AngleGrad))]
        }
        MenuId::Clr => vec![("Setup", Clear(1)), ("Memory", Clear(2)), ("All", Clear(3))],
        MenuId::Stat => {
            let mut items = vec![
                ("Type", Open(MenuId::StatType)),
                ("Data", StatEditor),
                ("Sum", Open(MenuId::StatSum)),
                ("Var", Open(MenuId::StatVar)),
            ];
            if paired {
                items.push(("Reg", Open(MenuId::StatReg)));
            }
            items.push(("MinMax", Open(MenuId::StatMinMax)));
            items
        }
        MenuId::StatSum if paired => {
            stat_items(&[StatVar::SumX2, StatVar::SumX, StatVar::SumY2, StatVar::SumY, StatVar::SumXY])
        }
        MenuId::StatSum => stat_items(&[StatVar::SumX2, StatVar::SumX]),
        MenuId::StatVar if paired => stat_items(&[
            StatVar::N,
            StatVar::MeanX,
            StatVar::PopSdX,
            StatVar::SampleSdX,
            StatVar::MeanY,
            StatVar::PopSdY,
            StatVar::SampleSdY,
        ]),
        MenuId::StatVar => stat_items(&[StatVar::N, StatVar::MeanX, StatVar::PopSdX, StatVar::SampleSdX]),
        MenuId::StatReg => {
            let mut items = stat_items(&[StatVar::RegA, StatVar::RegB, StatVar::RegR]);
            items.push(("x̂", Insert(Token::EstX)));
            items.push(("ŷ", Insert(Token::EstY)));
            items
        }
        MenuId::StatMinMax if paired => stat_items(&[StatVar::MinX, StatVar::MaxX, StatVar::MinY, StatVar::MaxY]),
        MenuId::StatMinMax => stat_items(&[StatVar::MinX, StatVar::MaxX]),
        MenuId::Fix | MenuId::Sci | MenuId::Norm => Vec::new(),
    }
}

fn prompt(id: MenuId) -> Option<&'static str> {
    match id {
        MenuId::Fix => Some("Fix 0~9?"),
        MenuId::Sci => Some("Sci 0~9?"),
        MenuId::Norm => Some("Norm 1~2?"),
        _ => None,
    }
}

fn title(id: MenuId) -> Option<&'static str> {
    match id {
        MenuId::StatFreq => Some("Frequency?"),
        MenuId::CmplxFormat => Some("Complex Result?"),
        MenuId::MatSelect { .. } => Some("Matrix?"),
        MenuId::MatDim { .. } => Some("m×n?"),
        MenuId::Clr => Some("Clear?"),
        MenuId::Drg => Some("DRG▸"),
        _ => None,
    }
}

/// The action for pressing `digit` in a menu, if that digit is a choice.
pub(crate) fn select(id: MenuId, digit: u8, kind: StatKind) -> Option<MenuAction> {
    match id {
        MenuId::Fix => Some(MenuAction::Number(NumberFormat::Fix(digit))),
        // Sci 0 means 10 significant digits.
        MenuId::Sci => Some(MenuAction::Number(NumberFormat::Sci(if digit == 0 { 10 } else { digit }))),
        MenuId::Norm => match digit {
            1 => Some(MenuAction::Number(NumberFormat::Norm1)),
            2 => Some(MenuAction::Number(NumberFormat::Norm2)),
            _ => None,
        },
        _ => {
            let index = usize::from(digit).checked_sub(1)?;
            entries(id, kind).get(index).map(|&(_, action)| action)
        }
    }
}

/// The other page of a two-page menu, for ▲/▼.
pub(crate) fn page(id: MenuId, down: bool) -> Option<MenuId> {
    match (id, down) {
        (MenuId::Setup, true) => Some(MenuId::Setup2),
        (MenuId::Setup2, false) => Some(MenuId::Setup),
        (MenuId::MatDim { id, more: false }, true) => Some(MenuId::MatDim { id, more: true }),
        (MenuId::MatDim { id, more: true }, false) => Some(MenuId::MatDim { id, more: false }),
        _ => None,
    }
}

pub(crate) fn view(id: MenuId, kind: StatKind) -> MenuView {
    let items = entries(id, kind).iter().enumerate().map(|(i, (label, _))| format!("{}:{label}", i + 1)).collect();
    MenuView {
        title: title(id),
        items,
        prompt: prompt(id),
        more_below: page(id, true).is_some(),
        more_above: page(id, false).is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_menus_depend_on_the_data_type() {
        assert_eq!(view(MenuId::Stat, StatKind::OneVar).items.len(), 5);
        assert_eq!(view(MenuId::Stat, StatKind::Linear).items[4], "5:Reg");
        assert_eq!(view(MenuId::StatVar, StatKind::Linear).items[4], "5:ȳ");
        assert_eq!(select(MenuId::StatVar, 2, StatKind::OneVar), Some(MenuAction::Insert(Token::Stat(StatVar::MeanX))));
        assert_eq!(select(MenuId::StatVar, 5, StatKind::OneVar), None);
    }

    #[test]
    fn digit_prompts() {
        assert_eq!(select(MenuId::Fix, 0, StatKind::OneVar), Some(MenuAction::Number(NumberFormat::Fix(0))));
        assert_eq!(select(MenuId::Sci, 0, StatKind::OneVar), Some(MenuAction::Number(NumberFormat::Sci(10))));
        assert_eq!(select(MenuId::Norm, 3, StatKind::OneVar), None);
        assert!(view(MenuId::Fix, StatKind::OneVar).prompt.is_some());
    }

    #[test]
    fn setup_has_two_pages() {
        let first = view(MenuId::Setup, StatKind::OneVar);
        assert!(first.more_below && !first.more_above);
        assert_eq!(page(MenuId::Setup, true), Some(MenuId::Setup2));
        assert_eq!(select(MenuId::Setup2, 1, StatKind::OneVar), Some(MenuAction::Fraction(FractionStyle::Mixed)));
        assert_eq!(select(MenuId::Setup, 0, StatKind::OneVar), None);
    }
}
