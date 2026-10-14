//! Input tree → expression tree, with the device's priority rules.
//!
//! From tightest to loosest binding:
//! 1. numbers, constants, `( )`, functions with parentheses, templates
//! 2. postfix: `² ³ ⁻¹ ! % ° ʳ ᵍ`, `x^■`, and LineIO `^(` and `ˣ√(`
//! 3. LineIO fractions: `a⌟b`, `a⌟b⌟c`
//! 4. prefix signs: `(−)`, and `−`/`+` where an operand is expected
//! 5. regression estimates `x̂`, `ŷ`
//! 6. multiplication with the sign omitted (`2π`, `3sin(30)`, `2(1+1)`, `3i`)
//! 7. `∠` between a modulus and an angle (CMPLX mode)
//! 8. nPr, nCr
//! 9. ×, ÷
//! 10. +, −
//!
//! Because 6 binds tighter than 8, `1÷2π` means `1÷(2π)`, as on school
//! calculators. Missing closing parentheses are added at the end of a row.

use crate::ast::{BinaryOp, Expr, ExprKind, LocatedError, UnaryOp};
use crate::error::CalcError;
use crate::input::{Cursor, Func, Item, Row, SlotRef, Template, TemplateKind, Token};
use crate::value::{Rational, Value};

/// Deepest nesting of parentheses, functions and templates before a Stack ERROR.
pub const MAX_NESTING: usize = 24;

/// Parses a whole input line. `Pol(` and `Rec(` are only allowed as the
/// entire expression.
pub fn parse(row: &Row) -> Result<Expr, LocatedError> {
    let expr = Parser::new(&row.items, Vec::new(), 0).parse_all()?;
    check_statements(&expr, true)?;
    Ok(expr)
}

/// How a CMPLX result should be shown, from a trailing ▸r∠θ or ▸a+bi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComplexForm {
    Rect,
    Polar,
}

/// Parses a calculation line, which may end in ▸r∠θ or ▸a+bi to choose how
/// a complex result is shown. Anywhere else those are a Syntax ERROR.
pub fn parse_statement(row: &Row) -> Result<(Expr, Option<ComplexForm>), LocatedError> {
    let form = match row.items.last() {
        Some(Item::Token(Token::ToPolar)) => Some(ComplexForm::Polar),
        Some(Item::Token(Token::ToRect)) => Some(ComplexForm::Rect),
        _ => None,
    };
    let items = if form.is_some() { &row.items[..row.items.len() - 1] } else { &row.items[..] };
    let expr = Parser::new(items, Vec::new(), 0).parse_all()?;
    check_statements(&expr, true)?;
    Ok((expr, form))
}

fn check_statements(expr: &Expr, top: bool) -> Result<(), LocatedError> {
    match &expr.kind {
        ExprKind::Call(Func::Pol | Func::Rec, _) if !top => {
            Err(LocatedError { error: CalcError::Syntax, at: expr.at.clone() })
        }
        ExprKind::Call(_, args) => args.iter().try_for_each(|a| check_statements(a, false)),
        ExprKind::Unary(_, a) => check_statements(a, false),
        ExprKind::Binary(_, a, b) => check_statements(a, false).and_then(|()| check_statements(b, false)),
        _ => Ok(()),
    }
}

struct Parser<'a> {
    items: &'a [Item],
    pos: usize,
    path: Vec<SlotRef>,
    depth: usize,
}

type PResult<T> = Result<T, LocatedError>;

impl<'a> Parser<'a> {
    fn new(items: &'a [Item], path: Vec<SlotRef>, depth: usize) -> Self {
        Self { items, pos: 0, path, depth }
    }

    fn here(&self) -> Cursor {
        Cursor { path: self.path.clone(), pos: self.pos }
    }

    fn error(&self, error: CalcError) -> LocatedError {
        LocatedError { error, at: self.here() }
    }

    fn peek_token(&self) -> Option<Token> {
        match self.items.get(self.pos)? {
            Item::Token(t) => Some(*t),
            Item::Template(_) => None,
        }
    }

    fn node(&self, kind: ExprKind, at: Cursor) -> Expr {
        Expr { kind, at }
    }

    fn nest(&mut self) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(self.error(CalcError::Stack));
        }
        Ok(())
    }

    fn parse_all(mut self) -> PResult<Expr> {
        let expr = self.additive()?;
        if self.pos < self.items.len() {
            return Err(self.error(CalcError::Syntax));
        }
        Ok(expr)
    }

    fn binary_loop(
        &mut self,
        operand: fn(&mut Self) -> PResult<Expr>,
        operator: fn(Token) -> Option<BinaryOp>,
    ) -> PResult<Expr> {
        let mut lhs = operand(self)?;
        while let Some(op) = self.peek_token().and_then(operator) {
            let at = self.here();
            self.pos += 1;
            let rhs = operand(self)?;
            lhs = self.node(ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), at);
        }
        Ok(lhs)
    }

    fn additive(&mut self) -> PResult<Expr> {
        self.binary_loop(Self::multiplicative, |t| match t {
            Token::Plus => Some(BinaryOp::Add),
            Token::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn multiplicative(&mut self) -> PResult<Expr> {
        self.binary_loop(Self::permutation, |t| match t {
            Token::Times => Some(BinaryOp::Mul),
            Token::Divide => Some(BinaryOp::Div),
            _ => None,
        })
    }

    fn permutation(&mut self) -> PResult<Expr> {
        self.binary_loop(Self::polar, |t| match t {
            Token::Perm => Some(BinaryOp::Perm),
            Token::Comb => Some(BinaryOp::Comb),
            _ => None,
        })
    }

    /// `r∠θ`: binds looser than implied multiplication, so `√2∠45` and `2∠−30` work.
    fn polar(&mut self) -> PResult<Expr> {
        self.binary_loop(Self::implicit, |t| match t {
            Token::Angle => Some(BinaryOp::Polar),
            _ => None,
        })
    }

    fn starts_operand(&self) -> bool {
        match self.items.get(self.pos) {
            Some(Item::Token(t)) => matches!(
                t,
                Token::Digit(_)
                    | Token::Point
                    | Token::Exp10
                    | Token::LParen
                    | Token::Func(_)
                    | Token::Pi
                    | Token::Euler
                    | Token::Ans
                    | Token::Ran
                    | Token::Var(_)
                    | Token::Stat(_)
                    | Token::Imag
            ),
            Some(Item::Template(t)) => t.kind != TemplateKind::Power,
            None => false,
        }
    }

    fn implicit(&mut self) -> PResult<Expr> {
        let mut lhs = self.estimated()?;
        while self.starts_operand() {
            let at = self.here();
            let rhs = self.estimated()?;
            lhs = self.node(ExprKind::Binary(BinaryOp::Mul, Box::new(lhs), Box::new(rhs)), at);
        }
        Ok(lhs)
    }

    /// `x̂` and `ŷ` bind looser than signs: `−2ŷ` is the estimate for −2.
    fn estimated(&mut self) -> PResult<Expr> {
        let mut expr = self.signed()?;
        loop {
            let op = match self.peek_token() {
                Some(Token::EstX) => UnaryOp::EstimateX,
                Some(Token::EstY) => UnaryOp::EstimateY,
                _ => return Ok(expr),
            };
            let at = self.here();
            self.pos += 1;
            expr = self.node(ExprKind::Unary(op, Box::new(expr)), at);
        }
    }

    fn signed(&mut self) -> PResult<Expr> {
        match self.peek_token() {
            Some(Token::Neg | Token::Minus) => {
                let at = self.here();
                self.pos += 1;
                let operand = self.signed()?;
                Ok(self.node(ExprKind::Unary(UnaryOp::Neg, Box::new(operand)), at))
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.signed()
            }
            _ => self.fraction(),
        }
    }

    /// LineIO fractions: `a⌟b` is a/b and `a⌟b⌟c` is a + b/c.
    fn fraction(&mut self) -> PResult<Expr> {
        let first = self.postfix()?;
        if self.peek_token() != Some(Token::FracBar) {
            return Ok(first);
        }
        let at = self.here();
        self.pos += 1;
        let second = self.postfix()?;
        if self.peek_token() != Some(Token::FracBar) {
            return Ok(self.node(ExprKind::Binary(BinaryOp::Div, Box::new(first), Box::new(second)), at));
        }
        self.pos += 1;
        let third = self.postfix()?;
        let part = self.node(ExprKind::Binary(BinaryOp::Div, Box::new(second), Box::new(third)), at.clone());
        Ok(self.node(ExprKind::Binary(BinaryOp::Add, Box::new(first), Box::new(part)), at))
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.primary()?;
        loop {
            let at = self.here();
            let op = match self.items.get(self.pos) {
                Some(Item::Template(t)) if t.kind == TemplateKind::Power => {
                    let exponent = self.slot(t, 0)?;
                    self.pos += 1;
                    expr = self.node(ExprKind::Binary(BinaryOp::Pow, Box::new(expr), Box::new(exponent)), at);
                    continue;
                }
                Some(Item::Token(t @ (Token::PowOpen | Token::RootOpen))) => {
                    // LineIO `x^(n)` and `n ˣ√(x)`: the parenthesized part follows.
                    let op = if *t == Token::PowOpen { BinaryOp::Pow } else { BinaryOp::Root };
                    self.pos += 1;
                    self.nest()?;
                    let operand = self.additive()?;
                    self.close_paren()?;
                    self.depth -= 1;
                    expr = self.node(ExprKind::Binary(op, Box::new(expr), Box::new(operand)), at);
                    continue;
                }
                Some(Item::Token(t)) => match t {
                    Token::Square => UnaryOp::Square,
                    Token::Cube => UnaryOp::Cube,
                    Token::Recip => UnaryOp::Recip,
                    Token::Factorial => UnaryOp::Factorial,
                    Token::Percent => UnaryOp::Percent,
                    Token::AngleDeg => UnaryOp::FromDeg,
                    Token::AngleRad => UnaryOp::FromRad,
                    Token::AngleGrad => UnaryOp::FromGrad,
                    _ => break,
                },
                _ => break,
            };
            self.pos += 1;
            expr = self.node(ExprKind::Unary(op, Box::new(expr)), at);
        }
        Ok(expr)
    }

    fn primary(&mut self) -> PResult<Expr> {
        let at = self.here();
        let token = match self.items.get(self.pos) {
            None => return Err(self.error(CalcError::Syntax)),
            Some(Item::Template(t)) if t.kind != TemplateKind::Power => return self.template(t),
            Some(Item::Template(_)) => return Err(self.error(CalcError::Syntax)),
            Some(Item::Token(t)) => *t,
        };
        let simple = match token {
            Token::Digit(_) | Token::Point | Token::Exp10 => return self.number(),
            Token::LParen => {
                self.pos += 1;
                self.nest()?;
                let inner = self.additive()?;
                self.close_paren()?;
                self.depth -= 1;
                return Ok(inner);
            }
            Token::Func(f) => return self.call(f),
            Token::Pi => ExprKind::Pi,
            Token::Euler => ExprKind::Euler,
            Token::Ans => ExprKind::Ans,
            Token::Ran => ExprKind::Ran,
            Token::Var(v) => ExprKind::Var(v),
            Token::Stat(v) => ExprKind::Stat(v),
            Token::Imag => ExprKind::Imag,
            _ => return Err(self.error(CalcError::Syntax)),
        };
        self.pos += 1;
        Ok(self.node(simple, at))
    }

    /// Accepts `)` or the end of the row (an implied closing parenthesis).
    fn close_paren(&mut self) -> PResult<()> {
        match self.items.get(self.pos) {
            Some(Item::Token(Token::RParen)) => {
                self.pos += 1;
                Ok(())
            }
            None => Ok(()),
            Some(_) => Err(self.error(CalcError::Syntax)),
        }
    }

    fn call(&mut self, func: Func) -> PResult<Expr> {
        let at = self.here();
        self.pos += 1;
        self.nest()?;
        let mut args = vec![self.additive()?];
        while self.peek_token() == Some(Token::Comma) {
            self.pos += 1;
            args.push(self.additive()?);
        }
        let arity_ok = match func {
            Func::Pol | Func::Rec | Func::RanInt => args.len() == 2,
            Func::Log => args.len() <= 2,
            _ => args.len() == 1,
        };
        if !arity_ok {
            return Err(LocatedError { error: CalcError::Syntax, at });
        }
        self.close_paren()?;
        self.depth -= 1;
        Ok(self.node(ExprKind::Call(func, args), at))
    }

    /// Parses one slot of the template at the current position as a complete expression.
    fn slot(&self, template: &Template, slot: usize) -> PResult<Expr> {
        let mut path = self.path.clone();
        path.push(SlotRef { item: self.pos, slot });
        let mut sub = Parser::new(&template.slots[slot].items, path, self.depth);
        sub.nest()?;
        sub.parse_all()
    }

    fn template(&mut self, t: &Template) -> PResult<Expr> {
        let at = self.here();
        let slots = (0..t.slots.len()).map(|i| self.slot(t, i)).collect::<PResult<Vec<_>>>()?;
        self.pos += 1;
        let mut slots = slots.into_iter();
        let mut next = || Box::new(slots.next().expect("slot count matches template kind"));
        let kind = match t.kind {
            TemplateKind::Fraction => ExprKind::Binary(BinaryOp::Div, next(), next()),
            TemplateKind::MixedFraction => {
                let whole = next();
                let fraction = Expr { kind: ExprKind::Binary(BinaryOp::Div, next(), next()), at: at.clone() };
                ExprKind::Binary(BinaryOp::Add, whole, Box::new(fraction))
            }
            TemplateKind::Pow10 => {
                let ten = Expr { kind: ExprKind::Number(Value::integer(10)), at: at.clone() };
                ExprKind::Binary(BinaryOp::Pow, Box::new(ten), next())
            }
            TemplateKind::ExpE => ExprKind::Unary(UnaryOp::Exp, next()),
            TemplateKind::Sqrt => ExprKind::Unary(UnaryOp::Sqrt, next()),
            TemplateKind::CubeRoot => ExprKind::Unary(UnaryOp::Cbrt, next()),
            TemplateKind::Abs => ExprKind::Unary(UnaryOp::Abs, next()),
            TemplateKind::Root => ExprKind::Binary(BinaryOp::Root, next(), next()),
            TemplateKind::LogBase => ExprKind::Binary(BinaryOp::LogBase, next(), next()),
            TemplateKind::Power => unreachable!("x^■ is parsed as a postfix operator"),
        };
        Ok(self.node(kind, at))
    }

    /// Digits with an optional `.`, then an optional `E±n` exponent. A
    /// degrees-minutes-seconds literal such as `1°30°15°` continues from here.
    fn number(&mut self) -> PResult<Expr> {
        let at = self.here();
        let mut value = match self.digits(true) {
            Some(text) => Value::parse_literal(&text).map_err(|e| LocatedError { error: e, at: at.clone() })?,
            None => Value::integer(1), // a bare ×10^n means 1×10^n
        };
        if self.peek_token() == Some(Token::Exp10) {
            self.pos += 1;
            let negative = matches!(self.peek_token(), Some(Token::Neg | Token::Minus));
            if matches!(self.peek_token(), Some(Token::Neg | Token::Minus | Token::Plus)) {
                self.pos += 1;
            }
            let exponent: i64 =
                self.digits(false).and_then(|d| d.parse().ok()).ok_or_else(|| self.error(CalcError::Syntax))?;
            let scale = Value::integer(10).powi(if negative { -exponent } else { exponent });
            value = scale.and_then(|s| value.mul(&s)).map_err(|e| LocatedError { error: e, at: at.clone() })?;
        } else if self.peek_token() == Some(Token::Dms) {
            value = self.dms(value)?;
        }
        Ok(self.node(ExprKind::Number(value), at))
    }

    /// Consumes a run of digits (and points, if allowed) as text.
    fn digits(&mut self, allow_point: bool) -> Option<String> {
        let mut text = String::new();
        while let Some(t) = self.peek_token() {
            match t {
                Token::Digit(d) => text.push(char::from(b'0' + d)),
                Token::Point if allow_point => text.push('.'),
                _ => break,
            }
            self.pos += 1;
        }
        (!text.is_empty()).then_some(text)
    }

    /// `degrees° [minutes°] [seconds°]`, already past the degrees.
    fn dms(&mut self, degrees: Value) -> PResult<Value> {
        let mut total = degrees;
        for divisor in [60, 3600] {
            if self.peek_token() != Some(Token::Dms) {
                break;
            }
            self.pos += 1;
            let start = self.pos;
            let Some(text) = self.digits(true) else { break };
            if self.peek_token() != Some(Token::Dms) {
                self.pos = start; // not a DMS part; leave it for the caller
                break;
            }
            let part = Value::parse_literal(&text).map_err(|e| self.error(e))?;
            let scaled = part.div(&Value::rational(Rational::integer(divisor)));
            total = scaled.and_then(|s| total.add(&s)).map_err(|e| self.error(e))?;
        }
        if self.peek_token() == Some(Token::Dms) {
            self.pos += 1;
        }
        Ok(total)
    }
}
