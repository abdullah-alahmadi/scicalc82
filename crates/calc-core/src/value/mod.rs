//! The calculator's number system.
//!
//! Every operation first tries to keep an exact result (a rational, a sum of
//! square roots, or a multiple of π). When it can't, it falls back to a
//! [`Decimal`], just like the device switching from `√2/2` to `0.7071067812`.

mod decimal;
mod intmath;
mod rational;
mod surd;

use std::f64::consts::PI;

pub(crate) use decimal::sig_digits;
pub use decimal::{Decimal, INTERNAL_DIGITS};
pub use rational::Rational;
pub use surd::{MAX_TERMS, Surd, SurdTerm};

use crate::error::{CalcError, CalcResult};

/// Largest root index (or rational-exponent denominator) we try to keep exact.
const MAX_EXACT_ROOT: u32 = 1000;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Exact rationals and sums of square roots.
    Surd(Surd),
    /// An exact, non-zero multiple of π.
    Pi(Rational),
    /// Anything that couldn't stay exact.
    Decimal(Decimal),
}

impl Value {
    pub fn zero() -> Self {
        Self::Surd(Surd::zero())
    }

    pub fn integer(n: i64) -> Self {
        Self::rational(n.into())
    }

    pub fn rational(r: Rational) -> Self {
        Self::Surd(Surd::from_rational(r))
    }

    pub fn pi() -> Self {
        Self::Pi(Rational::ONE)
    }

    pub fn decimal(x: f64) -> CalcResult<Self> {
        Decimal::new(x).map(Self::Decimal)
    }

    fn pi_multiple(q: Rational) -> Self {
        if q.is_zero() { Self::zero() } else { Self::Pi(q) }
    }

    /// Parses a typed number such as `12.5`, exactly whenever it fits.
    pub fn parse_literal(s: &str) -> CalcResult<Self> {
        if let Some(r) = Rational::from_decimal_str(s) {
            return Ok(Self::rational(r));
        }
        // Valid but too many digits for an exact fraction.
        let well_formed = s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            && s.bytes().filter(|&b| b == b'.').count() <= 1
            && s.bytes().any(|b| b.is_ascii_digit());
        if !well_formed {
            return Err(CalcError::Syntax);
        }
        Self::decimal(s.parse().map_err(|_| CalcError::Syntax)?)
    }

    pub fn is_exact(&self) -> bool {
        !matches!(self, Self::Decimal(_))
    }

    pub fn as_rational(&self) -> Option<Rational> {
        match self {
            Self::Surd(s) => s.as_rational(),
            _ => None,
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Self::Surd(s) => s.is_zero(),
            Self::Pi(_) => false,
            Self::Decimal(d) => d.get() == 0.0,
        }
    }

    fn is_exact_zero(&self) -> bool {
        matches!(self, Self::Surd(s) if s.is_zero())
    }

    pub fn signum(&self) -> i32 {
        match self {
            Self::Surd(s) => s.signum(),
            Self::Pi(q) => q.signum(),
            Self::Decimal(d) => {
                let x = d.get();
                if x == 0.0 { 0 } else { x.signum() as i32 }
            }
        }
    }

    pub fn to_f64(&self) -> f64 {
        match self {
            Self::Surd(s) => s.to_f64(),
            Self::Pi(q) => q.to_f64() * PI,
            Self::Decimal(d) => d.get(),
        }
    }

    /// The same value as a decimal.
    pub fn to_decimal(&self) -> CalcResult<Self> {
        Self::decimal(self.to_f64())
    }

    fn exact_or(exact: Option<Self>, decimal: impl FnOnce() -> f64) -> CalcResult<Self> {
        match exact {
            Some(v) => Ok(v),
            None => Self::decimal(decimal()),
        }
    }

    pub fn neg(&self) -> CalcResult<Self> {
        let exact = match self {
            Self::Surd(s) => s.checked_neg().map(Self::Surd),
            Self::Pi(q) => q.checked_neg().map(Self::Pi),
            Self::Decimal(_) => None,
        };
        Self::exact_or(exact, || -self.to_f64())
    }

    pub fn add(&self, rhs: &Self) -> CalcResult<Self> {
        let exact = match (self, rhs) {
            (_, b) if b.is_exact_zero() => Some(self.clone()),
            (a, _) if a.is_exact_zero() => Some(rhs.clone()),
            (Self::Surd(a), Self::Surd(b)) => a.checked_add(b).map(Self::Surd),
            (Self::Pi(a), Self::Pi(b)) => a.checked_add(*b).map(Self::pi_multiple),
            _ => None,
        };
        Self::exact_or(exact, || self.to_f64() + rhs.to_f64())
    }

    pub fn sub(&self, rhs: &Self) -> CalcResult<Self> {
        self.add(&rhs.neg()?)
    }

    pub fn mul(&self, rhs: &Self) -> CalcResult<Self> {
        let exact = match (self, rhs) {
            (Self::Surd(a), Self::Surd(b)) => a.checked_mul(b).map(Self::Surd),
            (Self::Pi(q), Self::Surd(s)) | (Self::Surd(s), Self::Pi(q)) => {
                s.as_rational().and_then(|r| q.checked_mul(r)).map(Self::pi_multiple)
            }
            _ => None,
        };
        Self::exact_or(exact, || self.to_f64() * rhs.to_f64())
    }

    pub fn div(&self, rhs: &Self) -> CalcResult<Self> {
        if rhs.is_zero() {
            return Err(CalcError::Math);
        }
        let exact = match (self, rhs) {
            (a, _) if a.is_exact_zero() => Some(Self::zero()),
            (Self::Surd(a), Self::Surd(b)) => a.checked_div(b).map(Self::Surd),
            (Self::Pi(a), Self::Surd(s)) => s.as_rational().and_then(|r| a.checked_div(r)).map(Self::pi_multiple),
            (Self::Pi(a), Self::Pi(b)) => a.checked_div(*b).map(Self::rational),
            _ => None,
        };
        Self::exact_or(exact, || self.to_f64() / rhs.to_f64())
    }

    pub fn recip(&self) -> CalcResult<Self> {
        Self::integer(1).div(self)
    }

    /// `self^n` for an integer `n`. `0^0` and negative powers of zero are a Math ERROR.
    pub fn powi(&self, n: i64) -> CalcResult<Self> {
        if self.is_zero() {
            return if n > 0 { Ok(Self::zero()) } else { Err(CalcError::Math) };
        }
        if n == 0 {
            return Ok(Self::integer(1));
        }
        let exact = match self {
            Self::Surd(s) => s
                .checked_pow(n.unsigned_abs())
                .and_then(|p| if n < 0 { p.checked_recip() } else { Some(p) })
                .map(Self::Surd),
            Self::Pi(_) if n == 1 => Some(self.clone()),
            _ => None,
        };
        Self::exact_or(exact, || self.to_f64().powf(n as f64))
    }

    /// The real `n`-th root. Even roots of negative numbers are a Math ERROR.
    pub fn nth_root(&self, n: u32) -> CalcResult<Self> {
        if n == 0 || (n.is_multiple_of(2) && self.signum() < 0) {
            return Err(CalcError::Math);
        }
        let exact = match self {
            _ if n == 1 => Some(self.clone()),
            Self::Surd(s) if n == 2 => s.checked_sqrt().map(Self::Surd),
            Self::Surd(s) => s.as_rational().and_then(|r| rational_root(r, n)).map(Self::rational),
            _ => None,
        };
        Self::exact_or(exact, || {
            let x = self.to_f64();
            let root = if n == 3 { x.abs().cbrt() } else { x.abs().powf(1.0 / f64::from(n)) };
            root.copysign(x)
        })
    }

    pub fn sqrt(&self) -> CalcResult<Self> {
        self.nth_root(2)
    }

    /// `self^exp`. Rational exponents stay exact where possible, so
    /// `8^(2/3) = 4` and `(−8)^(1/3) = −2`.
    pub fn pow(&self, exp: &Self) -> CalcResult<Self> {
        if let Some(e) = exp.as_rational() {
            if e.is_integer() {
                if let Ok(n) = i64::try_from(e.numer()) {
                    return self.powi(n);
                }
            } else if let (Ok(p), Ok(q)) = (i64::try_from(e.numer()), u32::try_from(e.denom()))
                && q <= MAX_EXACT_ROOT
            {
                return self.nth_root(q)?.powi(p);
            }
        }
        let (x, e) = (self.to_f64(), exp.to_f64());
        if x == 0.0 {
            return if e > 0.0 { Ok(Self::zero()) } else { Err(CalcError::Math) };
        }
        if x < 0.0 && e.fract() != 0.0 {
            return Err(CalcError::Math);
        }
        Self::decimal(x.powf(e))
    }
}

/// The exact `n`-th root of a rational, if numerator and denominator are perfect powers.
fn rational_root(r: Rational, n: u32) -> Option<Rational> {
    let num = intmath::exact_root(r.numer().unsigned_abs(), n)?;
    let den = intmath::exact_root(r.denom() as u128, n)?;
    let num = i128::try_from(num).ok()? * i128::from(r.signum());
    Rational::new(num, den.try_into().ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn int(n: i64) -> Value {
        Value::integer(n)
    }

    fn frac(n: i128, d: i128) -> Value {
        Value::rational(Rational::new(n, d).unwrap())
    }

    fn assert_close(v: &Value, want: f64) {
        let got = v.to_f64();
        assert!((got - want).abs() <= 1e-12 * want.abs().max(1.0), "{got} != {want}");
    }

    #[test]
    fn literals_are_exact() {
        assert_eq!(Value::parse_literal("0.75"), Ok(frac(3, 4)));
        assert_eq!(Value::parse_literal("1..2"), Err(CalcError::Syntax));
        let long = Value::parse_literal(&format!("1.{}", "0".repeat(60))).unwrap();
        assert!(!long.is_exact());
        assert_close(&long, 1.0);
    }

    #[test]
    fn fractions_stay_exact() {
        let third = int(1).div(&int(3)).unwrap();
        assert_eq!(third.add(&frac(1, 6)).unwrap(), frac(1, 2));
        assert_eq!(third.mul(&int(3)).unwrap(), int(1));
        assert_eq!(frac(1, 10).add(&frac(2, 10)).unwrap().sub(&frac(3, 10)).unwrap(), Value::zero());
    }

    #[test]
    fn pi_multiples() {
        let half_pi = Value::pi().div(&int(2)).unwrap();
        assert_eq!(half_pi, Value::Pi(Rational::new(1, 2).unwrap()));
        assert_eq!(half_pi.mul(&int(2)).unwrap(), Value::pi());
        assert_eq!(Value::pi().div(&Value::pi()).unwrap(), int(1));
        assert_eq!(Value::pi().sub(&Value::pi()).unwrap(), Value::zero());
        assert_eq!(Value::pi().add(&Value::zero()).unwrap(), Value::pi());
        assert!(!Value::pi().mul(&int(2).sqrt().unwrap()).unwrap().is_exact());
        assert!(!Value::pi().add(&int(1)).unwrap().is_exact());
    }

    #[test]
    fn powers_and_roots() {
        assert_eq!(int(2).powi(-2).unwrap(), frac(1, 4));
        assert_eq!(int(8).pow(&frac(2, 3)).unwrap(), int(4));
        assert_eq!(int(-8).pow(&frac(1, 3)).unwrap(), int(-2));
        assert_eq!(int(4).pow(&frac(-1, 2)).unwrap(), frac(1, 2));
        assert_eq!(int(2).pow(&frac(1, 2)).unwrap(), int(2).sqrt().unwrap());
        assert_eq!(int(27).nth_root(3).unwrap(), int(3));
        assert_eq!(int(-27).nth_root(3).unwrap(), int(-3));
        assert_close(&int(2).nth_root(3).unwrap(), 2f64.cbrt());
        assert_close(&int(2).pow(&Value::decimal(0.3).unwrap()).unwrap(), 2f64.powf(0.3));
    }

    #[test]
    fn domain_errors() {
        assert_eq!(int(1).div(&Value::zero()), Err(CalcError::Math));
        assert_eq!(Value::zero().powi(0), Err(CalcError::Math));
        assert_eq!(Value::zero().powi(-1), Err(CalcError::Math));
        assert_eq!(int(-4).sqrt(), Err(CalcError::Math));
        assert_eq!(int(-8).pow(&frac(1, 2)), Err(CalcError::Math));
        assert_eq!(int(-8).pow(&Value::decimal(0.3).unwrap()), Err(CalcError::Math));
    }

    #[test]
    fn range_limits() {
        assert_eq!(int(10).powi(100), Err(CalcError::Math));
        let big = int(10).powi(99).unwrap();
        assert!(!big.is_exact());
        assert_close(&big, 1e99);
        assert_eq!(int(10).powi(-100).unwrap(), Value::Decimal(Decimal::ZERO));
    }

    fn small_value() -> impl Strategy<Value = Value> {
        prop_oneof![
            (-100i128..100, 1i128..50).prop_map(|(n, d)| frac(n, d)),
            (1i128..100).prop_map(|n| frac(n, 1).sqrt().unwrap()),
            (-20i128..20, 1i128..10).prop_map(|(n, d)| Value::pi().mul(&frac(n, d)).unwrap()),
            (-1e6f64..1e6).prop_map(|x| Value::decimal(x).unwrap()),
        ]
    }

    proptest! {
        #[test]
        fn exact_paths_agree_with_decimal(x in small_value(), y in small_value()) {
            let (a, b) = (x.to_f64(), y.to_f64());
            let close = |v: CalcResult<Value>, want: f64| {
                let got = v.unwrap().to_f64();
                (got - want).abs() <= 1e-12 * want.abs().max(1.0)
            };
            prop_assert!(close(x.add(&y), a + b));
            prop_assert!(close(x.sub(&y), a - b));
            prop_assert!(close(x.mul(&y), a * b));
            if !y.is_zero() {
                prop_assert!(close(x.div(&y), a / b));
            }
        }
    }
}
