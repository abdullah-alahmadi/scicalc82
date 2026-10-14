//! MATRIX mode: matrices up to 3×3 with exact entries.

use serde::{Deserialize, Serialize};

use crate::error::{CalcError, CalcResult};
use crate::value::Value;

/// Largest number of rows or columns, as on the device.
pub const MAX_DIM: usize = 3;

/// The matrix variables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatId {
    A,
    B,
    C,
    /// The last matrix result.
    Ans,
}

impl MatId {
    pub const STORED: [MatId; 3] = [MatId::A, MatId::B, MatId::C];

    pub fn name(self) -> &'static str {
        match self {
            Self::A => "MatA",
            Self::B => "MatB",
            Self::C => "MatC",
            Self::Ans => "MatAns",
        }
    }
}

/// MatA, MatB, MatC and MatAns. Undefined matrices are a Dimension ERROR.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Matrices {
    pub stored: [Option<Matrix>; 3],
    pub ans: Option<Matrix>,
}

impl Matrices {
    pub fn get(&self, id: MatId) -> CalcResult<&Matrix> {
        self.slot(id).as_ref().ok_or(CalcError::Dimension)
    }

    pub fn slot(&self, id: MatId) -> &Option<Matrix> {
        match id {
            MatId::A => &self.stored[0],
            MatId::B => &self.stored[1],
            MatId::C => &self.stored[2],
            MatId::Ans => &self.ans,
        }
    }

    pub fn slot_mut(&mut self, id: MatId) -> &mut Option<Matrix> {
        match id {
            MatId::A => &mut self.stored[0],
            MatId::B => &mut self.stored[1],
            MatId::C => &mut self.stored[2],
            MatId::Ans => &mut self.ans,
        }
    }
}

/// A matrix of real values, stored row by row. Saved through [`MatrixRepr`]
/// so a loaded matrix always has a valid size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MatrixRepr")]
pub struct Matrix {
    rows: usize,
    cols: usize,
    cells: Vec<Value>,
}

#[derive(Deserialize)]
struct MatrixRepr {
    rows: usize,
    cols: usize,
    cells: Vec<Value>,
}

impl TryFrom<MatrixRepr> for Matrix {
    type Error = &'static str;

    fn try_from(m: MatrixRepr) -> Result<Self, Self::Error> {
        let valid = (1..=MAX_DIM).contains(&m.rows) && (1..=MAX_DIM).contains(&m.cols);
        if !valid || m.cells.len() != m.rows * m.cols {
            return Err("invalid matrix size");
        }
        Ok(Self { rows: m.rows, cols: m.cols, cells: m.cells })
    }
}

impl Matrix {
    /// A matrix of zeros; `None` if a dimension is outside 1..=3.
    pub fn zeros(rows: usize, cols: usize) -> Option<Self> {
        ((1..=MAX_DIM).contains(&rows) && (1..=MAX_DIM).contains(&cols)).then(|| Self {
            rows,
            cols,
            cells: vec![Value::zero(); rows * cols],
        })
    }

    fn from_fn(rows: usize, cols: usize, f: impl FnMut(usize, usize) -> CalcResult<Value>) -> CalcResult<Self> {
        let mut f = f;
        let mut cells = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                cells.push(f(r, c)?);
            }
        }
        Ok(Self { rows, cols, cells })
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn get(&self, r: usize, c: usize) -> &Value {
        &self.cells[r * self.cols + c]
    }

    pub fn set(&mut self, r: usize, c: usize, v: Value) {
        self.cells[r * self.cols + c] = v;
    }

    fn map(&self, f: impl Fn(&Value) -> CalcResult<Value>) -> CalcResult<Self> {
        Self::from_fn(self.rows, self.cols, |r, c| f(self.get(r, c)))
    }

    fn same_size(&self, rhs: &Self) -> CalcResult<()> {
        if (self.rows, self.cols) == (rhs.rows, rhs.cols) { Ok(()) } else { Err(CalcError::Dimension) }
    }

    fn square(&self) -> CalcResult<usize> {
        if self.rows == self.cols { Ok(self.rows) } else { Err(CalcError::Dimension) }
    }

    pub fn identity(n: usize) -> CalcResult<Self> {
        Self::from_fn(n, n, |r, c| Ok(Value::integer(i64::from(r == c))))
    }

    pub fn add(&self, rhs: &Self) -> CalcResult<Self> {
        self.same_size(rhs)?;
        Self::from_fn(self.rows, self.cols, |r, c| self.get(r, c).add(rhs.get(r, c)))
    }

    pub fn sub(&self, rhs: &Self) -> CalcResult<Self> {
        self.same_size(rhs)?;
        Self::from_fn(self.rows, self.cols, |r, c| self.get(r, c).sub(rhs.get(r, c)))
    }

    pub fn neg(&self) -> CalcResult<Self> {
        self.map(Value::neg)
    }

    pub fn scale(&self, k: &Value) -> CalcResult<Self> {
        self.map(|v| v.mul(k))
    }

    /// Element-wise absolute value (Abs on a matrix).
    pub fn abs(&self) -> CalcResult<Self> {
        self.map(|v| if v.signum() < 0 { v.neg() } else { Ok(v.clone()) })
    }

    pub fn mul(&self, rhs: &Self) -> CalcResult<Self> {
        if self.cols != rhs.rows {
            return Err(CalcError::Dimension);
        }
        Self::from_fn(self.rows, rhs.cols, |r, c| {
            (0..self.cols).try_fold(Value::zero(), |sum, k| sum.add(&self.get(r, k).mul(rhs.get(k, c))?))
        })
    }

    pub fn transpose(&self) -> Self {
        Self::from_fn(self.cols, self.rows, |r, c| Ok(self.get(c, r).clone())).expect("copying cannot fail")
    }

    pub fn det(&self) -> CalcResult<Value> {
        let m = |r, c| self.get(r, c);
        match self.square()? {
            1 => Ok(m(0, 0).clone()),
            2 => m(0, 0).mul(m(1, 1))?.sub(&m(0, 1).mul(m(1, 0))?),
            _ => {
                // Expansion along the first row.
                let minor = |a: usize, b: usize| m(1, a).mul(m(2, b))?.sub(&m(1, b).mul(m(2, a))?);
                let t0 = m(0, 0).mul(&minor(1, 2)?)?;
                let t1 = m(0, 1).mul(&minor(0, 2)?)?;
                let t2 = m(0, 2).mul(&minor(0, 1)?)?;
                t0.sub(&t1)?.add(&t2)
            }
        }
    }

    /// The inverse, via the adjugate: exact for exact entries. A singular
    /// matrix is a Math ERROR.
    pub fn inverse(&self) -> CalcResult<Self> {
        let n = self.square()?;
        let det = self.det()?;
        if det.is_zero() {
            return Err(CalcError::Math);
        }
        let m = |r: usize, c: usize| self.get(r, c);
        match n {
            1 => Self::from_fn(1, 1, |_, _| Value::integer(1).div(&det)),
            2 => {
                let adj = [[m(1, 1).clone(), m(0, 1).neg()?], [m(1, 0).neg()?, m(0, 0).clone()]];
                Self::from_fn(2, 2, |r, c| adj[r][c].div(&det))
            }
            _ => Self::from_fn(3, 3, |r, c| {
                // Cofactor of (c, r), i.e. the adjugate's (r, c) entry.
                let rows: Vec<usize> = (0..3).filter(|&i| i != c).collect();
                let cols: Vec<usize> = (0..3).filter(|&j| j != r).collect();
                let minor = m(rows[0], cols[0])
                    .mul(m(rows[1], cols[1]))?
                    .sub(&m(rows[0], cols[1]).mul(m(rows[1], cols[0]))?)?;
                let cofactor = if (r + c) % 2 == 0 { minor } else { minor.neg()? };
                cofactor.div(&det)
            }),
        }
    }

    /// `self^n` for a square matrix; negative powers use the inverse.
    pub fn powi(&self, n: i64) -> CalcResult<Self> {
        let size = self.square()?;
        let base = if n < 0 { self.inverse()? } else { self.clone() };
        let mut exp = n.unsigned_abs();
        let mut result = Self::identity(size)?;
        let mut square = base;
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.mul(&square)?;
            }
            exp >>= 1;
            if exp > 0 {
                square = square.mul(&square)?;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Rational;

    fn mat(rows: &[&[i64]]) -> Matrix {
        let (r, c) = (rows.len(), rows[0].len());
        Matrix::from_fn(r, c, |i, j| Ok(Value::integer(rows[i][j]))).unwrap()
    }

    fn frac(n: i128, d: i128) -> Value {
        Value::rational(Rational::new(n, d).unwrap())
    }

    #[test]
    fn arithmetic() {
        let a = mat(&[&[1, 2], &[3, 4]]);
        assert_eq!(a.add(&a).unwrap(), mat(&[&[2, 4], &[6, 8]]));
        assert_eq!(a.mul(&a).unwrap(), mat(&[&[7, 10], &[15, 22]]));
        assert_eq!(a.scale(&Value::integer(3)).unwrap(), mat(&[&[3, 6], &[9, 12]]));
        assert_eq!(a.transpose(), mat(&[&[1, 3], &[2, 4]]));
        assert_eq!(a.powi(3).unwrap(), a.mul(&a).unwrap().mul(&a).unwrap());
        assert_eq!(a.powi(0).unwrap(), Matrix::identity(2).unwrap());
        let row = mat(&[&[1, 2, 3]]);
        assert_eq!(row.mul(&row.transpose()).unwrap(), mat(&[&[14]]));
    }

    #[test]
    fn determinants_and_inverses() {
        let a = mat(&[&[1, 2], &[3, 4]]);
        assert_eq!(a.det(), Ok(Value::integer(-2)));
        let inv = a.inverse().unwrap();
        assert_eq!(inv.get(1, 0), &frac(3, 2));
        assert_eq!(a.mul(&inv).unwrap(), Matrix::identity(2).unwrap());
        let b = mat(&[&[2, 0, 1], &[1, 3, 2], &[1, 1, 2]]);
        assert_eq!(b.det(), Ok(Value::integer(6)));
        assert_eq!(mat(&[&[2, 0, 1], &[1, 3, 2], &[1, 1, 1]]).det(), Ok(Value::zero()));
        assert_eq!(b.mul(&b.inverse().unwrap()).unwrap(), Matrix::identity(3).unwrap());
        assert_eq!(b.powi(-1).unwrap(), b.inverse().unwrap());
        assert_eq!(mat(&[&[5]]).inverse().unwrap(), Matrix::from_fn(1, 1, |_, _| Ok(frac(1, 5))).unwrap());
    }

    #[test]
    fn errors() {
        let a = mat(&[&[1, 2], &[3, 4]]);
        let col = mat(&[&[1], &[2], &[3]]);
        assert_eq!(a.add(&col), Err(CalcError::Dimension));
        assert_eq!(a.mul(&col), Err(CalcError::Dimension));
        assert_eq!(col.det(), Err(CalcError::Dimension));
        assert_eq!(mat(&[&[1, 2], &[2, 4]]).inverse(), Err(CalcError::Math));
        assert!(Matrix::zeros(4, 1).is_none());
        assert!(Matrix::zeros(0, 2).is_none());
    }

    #[test]
    fn loading_checks_the_size() {
        let a = mat(&[&[1, 2], &[3, 4]]);
        let saved = ron::to_string(&a).unwrap();
        assert_eq!(ron::from_str::<Matrix>(&saved).unwrap(), a);
        assert!(ron::from_str::<Matrix>("(rows: 2, cols: 2, cells: [])").is_err());
        assert!(ron::from_str::<Matrix>("(rows: 4, cols: 1, cells: [])").is_err());
    }
}
