use serde::{Deserialize, Serialize};

use crate::error::{CalcError, CalcResult};

/// Significant digits carried internally, as on the real device.
pub const INTERNAL_DIGITS: usize = 15;

/// Results at or above this magnitude are a Math ERROR.
const MAX_MAGNITUDE: f64 = 1e100;
/// Results below this magnitude underflow to zero.
const MIN_MAGNITUDE: f64 = 1e-99;

/// An inexact value, kept rounded to [`INTERNAL_DIGITS`] significant digits
/// and inside the device's range of 1e-99 ≤ |x| < 1e100.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Decimal(f64);

impl From<Decimal> for f64 {
    fn from(d: Decimal) -> Self {
        d.0
    }
}

impl TryFrom<f64> for Decimal {
    type Error = CalcError;

    fn try_from(x: f64) -> Result<Self, Self::Error> {
        Self::new(x)
    }
}

impl Decimal {
    pub const ZERO: Self = Self(0.0);

    pub fn new(x: f64) -> CalcResult<Self> {
        if !x.is_finite() {
            return Err(CalcError::Math);
        }
        let x: f64 = format!("{:.*e}", INTERNAL_DIGITS - 1, x).parse().expect("formatted f64 parses");
        if x.abs() >= MAX_MAGNITUDE {
            Err(CalcError::Math)
        } else if x.abs() < MIN_MAGNITUDE {
            Ok(Self::ZERO)
        } else {
            Ok(Self(x))
        }
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// The decimal digits of a finite `x` at [`INTERNAL_DIGITS`] significant
/// digits, as `(negative, digits, exponent)` with `x = ±d₀.d₁d₂… × 10^exponent`.
/// Zero gives all-zero digits and exponent 0.
pub(crate) fn sig_digits(x: f64) -> (bool, Vec<u8>, i32) {
    let s = format!("{:.*e}", INTERNAL_DIGITS - 1, x.abs());
    let (mantissa, exp) = s.split_once('e').expect("exponential format");
    let digits = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    (x < 0.0, digits, exp.parse().expect("integer exponent"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_to_internal_precision() {
        assert_eq!(Decimal::new(0.1 + 0.2).unwrap().get(), 0.3);
        assert_eq!(Decimal::new(1.0 / 3.0).unwrap().get(), 0.333333333333333);
    }

    #[test]
    fn enforces_range() {
        assert_eq!(Decimal::new(1e100), Err(CalcError::Math));
        assert_eq!(Decimal::new(-9.99999999999999e99).unwrap().get(), -9.99999999999999e99);
        assert_eq!(Decimal::new(1e-100).unwrap(), Decimal::ZERO);
        assert_eq!(Decimal::new(f64::NAN), Err(CalcError::Math));
        assert_eq!(Decimal::new(f64::INFINITY), Err(CalcError::Math));
    }

    #[test]
    fn extracts_digits() {
        let (neg, digits, exp) = sig_digits(-0.0125);
        assert!(neg);
        assert_eq!(&digits[..4], &[1, 2, 5, 0]);
        assert_eq!(digits.len(), INTERNAL_DIGITS);
        assert_eq!(exp, -2);
        assert_eq!(sig_digits(0.0).2, 0);
    }
}
