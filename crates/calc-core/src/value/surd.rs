use std::cmp::Ordering;

use super::intmath::{FACTOR_LIMIT, gcd, split_square};
use super::rational::Rational;

/// Most terms a [`Surd`] may hold, matching the device's two-term √ form.
pub const MAX_TERMS: usize = 2;

/// Largest power we try to keep exact before falling back to decimal.
const MAX_EXACT_POWER: u64 = 4096;

/// One term `coef·√radicand`, where `radicand` is squarefree.
/// A radicand of 1 makes the term a plain rational.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurdTerm {
    pub coef: Rational,
    pub radicand: u64,
}

impl SurdTerm {
    fn to_f64(self) -> f64 {
        self.coef.to_f64() * (self.radicand as f64).sqrt()
    }

    fn checked_mul(self, rhs: Self) -> Option<Self> {
        // Both radicands are squarefree, so with g = gcd(r1, r2):
        // √(r1·r2) = g·√((r1/g)·(r2/g)), and that product is squarefree too.
        let g = gcd(self.radicand.into(), rhs.radicand.into());
        let radicand = (u128::from(self.radicand) / g) * (u128::from(rhs.radicand) / g);
        if radicand > FACTOR_LIMIT {
            return None;
        }
        let coef = self.coef.checked_mul(rhs.coef)?.checked_mul(Rational::new(g as i128, 1)?)?;
        Some(Self { coef, radicand: radicand as u64 })
    }
}

/// An exact sum of at most [`MAX_TERMS`] terms with distinct radicands, sorted
/// by radicand, with no zero coefficients. Plain rationals are the special
/// case of a single radicand-1 term (or no terms, for zero).
///
/// Operations return `None` when the result leaves this form or overflows;
/// callers then continue in decimal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surd {
    terms: Vec<SurdTerm>,
}

impl Surd {
    pub fn zero() -> Self {
        Self { terms: Vec::new() }
    }

    pub fn from_rational(r: Rational) -> Self {
        Self::from_terms([SurdTerm { coef: r, radicand: 1 }]).expect("one term always fits")
    }

    /// Collects terms with already-squarefree radicands, merging like terms.
    fn from_terms(terms: impl IntoIterator<Item = SurdTerm>) -> Option<Self> {
        let mut merged: Vec<SurdTerm> = Vec::new();
        for t in terms {
            match merged.iter_mut().find(|m| m.radicand == t.radicand) {
                Some(m) => m.coef = m.coef.checked_add(t.coef)?,
                None => merged.push(t),
            }
        }
        merged.retain(|t| !t.coef.is_zero());
        if merged.len() > MAX_TERMS {
            return None;
        }
        merged.sort_by_key(|t| t.radicand);
        Some(Self { terms: merged })
    }

    pub fn terms(&self) -> &[SurdTerm] {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn as_rational(&self) -> Option<Rational> {
        match self.terms.as_slice() {
            [] => Some(Rational::ZERO),
            [t] if t.radicand == 1 => Some(t.coef),
            _ => None,
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.terms.iter().map(|t| t.to_f64()).sum()
    }

    pub fn signum(&self) -> i32 {
        match self.terms.as_slice() {
            [] => 0,
            [t] => t.coef.signum(),
            [a, b] => {
                if a.coef.signum() == b.coef.signum() {
                    return a.coef.signum();
                }
                // Opposite signs: the term with the larger square wins. The
                // squares can't be equal because the radicands differ.
                let square =
                    |t: &SurdTerm| t.coef.checked_mul(t.coef)?.checked_mul(Rational::new(t.radicand.into(), 1)?);
                let a_wins = match (square(a), square(b)) {
                    (Some(sa), Some(sb)) => sa.checked_cmp(sb).map(|o| o == Ordering::Greater),
                    _ => None,
                }
                .unwrap_or_else(|| a.to_f64().abs() > b.to_f64().abs());
                if a_wins { a.coef.signum() } else { b.coef.signum() }
            }
            _ => unreachable!("at most {MAX_TERMS} terms"),
        }
    }

    pub fn checked_neg(&self) -> Option<Self> {
        let terms = self
            .terms
            .iter()
            .map(|t| Some(SurdTerm { coef: t.coef.checked_neg()?, ..*t }))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { terms })
    }

    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        Self::from_terms(self.terms.iter().chain(&rhs.terms).copied())
    }

    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        self.checked_add(&rhs.checked_neg()?)
    }

    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        let mut products = Vec::with_capacity(self.terms.len() * rhs.terms.len());
        for a in &self.terms {
            for b in &rhs.terms {
                products.push(a.checked_mul(*b)?);
            }
        }
        Self::from_terms(products)
    }

    /// `None` for zero as well as when the result can't stay exact.
    pub fn checked_recip(&self) -> Option<Self> {
        match self.terms.as_slice() {
            [] => None,
            // 1/(c√r) = √r / (c·r)
            [t] => {
                let coef = t.coef.checked_mul(Rational::new(t.radicand.into(), 1)?)?.checked_recip()?;
                Self::from_terms([SurdTerm { coef, ..*t }])
            }
            // Rationalize with the conjugate: 1/(a√r + b√s) = (a√r − b√s) / (a²r − b²s).
            [a, b] => {
                let conjugate = Self { terms: vec![*a, SurdTerm { coef: b.coef.checked_neg()?, ..*b }] };
                let denominator = self.checked_mul(&conjugate)?.as_rational()?;
                conjugate.checked_mul(&Self::from_rational(denominator.checked_recip()?))
            }
            _ => unreachable!("at most {MAX_TERMS} terms"),
        }
    }

    pub fn checked_div(&self, rhs: &Self) -> Option<Self> {
        self.checked_mul(&rhs.checked_recip()?)
    }

    /// `self^exp` by repeated squaring.
    pub fn checked_pow(&self, mut exp: u64) -> Option<Self> {
        if exp > MAX_EXACT_POWER {
            return None;
        }
        let mut result = Self::from_rational(Rational::ONE);
        let mut base = self.clone();
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.checked_mul(&base)?;
            }
            exp >>= 1;
            if exp > 0 {
                base = base.checked_mul(&base)?;
            }
        }
        Some(result)
    }

    /// Square root of a non-negative rational, e.g. √(1/2) = √2/2.
    /// `None` for negative values and for surds that aren't rational.
    pub fn checked_sqrt(&self) -> Option<Self> {
        let r = self.as_rational()?;
        if r.signum() < 0 {
            return None;
        }
        if r.is_zero() {
            return Some(Self::zero());
        }
        // √(p/q) = k1√s1 / (k2√s2) = (k1/k2) · √s1 · (1/√s2)
        let (k1, s1) = split_square(r.numer() as u128)?;
        let (k2, s2) = split_square(r.denom() as u128)?;
        let term = |k: u128, s: u128| -> Option<Self> {
            Self::from_terms([SurdTerm { coef: Rational::new(k.try_into().ok()?, 1)?, radicand: s.try_into().ok()? }])
        };
        term(k1, s1)?.checked_div(&term(k2, s2)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rat(n: i128, d: i128) -> Surd {
        Surd::from_rational(Rational::new(n, d).unwrap())
    }

    fn sqrt(n: i128) -> Surd {
        rat(n, 1).checked_sqrt().unwrap()
    }

    fn assert_terms(s: &Surd, want: &[(i128, i128, u64)]) {
        let got: Vec<_> = s.terms().iter().map(|t| (t.coef.numer(), t.coef.denom(), t.radicand)).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn square_roots_simplify() {
        assert_terms(&sqrt(8), &[(2, 1, 2)]);
        assert_terms(&sqrt(9), &[(3, 1, 1)]);
        assert_terms(&rat(1, 2).checked_sqrt().unwrap(), &[(1, 2, 2)]);
        assert_terms(&rat(4, 9).checked_sqrt().unwrap(), &[(2, 3, 1)]);
        assert_eq!(rat(-4, 1).checked_sqrt(), None);
        assert_eq!(sqrt(2).checked_sqrt(), None);
    }

    #[test]
    fn products_and_sums() {
        assert_terms(&sqrt(2).checked_mul(&sqrt(3)).unwrap(), &[(1, 1, 6)]);
        assert_terms(&sqrt(6).checked_mul(&sqrt(10)).unwrap(), &[(2, 1, 15)]);
        assert_eq!(sqrt(2).checked_mul(&sqrt(2)).unwrap().as_rational(), Rational::new(2, 1));
        assert!(sqrt(2).checked_sub(&sqrt(2)).unwrap().is_zero());
        let one_plus_root5 = rat(1, 1).checked_add(&sqrt(5)).unwrap();
        assert_terms(&one_plus_root5, &[(1, 1, 1), (1, 1, 5)]);
        // A third distinct radicand doesn't fit.
        assert_eq!(one_plus_root5.checked_add(&sqrt(2)), None);
        // (√2+√3)² = 5 + 2√6
        let s = sqrt(2).checked_add(&sqrt(3)).unwrap();
        assert_terms(&s.checked_pow(2).unwrap(), &[(5, 1, 1), (2, 1, 6)]);
    }

    #[test]
    fn reciprocals_rationalize() {
        assert_terms(&sqrt(2).checked_recip().unwrap(), &[(1, 2, 2)]);
        // 1/(1+√2) = −1 + √2
        let s = rat(1, 1).checked_add(&sqrt(2)).unwrap();
        assert_terms(&s.checked_recip().unwrap(), &[(-1, 1, 1), (1, 1, 2)]);
        assert_eq!(Surd::zero().checked_recip(), None);
    }

    #[test]
    fn sign_of_mixed_terms() {
        assert_eq!(rat(1, 1).checked_sub(&sqrt(2)).unwrap().signum(), -1);
        assert_eq!(rat(3, 2).checked_sub(&sqrt(2)).unwrap().signum(), 1);
        // 99 − 70√2 ≈ 0.00714
        let close = rat(99, 1).checked_sub(&sqrt(2).checked_mul(&rat(70, 1)).unwrap()).unwrap();
        assert_eq!(close.signum(), 1);
    }

    fn small_surd() -> impl Strategy<Value = Surd> {
        (-50i128..50, 1i128..20, 1i128..50, -50i128..50, 1i128..20).prop_map(|(a, b, r, c, d)| {
            rat(a, b)
                .checked_add(&rat(r, 1).checked_sqrt().unwrap().checked_mul(&rat(c, d)).unwrap())
                .unwrap_or_else(|| rat(a, b))
        })
    }

    proptest! {
        #[test]
        fn exact_matches_float(x in small_surd(), y in small_surd()) {
            let close = |got: Option<Surd>, want: f64| match got {
                Some(s) => (s.to_f64() - want).abs() <= 1e-9 * want.abs().max(1.0),
                None => true, // leaving exact form is allowed
            };
            prop_assert!(close(x.checked_add(&y), x.to_f64() + y.to_f64()));
            prop_assert!(close(x.checked_mul(&y), x.to_f64() * y.to_f64()));
            if !y.is_zero() {
                prop_assert!(close(x.checked_div(&y), x.to_f64() / y.to_f64()));
            }
            prop_assert_eq!(f64::from(x.signum()), x.to_f64().signum() * f64::from(u8::from(!x.is_zero())));
        }
    }
}
