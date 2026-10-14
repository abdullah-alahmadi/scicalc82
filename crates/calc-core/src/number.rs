//! Numbers as the evaluator and memory see them: real, or complex in CMPLX mode.
//!
//! The real and imaginary parts are ordinary [`Value`]s, so complex results
//! stay exact the same way real ones do: (1+i)² = 2i, 2∠30° = √3+i.

use serde::{Deserialize, Serialize};

use crate::error::{CalcError, CalcResult};
use crate::eval::{pol, rec};
use crate::state::AngleUnit;
use crate::value::Value;

/// Largest |n| for an exact integer power of a complex number.
const MAX_COMPLEX_POWER: u64 = 1024;
/// A decimal part this much smaller than the other part is rounding noise.
const NEGLIGIBLE: f64 = 1e-12;

/// A real or complex number. Saved untagged, so a real number is stored
/// exactly like a plain [`Value`] (which keeps older save files loadable).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Number {
    Real(Value),
    Complex(Complex),
}

/// `re + im·i` with a non-zero imaginary part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    pub re: Value,
    pub im: Value,
}

impl From<Value> for Number {
    fn from(v: Value) -> Self {
        Self::Real(v)
    }
}

/// Whether a decimal `part` is negligible next to `other`.
fn negligible(part: &Value, other: &Value) -> bool {
    !part.is_exact() && part.to_f64().abs() <= NEGLIGIBLE * other.to_f64().abs()
}

impl Number {
    pub fn zero() -> Self {
        Self::Real(Value::zero())
    }

    /// The imaginary unit i.
    pub fn i() -> Self {
        Self::Complex(Complex { re: Value::zero(), im: Value::integer(1) })
    }

    /// `re + im·i`, real if the imaginary part is zero (or decimal noise).
    pub fn from_parts(re: Value, im: Value) -> Self {
        if im.is_zero() || negligible(&im, &re) {
            return Self::Real(re);
        }
        let re = if negligible(&re, &im) { Value::zero() } else { re };
        Self::Complex(Complex { re, im })
    }

    /// The number with modulus `r` at angle `theta` (in `unit`).
    pub fn from_polar(r: &Value, theta: &Value, unit: AngleUnit) -> CalcResult<Self> {
        let (re, im) = rec(r, theta, unit)?;
        Ok(Self::from_parts(re, im))
    }

    pub fn re(&self) -> &Value {
        match self {
            Self::Real(v) => v,
            Self::Complex(c) => &c.re,
        }
    }

    pub fn im(&self) -> Value {
        match self {
            Self::Real(_) => Value::zero(),
            Self::Complex(c) => c.im.clone(),
        }
    }

    pub fn is_complex(&self) -> bool {
        matches!(self, Self::Complex(_))
    }

    /// The value, for operations that only take real numbers: a Math ERROR
    /// if it is complex.
    pub fn real(&self) -> CalcResult<&Value> {
        match self {
            Self::Real(v) => Ok(v),
            Self::Complex(_) => Err(CalcError::Math),
        }
    }

    pub fn into_real(self) -> CalcResult<Value> {
        match self {
            Self::Real(v) => Ok(v),
            Self::Complex(_) => Err(CalcError::Math),
        }
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Real(v) if v.is_zero())
    }

    pub fn neg(&self) -> CalcResult<Self> {
        Ok(Self::from_parts(self.re().neg()?, self.im().neg()?))
    }

    pub fn conj(&self) -> CalcResult<Self> {
        Ok(Self::from_parts(self.re().clone(), self.im().neg()?))
    }

    pub fn add(&self, rhs: &Self) -> CalcResult<Self> {
        if let (Self::Real(a), Self::Real(b)) = (self, rhs) {
            return a.add(b).map(Self::Real);
        }
        Ok(Self::from_parts(self.re().add(rhs.re())?, self.im().add(&rhs.im())?))
    }

    pub fn sub(&self, rhs: &Self) -> CalcResult<Self> {
        self.add(&rhs.neg()?)
    }

    pub fn mul(&self, rhs: &Self) -> CalcResult<Self> {
        if let (Self::Real(a), Self::Real(b)) = (self, rhs) {
            return a.mul(b).map(Self::Real);
        }
        // (a+bi)(c+di) = (ac − bd) + (ad + bc)i
        let (a, b, c, d) = (self.re(), &self.im(), rhs.re(), &rhs.im());
        let re = a.mul(c)?.sub(&b.mul(d)?)?;
        let im = a.mul(d)?.add(&b.mul(c)?)?;
        Ok(Self::from_parts(re, im))
    }

    pub fn div(&self, rhs: &Self) -> CalcResult<Self> {
        if let (Self::Real(a), Self::Real(b)) = (self, rhs) {
            return a.div(b).map(Self::Real);
        }
        // (a+bi)/(c+di) = ((ac + bd) + (bc − ad)i) / (c² + d²)
        let (a, b, c, d) = (self.re(), &self.im(), rhs.re(), &rhs.im());
        let denominator = c.mul(c)?.add(&d.mul(d)?)?;
        if denominator.is_zero() {
            return Err(CalcError::Math);
        }
        let re = a.mul(c)?.add(&b.mul(d)?)?.div(&denominator)?;
        let im = b.mul(c)?.sub(&a.mul(d)?)?.div(&denominator)?;
        Ok(Self::from_parts(re, im))
    }

    pub fn recip(&self) -> CalcResult<Self> {
        Self::Real(Value::integer(1)).div(self)
    }

    /// `self^n` for an integer `n`, by repeated squaring.
    pub fn powi(&self, n: i64) -> CalcResult<Self> {
        let Self::Complex(_) = self else {
            return self.re().powi(n).map(Self::Real);
        };
        let mut exp = n.unsigned_abs();
        if exp > MAX_COMPLEX_POWER {
            return Err(CalcError::Math);
        }
        let mut result = Self::Real(Value::integer(1));
        let mut base = self.clone();
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.mul(&base)?;
            }
            exp >>= 1;
            if exp > 0 {
                base = base.mul(&base)?;
            }
        }
        if n < 0 { result.recip() } else { Ok(result) }
    }

    /// |z|, the modulus.
    pub fn abs(&self) -> CalcResult<Value> {
        match self {
            Self::Real(v) if v.signum() < 0 => v.neg(),
            Self::Real(v) => Ok(v.clone()),
            Self::Complex(c) => pol(&c.re, &c.im, AngleUnit::Deg).map(|(r, _)| r),
        }
    }

    /// arg z in `unit`, in (−180°, 180°]. arg 0 is 0.
    pub fn arg(&self, unit: AngleUnit) -> CalcResult<Value> {
        pol(self.re(), &self.im(), unit).map(|(_, theta)| theta)
    }

    /// (r, θ): the polar form.
    pub fn to_polar(&self, unit: AngleUnit) -> CalcResult<(Value, Value)> {
        pol(self.re(), &self.im(), unit)
    }

    /// The real square root, or in CMPLX mode an imaginary one for negative
    /// reals (√−4 = 2i). Square roots of non-real numbers aren't supported,
    /// as on the device.
    pub fn sqrt(&self, complex_mode: bool) -> CalcResult<Self> {
        let x = self.real()?;
        if complex_mode && x.signum() < 0 {
            return Ok(Self::from_parts(Value::zero(), x.neg()?.sqrt()?));
        }
        x.sqrt().map(Self::Real)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Rational;

    fn c(re: i64, im: i64) -> Number {
        Number::from_parts(Value::integer(re), Value::integer(im))
    }

    fn frac(n: i128, d: i128) -> Value {
        Value::rational(Rational::new(n, d).unwrap())
    }

    #[test]
    fn arithmetic_stays_exact() {
        assert_eq!(c(1, 2).add(&c(3, -2)).unwrap(), c(4, 0));
        assert_eq!(c(4, 0), Number::Real(Value::integer(4)));
        assert_eq!(c(1, 1).mul(&c(1, -1)).unwrap(), c(2, 0));
        assert_eq!(c(1, 1).powi(2).unwrap(), c(0, 2));
        assert_eq!(Number::i().powi(4).unwrap(), c(1, 0));
        assert_eq!(Number::i().recip().unwrap(), c(0, -1));
        // (3+4i)/(1+2i) = 11/5 − 2/5 i
        let q = c(3, 4).div(&c(1, 2)).unwrap();
        assert_eq!(q, Number::from_parts(frac(11, 5), frac(-2, 5)));
        assert_eq!(c(1, 1).div(&Number::zero()), Err(CalcError::Math));
        assert_eq!(c(1, -2).conj().unwrap(), c(1, 2));
    }

    #[test]
    fn polar_forms() {
        let root3 = Value::integer(3).sqrt().unwrap();
        let z = Number::from_polar(&Value::integer(2), &Value::integer(30), AngleUnit::Deg).unwrap();
        assert_eq!(z, Number::from_parts(root3, Value::integer(1)));
        let (r, theta) = c(1, 1).to_polar(AngleUnit::Deg).unwrap();
        assert_eq!((r, theta), (Value::integer(2).sqrt().unwrap(), Value::integer(45)));
        assert_eq!(c(-1, -1).arg(AngleUnit::Deg).unwrap(), Value::integer(-135));
        assert_eq!(c(3, 4).abs().unwrap(), Value::integer(5));
        assert_eq!(c(0, -2).arg(AngleUnit::Rad).unwrap(), Value::Pi(Rational::new(-1, 2).unwrap()));
    }

    #[test]
    fn decimal_noise_is_dropped() {
        // 1∠89.9999999999999° style results: tiny decimal parts vanish.
        let z = Number::from_parts(Value::decimal(1e-16).unwrap(), Value::integer(2));
        assert_eq!(z, c(0, 2));
        let z = Number::from_parts(Value::integer(3), Value::decimal(1e-15).unwrap());
        assert_eq!(z, c(3, 0));
        // Exact small parts are kept.
        let tiny = frac(1, 1_000_000_000_000_000);
        assert!(Number::from_parts(Value::integer(3), tiny).is_complex());
    }

    #[test]
    fn square_roots() {
        assert_eq!(c(-4, 0).sqrt(true).unwrap(), c(0, 2));
        assert_eq!(c(-4, 0).sqrt(false), Err(CalcError::Math));
        assert_eq!(c(1, 1).sqrt(true), Err(CalcError::Math));
        assert_eq!(c(9, 0).sqrt(true).unwrap(), c(3, 0));
    }

    #[test]
    fn real_only_operations_reject_complex() {
        assert_eq!(Number::i().real(), Err(CalcError::Math));
        assert_eq!(c(2, 0).real(), Ok(&Value::integer(2)));
    }
}
