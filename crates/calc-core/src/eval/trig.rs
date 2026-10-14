//! Trigonometry with exact results at multiples of 30° and 45°.
//!
//! Angles are converted to fractions of a full turn, so `sin 30°`,
//! `sin(π/6)` and `sin(100/3 grad)` all hit the same table entry and give 1/2.

use std::f64::consts::TAU;

use crate::error::{CalcError, CalcResult};
use crate::state::AngleUnit;
use crate::value::{Rational, Value};

/// Largest |angle| in turns the trig functions accept (the device's limit is
/// |x| < 9×10⁹ degrees).
const MAX_TURNS: f64 = 2.5e7;
/// Decimal sine and cosine results this small are taken to be zero; the
/// inputs only carry 15 significant digits.
const SNAP_TO_ZERO: f64 = 1e-14;

fn full_turn(unit: AngleUnit) -> Value {
    match unit {
        AngleUnit::Deg => Value::integer(360),
        AngleUnit::Gra => Value::integer(400),
        AngleUnit::Rad => Value::Pi(Rational::integer(2)),
    }
}

pub fn to_turns(angle: &Value, unit: AngleUnit) -> CalcResult<Value> {
    angle.div(&full_turn(unit))
}

pub fn from_turns(turns: &Value, unit: AngleUnit) -> CalcResult<Value> {
    turns.mul(&full_turn(unit))
}

/// Converts an angle between units (the DRG▸ markers).
pub fn convert(angle: &Value, from: AngleUnit, to: AngleUnit) -> CalcResult<Value> {
    from_turns(&to_turns(angle, from)?, to)
}

fn rat(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid constant")
}

fn half_sqrt(n: i64) -> Value {
    Value::integer(n).sqrt().and_then(|r| r.div(&Value::integer(2))).expect("small constant")
}

/// sin of `t` turns, if `t` is a multiple of 1/12 or 1/8 turn.
fn exact_sin(t: Rational) -> Option<Value> {
    let t = Rational::new(t.numer().rem_euclid(t.denom()), t.denom())?;
    for steps in [12, 8] {
        let k = t.checked_mul(Rational::integer(steps))?;
        if !k.is_integer() {
            continue;
        }
        let k = k.numer() as i64;
        let half = steps / 2;
        let magnitude = match (steps, k % half) {
            (_, 0) => Value::zero(),
            (12, 1 | 5) => Value::rational(rat(1, 2)),
            (12, 2 | 4) => half_sqrt(3),
            (8, 1 | 3) => half_sqrt(2),
            _ => Value::integer(1),
        };
        return if k >= half { magnitude.neg().ok() } else { Some(magnitude) };
    }
    None
}

fn exact_cos(t: Rational) -> Option<Value> {
    exact_sin(t.checked_add(rat(1, 4))?)
}

fn exact_tan(t: Rational) -> Option<CalcResult<Value>> {
    let (s, c) = (exact_sin(t)?, exact_cos(t)?);
    Some(if c.is_zero() { Err(CalcError::Math) } else { s.div(&c) })
}

/// The angle as turns, rejecting arguments beyond the device's range.
fn turns_in_range(angle: &Value, unit: AngleUnit) -> CalcResult<Value> {
    let turns = to_turns(angle, unit)?;
    if turns.to_f64().abs() >= MAX_TURNS {
        return Err(CalcError::Math);
    }
    Ok(turns)
}

/// The angle in radians, reduced to one turn for accuracy.
fn reduced_radians(turns: &Value) -> f64 {
    let t = turns.to_f64();
    (t - t.floor()) * TAU
}

fn snap(x: f64) -> f64 {
    if x.abs() < SNAP_TO_ZERO { 0.0 } else { x }
}

pub fn sin(angle: &Value, unit: AngleUnit) -> CalcResult<Value> {
    let turns = turns_in_range(angle, unit)?;
    match turns.as_rational().and_then(exact_sin) {
        Some(v) => Ok(v),
        None => Value::decimal(snap(reduced_radians(&turns).sin())),
    }
}

pub fn cos(angle: &Value, unit: AngleUnit) -> CalcResult<Value> {
    let turns = turns_in_range(angle, unit)?;
    match turns.as_rational().and_then(exact_cos) {
        Some(v) => Ok(v),
        None => Value::decimal(snap(reduced_radians(&turns).cos())),
    }
}

pub fn tan(angle: &Value, unit: AngleUnit) -> CalcResult<Value> {
    let turns = turns_in_range(angle, unit)?;
    if let Some(result) = turns.as_rational().and_then(exact_tan) {
        return result;
    }
    let r = reduced_radians(&turns);
    let c = snap(r.cos());
    if c == 0.0 {
        return Err(CalcError::Math);
    }
    Value::decimal(snap(r.sin()) / c)
}

/// Turn values (in the function's principal range) whose results we recognize exactly.
const ASIN_TURNS: [(i128, i128); 9] = [(-1, 4), (-1, 6), (-1, 8), (-1, 12), (0, 1), (1, 12), (1, 8), (1, 6), (1, 4)];
const ACOS_TURNS: [(i128, i128); 9] = [(0, 1), (1, 12), (1, 8), (1, 6), (1, 4), (1, 3), (3, 8), (5, 12), (1, 2)];
const ATAN_TURNS: [(i128, i128); 7] = [(-1, 6), (-1, 8), (-1, 12), (0, 1), (1, 12), (1, 8), (1, 6)];

fn exact_inverse(x: &Value, candidates: &[(i128, i128)], f: impl Fn(Rational) -> Option<Value>) -> Option<Rational> {
    candidates.iter().map(|&(n, d)| rat(n, d)).find(|&t| f(t).as_ref() == Some(x))
}

fn inverse(unit: AngleUnit, exact: Option<Rational>, radians: f64) -> CalcResult<Value> {
    let turns = match exact {
        Some(t) => Value::rational(t),
        None => Value::decimal(radians / TAU)?,
    };
    from_turns(&turns, unit)
}

pub fn asin(x: &Value, unit: AngleUnit) -> CalcResult<Value> {
    let v = x.to_f64();
    if v.abs() > 1.0 {
        return Err(CalcError::Math);
    }
    inverse(unit, exact_inverse(x, &ASIN_TURNS, exact_sin), v.asin())
}

pub fn acos(x: &Value, unit: AngleUnit) -> CalcResult<Value> {
    let v = x.to_f64();
    if v.abs() > 1.0 {
        return Err(CalcError::Math);
    }
    inverse(unit, exact_inverse(x, &ACOS_TURNS, exact_cos), v.acos())
}

/// The exact arctangent of `x` in turns, if it is a table value.
pub fn exact_atan_turns(x: &Value) -> Option<Rational> {
    exact_inverse(x, &ATAN_TURNS, |t| exact_tan(t)?.ok())
}

pub fn atan(x: &Value, unit: AngleUnit) -> CalcResult<Value> {
    inverse(unit, exact_atan_turns(x), x.to_f64().atan())
}

#[cfg(test)]
mod tests {
    use super::*;
    use AngleUnit::*;

    fn int(n: i64) -> Value {
        Value::integer(n)
    }

    #[test]
    fn exact_table_covers_all_special_angles() {
        for deg in (-720..=720).step_by(15) {
            let angle = int(deg);
            let want = (deg as f64).to_radians();
            let s = sin(&angle, Deg).unwrap();
            let c = cos(&angle, Deg).unwrap();
            assert!((s.to_f64() - want.sin()).abs() < 1e-12, "sin {deg}");
            assert!((c.to_f64() - want.cos()).abs() < 1e-12, "cos {deg}");
            // Multiples of 30° and 45° are exact; 15° steps in between are not.
            assert_eq!(s.is_exact(), deg % 30 == 0 || deg % 45 == 0, "sin {deg}");
        }
    }

    #[test]
    fn same_angle_in_every_unit() {
        let half = Value::rational(rat(1, 2));
        assert_eq!(sin(&int(30), Deg).unwrap(), half);
        assert_eq!(sin(&Value::Pi(rat(1, 6)), Rad).unwrap(), half);
        assert_eq!(sin(&Value::rational(rat(100, 3)), Gra).unwrap(), half);
    }

    #[test]
    fn tangent_poles_are_errors() {
        assert_eq!(tan(&int(90), Deg), Err(CalcError::Math));
        assert_eq!(tan(&int(-270), Deg), Err(CalcError::Math));
        assert_eq!(tan(&int(60), Deg).unwrap(), int(3).sqrt().unwrap());
        assert_eq!(tan(&Value::Pi(rat(1, 2)), Rad), Err(CalcError::Math));
    }

    #[test]
    fn inverses_give_exact_angles() {
        let half = Value::rational(rat(1, 2));
        assert_eq!(asin(&half, Deg).unwrap(), int(30));
        assert_eq!(asin(&half, Rad).unwrap(), Value::Pi(rat(1, 6)));
        assert_eq!(acos(&half_sqrt(2).neg().unwrap(), Deg).unwrap(), int(135));
        assert_eq!(atan(&int(3).sqrt().unwrap(), Deg).unwrap(), int(60));
        assert_eq!(atan(&int(-1), Gra).unwrap(), int(-50));
        assert_eq!(asin(&int(2), Deg), Err(CalcError::Math));
        let approx = asin(&Value::rational(rat(1, 3)), Deg).unwrap();
        assert!(!approx.is_exact());
        assert!((approx.to_f64() - (1f64 / 3.0).asin().to_degrees()).abs() < 1e-12);
    }

    #[test]
    fn decimal_angles() {
        let s = sin(&int(1), Rad).unwrap();
        assert!((s.to_f64() - 1f64.sin()).abs() < 1e-14);
        // π as a decimal still lands on zero rather than 1.2×10⁻¹⁶.
        assert_eq!(sin(&Value::pi().to_decimal().unwrap(), Rad).unwrap().to_f64(), 0.0);
        assert_eq!(sin(&Value::decimal(9e9).unwrap(), Deg), Err(CalcError::Math));
    }

    #[test]
    fn converts_units() {
        assert_eq!(convert(&int(90), Deg, Rad).unwrap(), Value::Pi(rat(1, 2)));
        assert_eq!(convert(&Value::pi(), Rad, Deg).unwrap(), int(180));
        assert_eq!(convert(&int(100), Gra, Deg).unwrap(), int(90));
    }
}
