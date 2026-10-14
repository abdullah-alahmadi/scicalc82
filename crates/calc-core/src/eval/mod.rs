//! Evaluating parsed expressions.

pub mod trig;

use std::f64::consts::{E, TAU};

use crate::ast::{BinaryOp, Expr, ExprKind, LocatedError, UnaryOp};
use crate::error::{CalcError, CalcResult};
use crate::format::{NumberFormat, format_decimal};
use crate::input::Func;
use crate::number::Number;
use crate::state::{AngleUnit, Memory, Rng, Settings};
use crate::stats::StatData;
use crate::value::{Rational, Value};

/// Largest argument of x!, since 70! ≥ 10¹⁰⁰.
const MAX_FACTORIAL: i128 = 69;
/// nPr and nCr need 0 ≤ r ≤ n < 10¹⁰.
const MAX_PERM_N: i128 = 10_000_000_000;

pub struct Env<'a> {
    pub settings: &'a Settings,
    pub memory: &'a Memory,
    pub rng: &'a mut Rng,
    /// STAT data, available in STAT mode only.
    pub stats: Option<&'a StatData>,
    /// CMPLX mode: i and ∠ are allowed, and √ of a negative number is imaginary.
    pub complex: bool,
}

impl Env<'_> {
    fn stats(&self) -> CalcResult<&StatData> {
        self.stats.ok_or(CalcError::Math)
    }
}

/// What a calculation produces. Pol( and Rec( give two values.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Single(Number),
    Polar { r: Value, theta: Value },
    Rect { x: Value, y: Value },
}

impl Outcome {
    /// The value that becomes Ans.
    pub fn primary(&self) -> Number {
        match self {
            Self::Single(n) => n.clone(),
            Self::Polar { r: v, .. } | Self::Rect { x: v, .. } => Number::Real(v.clone()),
        }
    }
}

pub fn evaluate(expr: &Expr, env: &mut Env<'_>) -> Result<Outcome, LocatedError> {
    let locate = |error| LocatedError { error, at: expr.at.clone() };
    match &expr.kind {
        ExprKind::Call(func @ (Func::Pol | Func::Rec), args) => {
            let a = eval(&args[0], env)?;
            let b = eval(&args[1], env)?;
            let (a, b) = (a.real().map_err(locate)?, b.real().map_err(locate)?);
            let unit = env.settings.angle;
            if *func == Func::Pol {
                pol(a, b, unit).map(|(r, theta)| Outcome::Polar { r, theta }).map_err(locate)
            } else {
                rec(a, b, unit).map(|(x, y)| Outcome::Rect { x, y }).map_err(locate)
            }
        }
        _ => eval(expr, env).map(Outcome::Single),
    }
}

fn eval(expr: &Expr, env: &mut Env<'_>) -> Result<Number, LocatedError> {
    let result = match &expr.kind {
        ExprKind::Number(v) => Ok(Number::Real(v.clone())),
        ExprKind::Pi => Ok(Number::Real(Value::pi())),
        ExprKind::Euler => Value::decimal(E).map(Number::Real),
        ExprKind::Imag if env.complex => Ok(Number::i()),
        ExprKind::Imag => Err(CalcError::Syntax),
        ExprKind::Var(v) => Ok(env.memory.get(*v).clone()),
        ExprKind::Ans => Ok(env.memory.ans.clone()),
        // Ran# gives 0.000–0.999 in steps of 0.001.
        ExprKind::Ran => {
            Ok(Number::Real(Value::rational(Rational::new(env.rng.below(1000).into(), 1000).expect("valid"))))
        }
        ExprKind::Stat(var) => env.stats().and_then(|s| s.value(*var)).map(Number::Real),
        ExprKind::Unary(op @ (UnaryOp::EstimateX | UnaryOp::EstimateY), a) => {
            let a = eval(a, env)?;
            a.real()
                .and_then(|a| {
                    let stats = env.stats()?;
                    if *op == UnaryOp::EstimateX { stats.estimate_x(a) } else { stats.estimate_y(a) }
                })
                .map(Number::Real)
        }
        ExprKind::Unary(op, a) => {
            let a = eval(a, env)?;
            unary(*op, &a, env)
        }
        ExprKind::Binary(op, a, b) => {
            let a = eval(a, env)?;
            let b = eval(b, env)?;
            binary(*op, &a, &b, env)
        }
        ExprKind::Call(func, args) => {
            let args = args.iter().map(|a| eval(a, env)).collect::<Result<Vec<_>, _>>()?;
            call(*func, &args, env)
        }
    };
    result.map_err(|error| LocatedError { error, at: expr.at.clone() })
}

/// Operators that accept complex numbers; the rest need real operands.
fn unary(op: UnaryOp, a: &Number, env: &Env<'_>) -> CalcResult<Number> {
    match op {
        UnaryOp::Neg => a.neg(),
        UnaryOp::Square => a.powi(2),
        UnaryOp::Cube => a.powi(3),
        UnaryOp::Recip => a.recip(),
        UnaryOp::Abs => a.abs().map(Number::Real),
        UnaryOp::Sqrt => a.sqrt(env.complex),
        _ => real_unary(op, a.real()?, env.settings.angle).map(Number::Real),
    }
}

fn real_unary(op: UnaryOp, a: &Value, unit: AngleUnit) -> CalcResult<Value> {
    match op {
        UnaryOp::Factorial => factorial(a),
        UnaryOp::Percent => a.div(&Value::integer(100)),
        UnaryOp::FromDeg => trig::convert(a, AngleUnit::Deg, unit),
        UnaryOp::FromRad => trig::convert(a, AngleUnit::Rad, unit),
        UnaryOp::FromGrad => trig::convert(a, AngleUnit::Gra, unit),
        UnaryOp::Cbrt => a.nth_root(3),
        UnaryOp::Exp => {
            if a.is_zero() {
                Ok(Value::integer(1))
            } else {
                Value::decimal(a.to_f64().exp())
            }
        }
        _ => unreachable!("handled in `unary` or `eval`"),
    }
}

fn binary(op: BinaryOp, a: &Number, b: &Number, env: &Env<'_>) -> CalcResult<Number> {
    match op {
        BinaryOp::Add => a.add(b),
        BinaryOp::Sub => a.sub(b),
        BinaryOp::Mul => a.mul(b),
        BinaryOp::Div => a.div(b),
        // Complex numbers can be raised to integer powers only, as on the device.
        BinaryOp::Pow if a.is_complex() || b.is_complex() => {
            let n = b.real().ok().and_then(integer).and_then(|n| i64::try_from(n).ok()).ok_or(CalcError::Math)?;
            a.powi(n)
        }
        BinaryOp::Polar if env.complex => Number::from_polar(a.real()?, b.real()?, env.settings.angle),
        BinaryOp::Polar => Err(CalcError::Syntax),
        _ => real_binary(op, a.real()?, b.real()?).map(Number::Real),
    }
}

fn real_binary(op: BinaryOp, a: &Value, b: &Value) -> CalcResult<Value> {
    match op {
        BinaryOp::Pow => a.pow(b),
        BinaryOp::Root => b.pow(&a.recip()?),
        BinaryOp::LogBase => log_base(a, b),
        BinaryOp::Perm => perm_comb(a, b, false),
        BinaryOp::Comb => perm_comb(a, b, true),
        _ => unreachable!("handled in `binary`"),
    }
}

fn call(func: Func, args: &[Number], env: &mut Env<'_>) -> CalcResult<Number> {
    let z = &args[0];
    match func {
        Func::Arg => z.arg(env.settings.angle).map(Number::Real),
        Func::Conjg => z.conj(),
        Func::Abs => z.abs().map(Number::Real),
        Func::Sqrt => z.sqrt(env.complex),
        _ => {
            let reals = args.iter().map(|a| a.real().cloned()).collect::<CalcResult<Vec<_>>>()?;
            real_call(func, &reals, env).map(Number::Real)
        }
    }
}

fn real_call(func: Func, args: &[Value], env: &mut Env<'_>) -> CalcResult<Value> {
    let unit = env.settings.angle;
    let x = &args[0];
    let decimal = |f: fn(f64) -> f64| Value::decimal(f(x.to_f64()));
    match func {
        Func::Sin => trig::sin(x, unit),
        Func::Cos => trig::cos(x, unit),
        Func::Tan => trig::tan(x, unit),
        Func::Asin => trig::asin(x, unit),
        Func::Acos => trig::acos(x, unit),
        Func::Atan => trig::atan(x, unit),
        Func::Sinh => decimal(f64::sinh),
        Func::Cosh => decimal(f64::cosh),
        Func::Tanh => decimal(f64::tanh),
        Func::Asinh => decimal(f64::asinh),
        Func::Acosh if x.to_f64() < 1.0 => Err(CalcError::Math),
        Func::Acosh => decimal(f64::acosh),
        Func::Atanh if x.to_f64().abs() >= 1.0 => Err(CalcError::Math),
        Func::Atanh => decimal(f64::atanh),
        Func::Log if args.len() == 2 => log_base(&args[0], &args[1]),
        Func::Log => log_base(&Value::integer(10), x),
        Func::Ln if x.signum() <= 0 => Err(CalcError::Math),
        Func::Ln if x.as_rational() == Some(Rational::ONE) => Ok(Value::zero()),
        Func::Ln => decimal(f64::ln),
        Func::RanInt => ran_int(&args[0], &args[1], env.rng),
        Func::Rnd => rnd(x, env.settings.format.number),
        Func::Cbrt => x.nth_root(3),
        // Only valid as a whole statement; see `evaluate`.
        Func::Pol | Func::Rec => Err(CalcError::Syntax),
        Func::Arg | Func::Conjg | Func::Abs | Func::Sqrt => unreachable!("handled in `call`"),
    }
}

fn integer(v: &Value) -> Option<i128> {
    v.as_rational().filter(|r| r.is_integer()).map(Rational::numer)
}

fn factorial(a: &Value) -> CalcResult<Value> {
    let n = integer(a).filter(|n| (0..=MAX_FACTORIAL).contains(n)).ok_or(CalcError::Math)?;
    (2..=n as i64).try_fold(Value::integer(1), |acc, i| acc.mul(&Value::integer(i)))
}

fn perm_comb(n: &Value, r: &Value, comb: bool) -> CalcResult<Value> {
    let (n, r) = match (integer(n), integer(r)) {
        (Some(n), Some(r)) if 0 <= r && r <= n && n < MAX_PERM_N => (n as i64, r as i64),
        _ => return Err(CalcError::Math),
    };
    if comb {
        // Multiply and divide alternately so each step stays an integer.
        let k = r.min(n - r);
        (1..=k).try_fold(Value::integer(1), |acc, i| acc.mul(&Value::integer(n - k + i))?.div(&Value::integer(i)))
    } else {
        (n - r + 1..=n).try_fold(Value::integer(1), |acc, i| acc.mul(&Value::integer(i)))
    }
}

/// `log_base(x)`. Exact when `x` is an integer power of `base`, e.g. log₂ 8 = 3.
fn log_base(base: &Value, x: &Value) -> CalcResult<Value> {
    if base.signum() <= 0 || x.signum() <= 0 || base.as_rational() == Some(Rational::ONE) {
        return Err(CalcError::Math);
    }
    let ratio = x.to_f64().ln() / base.to_f64().ln();
    if !ratio.is_finite() {
        return Err(CalcError::Math);
    }
    let k = ratio.round();
    if base.is_exact() && x.is_exact() && k.abs() <= 1000.0 && base.powi(k as i64).as_ref() == Ok(x) {
        return Ok(Value::integer(k as i64));
    }
    Value::decimal(ratio)
}

fn ran_int(a: &Value, b: &Value, rng: &mut Rng) -> CalcResult<Value> {
    match (integer(a).and_then(|a| i64::try_from(a).ok()), integer(b).and_then(|b| i64::try_from(b).ok())) {
        (Some(a), Some(b)) if a <= b => {
            let span = (b - a) as u64 + 1;
            Ok(Value::integer(a + rng.below(span) as i64))
        }
        _ => Err(CalcError::Math),
    }
}

/// Rnd(: rounds to what the current display format shows.
fn rnd(x: &Value, number: NumberFormat) -> CalcResult<Value> {
    if integer(x).is_some() {
        return Ok(x.clone());
    }
    let shown = format_decimal(x.to_f64(), number);
    let mantissa: f64 = shown.mantissa.parse().map_err(|_| CalcError::Math)?;
    let sign = if shown.negative { -1.0 } else { 1.0 };
    Value::decimal(sign * mantissa * 10f64.powi(shown.exponent.unwrap_or(0)))
}

/// Pol(x, y) → (r, θ), with θ in (−180°, 180°].
pub(crate) fn pol(x: &Value, y: &Value, unit: AngleUnit) -> CalcResult<(Value, Value)> {
    let r = x.powi(2)?.add(&y.powi(2)?)?.sqrt()?;
    let exact_turns = match (x.signum(), y.signum()) {
        (0, 0) => Some(Rational::ZERO),
        (0, s) => Rational::new(s.into(), 4),
        (sx, sy) => y.div(x).ok().and_then(|ratio| trig::exact_atan_turns(&ratio)).and_then(|t| {
            // atan covers the right half-plane; shift by half a turn on the left.
            match (sx, sy) {
                (1, _) => Some(t),
                (_, s) if s >= 0 => t.checked_add(Rational::new(1, 2)?),
                _ => t.checked_sub(Rational::new(1, 2)?),
            }
        }),
    };
    let turns = match exact_turns {
        Some(t) => Value::rational(t),
        None => Value::decimal(y.to_f64().atan2(x.to_f64()) / TAU)?,
    };
    Ok((r, trig::from_turns(&turns, unit)?))
}

/// Rec(r, θ) → (x, y).
pub(crate) fn rec(r: &Value, theta: &Value, unit: AngleUnit) -> CalcResult<(Value, Value)> {
    Ok((r.mul(&trig::cos(theta, unit)?)?, r.mul(&trig::sin(theta, unit)?)?))
}
