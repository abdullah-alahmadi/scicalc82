//! EQN mode: simultaneous linear equations and polynomial equations.
//!
//! Solutions are exact where the coefficients allow it: linear systems by
//! exact elimination, quadratics by the formula with exact square roots, and
//! cubics by finding an exact rational root (verified exactly) and solving
//! the remaining quadratic. Otherwise the roots are decimals.

use serde::{Deserialize, Serialize};

use crate::error::{CalcError, CalcResult};
use crate::number::Number;
use crate::value::{Rational, Value};

/// A decimal this much smaller than the largest coefficient counts as zero.
const NEGLIGIBLE: f64 = 1e-12;
/// Largest denominator tried when looking for an exact rational root.
const MAX_ROOT_DENOMINATOR: i128 = 10_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EqnKind {
    /// anX + bnY = cn, n = 1, 2
    #[default]
    Linear2,
    /// anX + bnY + cnZ = dn, n = 1, 2, 3
    Linear3,
    /// aX² + bX + c = 0
    Quadratic,
    /// aX³ + bX² + cX + d = 0
    Cubic,
}

impl EqnKind {
    pub fn rows(self) -> usize {
        match self {
            Self::Linear2 => 2,
            Self::Linear3 => 3,
            Self::Quadratic | Self::Cubic => 1,
        }
    }

    pub fn cols(self) -> usize {
        match self {
            Self::Linear2 | Self::Quadratic => 3,
            Self::Linear3 | Self::Cubic => 4,
        }
    }

    /// Column headings in the coefficient editor.
    pub fn headers(self) -> &'static [&'static str] {
        &["a", "b", "c", "d"][..self.cols()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear2 => "anX+bnY=cn",
            Self::Linear3 => "anX+bnY+cnZ=dn",
            Self::Quadratic => "aX²+bX+c=0",
            Self::Cubic => "aX³+bX²+cX+d=0",
        }
    }
}

/// The equation type and its coefficients, row by row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EquationsRepr")]
pub struct Equations {
    kind: EqnKind,
    coefs: Vec<Value>,
}

#[derive(Deserialize)]
struct EquationsRepr {
    kind: EqnKind,
    coefs: Vec<Value>,
}

impl TryFrom<EquationsRepr> for Equations {
    type Error = &'static str;

    fn try_from(e: EquationsRepr) -> Result<Self, Self::Error> {
        if e.coefs.len() != e.kind.rows() * e.kind.cols() {
            return Err("wrong number of coefficients");
        }
        Ok(Self { kind: e.kind, coefs: e.coefs })
    }
}

impl Default for Equations {
    fn default() -> Self {
        Self::new(EqnKind::default())
    }
}

/// One solution: its label (X, Y, Z, X1, ...) and value.
#[derive(Clone, Debug, PartialEq)]
pub struct Root {
    pub label: &'static str,
    pub value: Number,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Solution {
    Roots(Vec<Root>),
    /// A linear system with no solution.
    NoSolution,
    /// A linear system with infinitely many solutions.
    Infinite,
}

impl Equations {
    pub fn new(kind: EqnKind) -> Self {
        Self { kind, coefs: vec![Value::zero(); kind.rows() * kind.cols()] }
    }

    pub fn kind(&self) -> EqnKind {
        self.kind
    }

    pub fn get(&self, row: usize, col: usize) -> &Value {
        &self.coefs[row * self.kind.cols() + col]
    }

    pub fn set(&mut self, row: usize, col: usize, value: Value) {
        let cols = self.kind.cols();
        self.coefs[row * cols + col] = value;
    }

    fn row(&self, r: usize) -> Vec<Value> {
        (0..self.kind.cols()).map(|c| self.get(r, c).clone()).collect()
    }

    pub fn solve(&self) -> CalcResult<Solution> {
        match self.kind {
            EqnKind::Linear2 | EqnKind::Linear3 => {
                let n = self.kind.rows();
                solve_linear((0..n).map(|r| self.row(r)).collect(), &["X", "Y", "Z"][..n])
            }
            EqnKind::Quadratic => {
                let c = self.row(0);
                let roots = quadratic(&c[0], &c[1], &c[2])?;
                // A double root is shown once, as on the device.
                if roots[0] == roots[1] {
                    return Ok(Solution::Roots(vec![Root { label: "X", value: roots[0].clone() }]));
                }
                Ok(labelled(roots))
            }
            EqnKind::Cubic => {
                let c = self.row(0);
                cubic(&c[0], &c[1], &c[2], &c[3]).map(labelled)
            }
        }
    }
}

fn labelled(roots: Vec<Number>) -> Solution {
    let labels = ["X1", "X2", "X3"];
    Solution::Roots(roots.into_iter().zip(labels).map(|(value, label)| Root { label, value }).collect())
}

/// Zero, allowing decimal rounding noise relative to `scale`.
fn negligible(v: &Value, scale: f64) -> bool {
    v.is_zero() || (!v.is_exact() && v.to_f64().abs() <= NEGLIGIBLE * scale)
}

/// Gauss–Jordan elimination on the augmented rows `[a b (c) | d]`.
fn solve_linear(mut m: Vec<Vec<Value>>, labels: &[&'static str]) -> CalcResult<Solution> {
    let n = m.len();
    let scale = m.iter().flatten().map(|v| v.to_f64().abs()).fold(1.0, f64::max);
    let mut pivots = Vec::new();
    for col in 0..n {
        let Some(found) = (pivots.len()..n).find(|&r| !negligible(&m[r][col], scale)) else { continue };
        let row = pivots.len();
        m.swap(row, found);
        let pivot = m[row].clone();
        for other in m.iter_mut().enumerate().filter(|(i, _)| *i != row).map(|(_, r)| r) {
            if other[col].is_zero() {
                continue;
            }
            let factor = other[col].div(&pivot[col])?;
            for (cell, p) in other.iter_mut().zip(&pivot).skip(col) {
                *cell = cell.sub(&factor.mul(p)?)?;
            }
        }
        pivots.push(col);
    }
    if pivots.len() < n {
        // Rank-deficient: inconsistent if a zero row has a non-zero right-hand side.
        let inconsistent = m[pivots.len()..].iter().any(|row| !negligible(&row[n], scale));
        return Ok(if inconsistent { Solution::NoSolution } else { Solution::Infinite });
    }
    let roots = (0..n)
        .map(|r| Ok(Root { label: labels[r], value: Number::Real(m[r][n].div(&m[r][r])?) }))
        .collect::<CalcResult<Vec<_>>>()?;
    Ok(Solution::Roots(roots))
}

/// Both roots of aX² + bX + c = 0, larger (or +i) first; a double root twice.
fn quadratic(a: &Value, b: &Value, c: &Value) -> CalcResult<Vec<Number>> {
    if a.is_zero() {
        return Err(CalcError::Math);
    }
    let two_a = a.mul(&Value::integer(2))?;
    let disc = b.mul(b)?.sub(&Value::integer(4).mul(a)?.mul(c)?)?;
    let scale = [a, b, c].iter().map(|v| v.to_f64().abs()).fold(1.0, f64::max);
    let centre = b.neg()?.div(&two_a)?;
    if negligible(&disc, scale * scale) {
        return Ok(vec![Number::Real(centre.clone()), Number::Real(centre)]);
    }
    if disc.signum() > 0 {
        let offset = disc.sqrt()?.div(&two_a)?;
        let (plus, minus) = (centre.add(&offset)?, centre.sub(&offset)?);
        let mut roots = [plus, minus];
        roots.sort_by(|x, y| y.to_f64().total_cmp(&x.to_f64()));
        return Ok(roots.into_iter().map(Number::Real).collect());
    }
    let im = disc.neg()?.sqrt()?.div(&two_a)?;
    let im = if im.signum() < 0 { im.neg()? } else { im };
    Ok(vec![Number::from_parts(centre.clone(), im.clone()), Number::from_parts(centre, im.neg()?)])
}

/// All three roots of aX³ + bX² + cX + d = 0: real roots (largest first),
/// then any complex pair.
fn cubic(a: &Value, b: &Value, c: &Value, d: &Value) -> CalcResult<Vec<Number>> {
    if a.is_zero() {
        return Err(CalcError::Math);
    }
    let coefs = [a, b, c, d];
    let numeric = numeric_cubic_roots(coefs.map(Value::to_f64));
    if coefs.iter().all(|v| v.is_exact()) {
        for root in numeric.iter().filter(|z| z.im.abs() < 1e-6 * z.re.abs().max(1.0)) {
            let Some(r) = approximate_rational(root.re) else { continue };
            let r = Value::rational(r);
            if horner(&coefs, &r)?.is_zero() {
                // Divide out (X − r) exactly and solve the quadratic that's left.
                let q1 = b.add(&r.mul(a)?)?;
                let q0 = c.add(&r.mul(&q1)?)?;
                let mut roots = vec![Number::Real(r)];
                roots.extend(quadratic(a, &q1, &q0)?);
                return Ok(sort_roots(roots));
            }
        }
    }
    let roots = numeric
        .into_iter()
        .map(|z| {
            let (re, im) = if z.im.abs() < 1e-9 * z.re.abs().max(1.0) { (z.re, 0.0) } else { (z.re, z.im) };
            Ok(Number::from_parts(Value::decimal(re)?, Value::decimal(im)?))
        })
        .collect::<CalcResult<Vec<_>>>()?;
    Ok(sort_roots(roots))
}

fn sort_roots(mut roots: Vec<Number>) -> Vec<Number> {
    // Real roots first, largest first; then complex roots, +i first.
    let key = |n: &Number| (n.is_complex(), -n.re().to_f64(), -n.im().to_f64());
    roots.sort_by(|x, y| key(x).partial_cmp(&key(y)).unwrap_or(std::cmp::Ordering::Equal));
    roots
}

/// p(x) for coefficients highest power first.
fn horner(coefs: &[&Value], x: &Value) -> CalcResult<Value> {
    coefs.iter().try_fold(Value::zero(), |acc, c| acc.mul(x)?.add(c))
}

/// A nearby fraction with a small denominator, by continued fractions. Only
/// a candidate: the caller checks it exactly.
fn approximate_rational(x: f64) -> Option<Rational> {
    let (mut h0, mut h1, mut k0, mut k1) = (0i128, 1i128, 1i128, 0i128);
    let mut v = x;
    for _ in 0..24 {
        let a = v.floor();
        if a.abs() > 1e15 {
            break;
        }
        let ai = a as i128;
        let (h2, k2) = (ai.checked_mul(h1)?.checked_add(h0)?, ai.checked_mul(k1)?.checked_add(k0)?);
        if k2 > MAX_ROOT_DENOMINATOR {
            break;
        }
        (h0, h1, k0, k1) = (h1, h2, k1, k2);
        let frac = v - a;
        if frac.abs() < 1e-12 {
            break;
        }
        v = 1.0 / frac;
    }
    (k1 > 0).then(|| Rational::new(h1, k1)).flatten()
}

#[derive(Clone, Copy, Debug)]
struct C64 {
    re: f64,
    im: f64,
}

impl C64 {
    fn add(self, o: Self) -> Self {
        Self { re: self.re + o.re, im: self.im + o.im }
    }
    fn sub(self, o: Self) -> Self {
        Self { re: self.re - o.re, im: self.im - o.im }
    }
    fn mul(self, o: Self) -> Self {
        Self { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }
    fn div(self, o: Self) -> Self {
        let d = o.re * o.re + o.im * o.im;
        Self { re: (self.re * o.re + self.im * o.im) / d, im: (self.im * o.re - self.re * o.im) / d }
    }
    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

/// The roots of a cubic by Durand–Kerner iteration, polished with Newton steps.
fn numeric_cubic_roots([a, b, c, d]: [f64; 4]) -> Vec<C64> {
    let (b, c, d) = (b / a, c / a, d / a);
    let p = |z: C64| {
        let r = |v: f64| C64 { re: v, im: 0.0 };
        z.mul(z).mul(z).add(r(b).mul(z).mul(z)).add(r(c).mul(z)).add(r(d))
    };
    let dp = |z: C64| {
        let r = |v: f64| C64 { re: v, im: 0.0 };
        r(3.0).mul(z).mul(z).add(r(2.0 * b).mul(z)).add(r(c))
    };
    let seed = C64 { re: 0.4, im: 0.9 };
    let mut z = [C64 { re: 1.0, im: 0.0 }, seed, seed.mul(seed)];
    for _ in 0..500 {
        let mut change: f64 = 0.0;
        for i in 0..3 {
            let others = (0..3).filter(|&j| j != i).fold(C64 { re: 1.0, im: 0.0 }, |acc, j| acc.mul(z[i].sub(z[j])));
            let step = p(z[i]).div(others);
            z[i] = z[i].sub(step);
            change = change.max(step.abs());
        }
        if change < 1e-16 {
            break;
        }
    }
    for root in &mut z {
        for _ in 0..3 {
            let slope = dp(*root);
            if slope.abs() > 1e-300 {
                *root = root.sub(p(*root).div(slope));
            }
        }
    }
    z.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(n: i64) -> Value {
        Value::integer(n)
    }

    fn frac(n: i128, d: i128) -> Value {
        Value::rational(Rational::new(n, d).unwrap())
    }

    fn eqs(kind: EqnKind, coefs: &[i64]) -> Equations {
        let mut e = Equations::new(kind);
        for (i, &c) in coefs.iter().enumerate() {
            e.set(i / kind.cols(), i % kind.cols(), int(c));
        }
        e
    }

    fn roots(kind: EqnKind, coefs: &[i64]) -> Vec<(&'static str, Number)> {
        match eqs(kind, coefs).solve().unwrap() {
            Solution::Roots(r) => r.into_iter().map(|r| (r.label, r.value)).collect(),
            other => panic!("expected roots, got {other:?}"),
        }
    }

    fn real(v: Value) -> Number {
        Number::Real(v)
    }

    #[test]
    fn linear_systems() {
        // x + y = 3, x − y = 1
        assert_eq!(roots(EqnKind::Linear2, &[1, 1, 3, 1, -1, 1]), vec![("X", real(int(2))), ("Y", real(int(1)))]);
        // 2x + 3y = 1, 4x + 5y = 1 → x = −1, y = 1
        assert_eq!(roots(EqnKind::Linear2, &[2, 3, 1, 4, 5, 1]), vec![("X", real(int(-1))), ("Y", real(int(1)))]);
        // Needs a row swap: 0x + y = 2, x + y = 5
        assert_eq!(roots(EqnKind::Linear2, &[0, 1, 2, 1, 1, 5]), vec![("X", real(int(3))), ("Y", real(int(2)))]);
        // x + y + z = 6, 2y + 5z = −4, 2x + 5y − z = 27 → (5, 3, −2)
        let three = roots(EqnKind::Linear3, &[1, 1, 1, 6, 0, 2, 5, -4, 2, 5, -1, 27]);
        assert_eq!(three, vec![("X", real(int(5))), ("Y", real(int(3))), ("Z", real(int(-2)))]);
        // Fractions stay exact: 3x = 1, 3y = 2
        assert_eq!(
            roots(EqnKind::Linear2, &[3, 0, 1, 0, 3, 2]),
            vec![("X", real(frac(1, 3))), ("Y", real(frac(2, 3)))]
        );
    }

    #[test]
    fn degenerate_linear_systems() {
        assert_eq!(eqs(EqnKind::Linear2, &[1, 1, 1, 2, 2, 3]).solve(), Ok(Solution::NoSolution));
        assert_eq!(eqs(EqnKind::Linear2, &[1, 1, 1, 2, 2, 2]).solve(), Ok(Solution::Infinite));
        assert_eq!(eqs(EqnKind::Linear2, &[0, 0, 1, 0, 0, 0]).solve(), Ok(Solution::NoSolution));
        assert_eq!(Equations::new(EqnKind::Linear3).solve(), Ok(Solution::Infinite));
    }

    #[test]
    fn quadratics() {
        let root2 = int(2).sqrt().unwrap();
        assert_eq!(roots(EqnKind::Quadratic, &[1, -3, 2]), vec![("X1", real(int(2))), ("X2", real(int(1)))]);
        assert_eq!(
            roots(EqnKind::Quadratic, &[1, 0, -2]),
            vec![("X1", real(root2.clone())), ("X2", real(root2.neg().unwrap()))]
        );
        assert_eq!(roots(EqnKind::Quadratic, &[2, 3, -2]), vec![("X1", real(frac(1, 2))), ("X2", real(int(-2)))]);
        assert_eq!(roots(EqnKind::Quadratic, &[1, -2, 1]), vec![("X", real(int(1)))]);
        let i = Number::i();
        assert_eq!(roots(EqnKind::Quadratic, &[1, 0, 1]), vec![("X1", i.clone()), ("X2", i.neg().unwrap())]);
        // −x² + 2x − 5 = 0 → 1 ± 2i, with +i first even though a < 0.
        let r = roots(EqnKind::Quadratic, &[-1, 2, -5]);
        assert_eq!(r[0].1, Number::from_parts(int(1), int(2)));
        assert_eq!(eqs(EqnKind::Quadratic, &[0, 1, 1]).solve(), Err(CalcError::Math));
    }

    #[test]
    fn cubics_with_exact_roots() {
        let three = |c: &[i64]| roots(EqnKind::Cubic, c).into_iter().map(|(_, v)| v).collect::<Vec<_>>();
        assert_eq!(three(&[1, -6, 11, -6]), vec![real(int(3)), real(int(2)), real(int(1))]);
        let root2 = int(2).sqrt().unwrap();
        assert_eq!(three(&[1, 0, -2, 0]), vec![real(root2.clone()), real(int(0)), real(root2.neg().unwrap())]);
        // x³ − 1: 1 and −1/2 ± √3/2 i
        let half_root3 = int(3).sqrt().unwrap().div(&int(2)).unwrap();
        let x = three(&[1, 0, 0, -1]);
        assert_eq!(x[0], real(int(1)));
        assert_eq!(x[1], Number::from_parts(frac(-1, 2), half_root3.clone()));
        assert_eq!(x[2], Number::from_parts(frac(-1, 2), half_root3.neg().unwrap()));
        // A triple root and a fractional root.
        assert_eq!(three(&[1, -3, 3, -1]), vec![real(int(1)); 3]);
        assert_eq!(three(&[2, -1, 0, 0])[0], real(frac(1, 2)));
        assert_eq!(eqs(EqnKind::Cubic, &[0, 1, 1, 1]).solve(), Err(CalcError::Math));
    }

    #[test]
    fn cubics_without_rational_roots() {
        let x = roots(EqnKind::Cubic, &[1, 0, 0, -2]);
        assert!((x[0].1.re().to_f64() - 2f64.cbrt()).abs() < 1e-12);
        assert!(!x[0].1.is_complex() && x[1].1.is_complex());
        assert!((x[1].1.im().to_f64() - 3f64.sqrt() / 2.0 * 2f64.cbrt()).abs() < 1e-12);
        // x³ − 3x + 1 has three irrational real roots.
        let x = roots(EqnKind::Cubic, &[1, 0, -3, 1]);
        assert!(x.iter().all(|(_, v)| !v.is_complex()));
        assert!(x[0].1.re().to_f64() > x[1].1.re().to_f64());
        for (_, v) in &x {
            let t = v.re().to_f64();
            assert!((t * t * t - 3.0 * t + 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn saved_coefficients_are_checked() {
        let e = eqs(EqnKind::Quadratic, &[1, 2, 3]);
        assert_eq!(ron::from_str::<Equations>(&ron::to_string(&e).unwrap()).unwrap(), e);
        assert!(ron::from_str::<Equations>("(kind: Cubic, coefs: [])").is_err());
    }
}
