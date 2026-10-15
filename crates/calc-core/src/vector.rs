//! VECTOR mode: 2D and 3D vectors with exact components.

use serde::{Deserialize, Serialize};

use crate::error::{CalcError, CalcResult};
use crate::eval::trig;
use crate::state::AngleUnit;
use crate::value::Value;

/// The vector variables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VctId {
    A,
    B,
    C,
    /// The last vector result.
    Ans,
}

impl VctId {
    pub const STORED: [VctId; 3] = [VctId::A, VctId::B, VctId::C];

    pub fn name(self) -> &'static str {
        match self {
            Self::A => "VctA",
            Self::B => "VctB",
            Self::C => "VctC",
            Self::Ans => "VctAns",
        }
    }
}

/// A 2D or 3D vector. Saved through [`VectorRepr`] so a loaded vector always
/// has a valid size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "VectorRepr")]
pub struct Vector {
    comps: Vec<Value>,
}

#[derive(Deserialize)]
struct VectorRepr {
    comps: Vec<Value>,
}

impl TryFrom<VectorRepr> for Vector {
    type Error = &'static str;

    fn try_from(v: VectorRepr) -> Result<Self, Self::Error> {
        if !(2..=3).contains(&v.comps.len()) {
            return Err("a vector has 2 or 3 components");
        }
        Ok(Self { comps: v.comps })
    }
}

/// VctA, VctB, VctC and VctAns. Undefined vectors are a Dimension ERROR.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vectors {
    pub stored: [Option<Vector>; 3],
    pub ans: Option<Vector>,
}

impl Vectors {
    pub fn get(&self, id: VctId) -> CalcResult<&Vector> {
        self.slot(id).as_ref().ok_or(CalcError::Dimension)
    }

    pub fn slot(&self, id: VctId) -> &Option<Vector> {
        match id {
            VctId::A => &self.stored[0],
            VctId::B => &self.stored[1],
            VctId::C => &self.stored[2],
            VctId::Ans => &self.ans,
        }
    }

    pub fn slot_mut(&mut self, id: VctId) -> &mut Option<Vector> {
        match id {
            VctId::A => &mut self.stored[0],
            VctId::B => &mut self.stored[1],
            VctId::C => &mut self.stored[2],
            VctId::Ans => &mut self.ans,
        }
    }
}

impl Vector {
    /// A zero vector; `None` unless `dim` is 2 or 3.
    pub fn zeros(dim: usize) -> Option<Self> {
        (2..=3).contains(&dim).then(|| Self { comps: vec![Value::zero(); dim] })
    }

    fn from_comps(comps: Vec<Value>) -> Self {
        Self { comps }
    }

    pub fn dim(&self) -> usize {
        self.comps.len()
    }

    pub fn get(&self, i: usize) -> &Value {
        &self.comps[i]
    }

    pub fn set(&mut self, i: usize, v: Value) {
        self.comps[i] = v;
    }

    fn same_dim(&self, rhs: &Self) -> CalcResult<()> {
        if self.dim() == rhs.dim() { Ok(()) } else { Err(CalcError::Dimension) }
    }

    fn zip(&self, rhs: &Self, f: impl Fn(&Value, &Value) -> CalcResult<Value>) -> CalcResult<Self> {
        self.same_dim(rhs)?;
        self.comps.iter().zip(&rhs.comps).map(|(a, b)| f(a, b)).collect::<CalcResult<_>>().map(Self::from_comps)
    }

    pub fn add(&self, rhs: &Self) -> CalcResult<Self> {
        self.zip(rhs, Value::add)
    }

    pub fn sub(&self, rhs: &Self) -> CalcResult<Self> {
        self.zip(rhs, Value::sub)
    }

    pub fn neg(&self) -> CalcResult<Self> {
        self.comps.iter().map(Value::neg).collect::<CalcResult<_>>().map(Self::from_comps)
    }

    pub fn scale(&self, k: &Value) -> CalcResult<Self> {
        self.comps.iter().map(|c| c.mul(k)).collect::<CalcResult<_>>().map(Self::from_comps)
    }

    pub fn dot(&self, rhs: &Self) -> CalcResult<Value> {
        self.same_dim(rhs)?;
        self.comps.iter().zip(&rhs.comps).try_fold(Value::zero(), |sum, (a, b)| sum.add(&a.mul(b)?))
    }

    /// The cross product. Two 2D vectors give the 3D vector (0, 0, z).
    pub fn cross(&self, rhs: &Self) -> CalcResult<Self> {
        self.same_dim(rhs)?;
        let three = |v: &Self| -> [Value; 3] {
            [v.comps[0].clone(), v.comps[1].clone(), v.comps.get(2).cloned().unwrap_or_else(Value::zero)]
        };
        let ([a1, a2, a3], [b1, b2, b3]) = (three(self), three(rhs));
        let det = |p: &Value, q: &Value, r: &Value, s: &Value| p.mul(q)?.sub(&r.mul(s)?);
        Ok(Self::from_comps(vec![det(&a2, &b3, &a3, &b2)?, det(&a3, &b1, &a1, &b3)?, det(&a1, &b2, &a2, &b1)?]))
    }

    /// The length, exact where possible (|(1, 2, 3)| = √14).
    pub fn norm(&self) -> CalcResult<Value> {
        self.dot(self)?.sqrt()
    }

    /// The angle between two vectors, in `unit`; exact at the special angles.
    pub fn angle(&self, rhs: &Self, unit: AngleUnit) -> CalcResult<Value> {
        let lengths = self.norm()?.mul(&rhs.norm()?)?;
        if lengths.is_zero() {
            return Err(CalcError::Math);
        }
        let cos = self.dot(rhs)?.div(&lengths)?;
        // Rounding can push |cos| a hair past 1 for parallel decimal vectors.
        let cos =
            if !cos.is_exact() && cos.to_f64().abs() > 1.0 { Value::decimal(cos.to_f64().signum())? } else { cos };
        trig::acos(&cos, unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Rational;

    fn v(comps: &[i64]) -> Vector {
        Vector::from_comps(comps.iter().map(|&c| Value::integer(c)).collect())
    }

    #[test]
    fn arithmetic() {
        let (a, b) = (v(&[1, 2, 3]), v(&[4, 5, 6]));
        assert_eq!(a.add(&b).unwrap(), v(&[5, 7, 9]));
        assert_eq!(b.sub(&a).unwrap(), v(&[3, 3, 3]));
        assert_eq!(a.scale(&Value::integer(2)).unwrap(), v(&[2, 4, 6]));
        assert_eq!(a.dot(&b), Ok(Value::integer(32)));
        assert_eq!(a.cross(&b).unwrap(), v(&[-3, 6, -3]));
        assert_eq!(v(&[1, 0]).cross(&v(&[0, 1])).unwrap(), v(&[0, 0, 1]));
        assert_eq!(a.norm(), Value::integer(14).sqrt());
        assert_eq!(v(&[3, 4]).norm(), Ok(Value::integer(5)));
    }

    #[test]
    fn angles() {
        assert_eq!(v(&[1, 0]).angle(&v(&[1, 1]), AngleUnit::Deg), Ok(Value::integer(45)));
        assert_eq!(v(&[1, 0, 0]).angle(&v(&[0, 1, 0]), AngleUnit::Deg), Ok(Value::integer(90)));
        assert_eq!(v(&[1, 0]).angle(&v(&[-1, 0]), AngleUnit::Rad), Ok(Value::pi()));
        assert_eq!(v(&[1, 0]).angle(&v(&[2, 0]), AngleUnit::Deg), Ok(Value::zero()));
        let third = v(&[2, 0])
            .angle(&Vector::from_comps(vec![Value::integer(1), Value::integer(3).sqrt().unwrap()]), AngleUnit::Deg);
        assert_eq!(third, Ok(Value::integer(60)));
        assert_eq!(v(&[0, 0]).angle(&v(&[1, 1]), AngleUnit::Deg), Err(CalcError::Math));
        let half = Value::rational(Rational::new(1, 2).unwrap());
        assert_eq!(v(&[1, 2]).scale(&half).unwrap().get(1), &Value::integer(1));
    }

    #[test]
    fn dimension_errors_and_loading() {
        assert_eq!(v(&[1, 2]).add(&v(&[1, 2, 3])), Err(CalcError::Dimension));
        assert_eq!(v(&[1, 2]).dot(&v(&[1, 2, 3])), Err(CalcError::Dimension));
        assert!(Vector::zeros(4).is_none() && Vector::zeros(1).is_none());
        let a = v(&[1, 2, 3]);
        assert_eq!(ron::from_str::<Vector>(&ron::to_string(&a).unwrap()).unwrap(), a);
        assert!(ron::from_str::<Vector>("(comps: [])").is_err());
    }
}
