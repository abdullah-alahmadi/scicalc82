use std::cmp::Ordering;
use std::fmt;

use super::intmath::gcd;

/// An exact fraction in lowest terms with a positive denominator.
///
/// All arithmetic is checked: `None` means the result does not fit in `i128`,
/// and the caller should fall back to decimal arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rational {
    num: i128,
    den: i128,
}

impl Rational {
    pub const ZERO: Self = Self { num: 0, den: 1 };
    pub const ONE: Self = Self { num: 1, den: 1 };

    /// Builds a reduced fraction. `None` if `den` is zero or a value is `i128::MIN`.
    pub fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 || num == i128::MIN || den == i128::MIN {
            return None;
        }
        let g = gcd(num.unsigned_abs(), den.unsigned_abs()) as i128;
        let (num, den) = (num / g, den / g);
        Some(if den < 0 { Self { num: -num, den: -den } } else { Self { num, den } })
    }

    pub fn integer(n: i64) -> Self {
        Self { num: n.into(), den: 1 }
    }

    /// Parses a plain decimal literal such as `"12.5"` or `".75"` exactly.
    pub fn from_decimal_str(s: &str) -> Option<Self> {
        let (int, frac) = s.split_once('.').unwrap_or((s, ""));
        let all_digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
        if int.len() + frac.len() == 0 || !all_digits(int) || !all_digits(frac) {
            return None;
        }
        let mut num: i128 = 0;
        for b in int.bytes().chain(frac.bytes()) {
            num = num.checked_mul(10)?.checked_add(i128::from(b - b'0'))?;
        }
        Self::new(num, 10i128.checked_pow(frac.len() as u32)?)
    }

    pub fn numer(self) -> i128 {
        self.num
    }

    pub fn denom(self) -> i128 {
        self.den
    }

    pub fn is_zero(self) -> bool {
        self.num == 0
    }

    pub fn is_integer(self) -> bool {
        self.den == 1
    }

    pub fn signum(self) -> i32 {
        self.num.signum() as i32
    }

    pub fn abs(self) -> Self {
        Self { num: self.num.abs(), den: self.den }
    }

    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub fn checked_neg(self) -> Option<Self> {
        Some(Self { num: self.num.checked_neg()?, den: self.den })
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let g = gcd(self.den as u128, rhs.den as u128) as i128;
        let lhs_num = self.num.checked_mul(rhs.den / g)?;
        let rhs_num = rhs.num.checked_mul(self.den / g)?;
        Self::new(lhs_num.checked_add(rhs_num)?, (self.den / g).checked_mul(rhs.den)?)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs.checked_neg()?)
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        // Cross-cancel first so intermediate products stay small.
        let g1 = gcd(self.num.unsigned_abs(), rhs.den as u128).max(1) as i128;
        let g2 = gcd(rhs.num.unsigned_abs(), self.den as u128).max(1) as i128;
        Self::new((self.num / g1).checked_mul(rhs.num / g2)?, (self.den / g2).checked_mul(rhs.den / g1)?)
    }

    /// `None` for zero as well as on overflow.
    pub fn checked_recip(self) -> Option<Self> {
        Self::new(self.den, self.num)
    }

    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs.checked_recip()?)
    }

    /// `None` for `0^0`, a negative power of zero, or overflow.
    pub fn checked_pow(self, exp: i32) -> Option<Self> {
        let base = if exp < 0 { self.checked_recip()? } else { self };
        if exp == 0 && self.is_zero() {
            return None;
        }
        let e = exp.unsigned_abs();
        // Powers of a reduced fraction stay reduced.
        Some(Self { num: base.num.checked_pow(e)?, den: base.den.checked_pow(e)? })
    }

    /// Exact comparison, `None` if the cross products overflow.
    pub fn checked_cmp(self, rhs: Self) -> Option<Ordering> {
        Some(self.num.checked_mul(rhs.den)?.cmp(&rhs.num.checked_mul(self.den)?))
    }
}

impl From<i64> for Rational {
    fn from(n: i64) -> Self {
        Self::integer(n)
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 { write!(f, "{}", self.num) } else { write!(f, "{}/{}", self.num, self.den) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn r(n: i128, d: i128) -> Rational {
        Rational::new(n, d).unwrap()
    }

    #[test]
    fn reduces_and_normalizes_sign() {
        assert_eq!(r(6, -8), r(-3, 4));
        assert_eq!(r(-6, -8).to_string(), "3/4");
        assert_eq!(r(0, -5), Rational::ZERO);
        assert_eq!(Rational::new(1, 0), None);
    }

    #[test]
    fn arithmetic() {
        assert_eq!(r(1, 3).checked_add(r(1, 6)), Some(r(1, 2)));
        assert_eq!(r(1, 3).checked_sub(r(1, 2)), Some(r(-1, 6)));
        assert_eq!(r(2, 3).checked_mul(r(9, 4)), Some(r(3, 2)));
        assert_eq!(r(2, 3).checked_div(r(4, 9)), Some(r(3, 2)));
        assert_eq!(r(1, 2).checked_div(Rational::ZERO), None);
        assert_eq!(r(2, 3).checked_pow(-2), Some(r(9, 4)));
        assert_eq!(Rational::ZERO.checked_pow(0), None);
        assert_eq!(Rational::ZERO.checked_pow(-1), None);
    }

    #[test]
    fn overflow_is_none() {
        let big = r(i128::MAX, 1);
        assert_eq!(big.checked_add(Rational::ONE), None);
        assert_eq!(big.checked_mul(r(2, 1)), None);
        assert_eq!(r(10, 1).checked_pow(39), None);
    }

    #[test]
    fn parses_decimal_literals() {
        assert_eq!(Rational::from_decimal_str("0.75"), Some(r(3, 4)));
        assert_eq!(Rational::from_decimal_str(".5"), Some(r(1, 2)));
        assert_eq!(Rational::from_decimal_str("12."), Some(r(12, 1)));
        assert_eq!(Rational::from_decimal_str("1.2.3"), None);
        assert_eq!(Rational::from_decimal_str("."), None);
        assert_eq!(Rational::from_decimal_str("1e5"), None);
        assert_eq!(Rational::from_decimal_str(&"9".repeat(50)), None);
    }

    proptest! {
        #[test]
        fn matches_float_arithmetic(a in -10_000i128..10_000, b in 1i128..10_000,
                                    c in -10_000i128..10_000, d in 1i128..10_000) {
            let (x, y) = (r(a, b), r(c, d));
            let close = |got: Rational, want: f64| (got.to_f64() - want).abs() <= 1e-9 * want.abs().max(1.0);
            prop_assert!(close(x.checked_add(y).unwrap(), x.to_f64() + y.to_f64()));
            prop_assert!(close(x.checked_mul(y).unwrap(), x.to_f64() * y.to_f64()));
            if c != 0 {
                prop_assert!(close(x.checked_div(y).unwrap(), x.to_f64() / y.to_f64()));
            }
            prop_assert_eq!(x.checked_cmp(y), x.to_f64().partial_cmp(&y.to_f64()));
        }
    }
}
