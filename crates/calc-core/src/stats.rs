//! STAT mode: sample data and the statistics computed from it.
//!
//! Sums are kept exact where the data allows, so the mean of 1 and 2 is 3/2.

use serde::{Deserialize, Serialize};

use crate::error::{CalcError, CalcResult};
use crate::value::Value;

/// Most rows the STAT editor holds.
pub const MAX_ROWS: usize = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatKind {
    /// Single-variable statistics.
    #[default]
    OneVar,
    /// Paired data with linear regression y = A + Bx.
    Linear,
}

impl StatKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OneVar => "1-VAR",
            Self::Linear => "A+BX",
        }
    }

    pub fn paired(self) -> bool {
        self == Self::Linear
    }
}

/// Values available from the STAT menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatVar {
    N,
    MeanX,
    PopSdX,
    SampleSdX,
    SumX,
    SumX2,
    MeanY,
    PopSdY,
    SampleSdY,
    SumY,
    SumY2,
    SumXY,
    MinX,
    MaxX,
    MinY,
    MaxY,
    RegA,
    RegB,
    RegR,
}

impl StatVar {
    pub fn name(self) -> &'static str {
        match self {
            Self::N => "n",
            Self::MeanX => "x̄",
            Self::PopSdX => "σx",
            Self::SampleSdX => "sx",
            Self::SumX => "Σx",
            Self::SumX2 => "Σx²",
            Self::MeanY => "ȳ",
            Self::PopSdY => "σy",
            Self::SampleSdY => "sy",
            Self::SumY => "Σy",
            Self::SumY2 => "Σy²",
            Self::SumXY => "Σxy",
            Self::MinX => "minX",
            Self::MaxX => "maxX",
            Self::MinY => "minY",
            Self::MaxY => "maxY",
            Self::RegA => "A",
            Self::RegB => "B",
            Self::RegR => "r",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    X,
    Y,
    Freq,
}

impl Column {
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Freq => "FREQ",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatRow {
    pub x: Value,
    pub y: Value,
    pub freq: Value,
}

impl Default for StatRow {
    fn default() -> Self {
        Self { x: Value::zero(), y: Value::zero(), freq: Value::integer(1) }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StatData {
    pub kind: StatKind,
    pub rows: Vec<StatRow>,
    /// Whether the FREQ column is used; without it every row counts once.
    pub freq: bool,
}

/// Frequency-weighted sums and the centered sums of squares built from them.
struct Sums {
    n: Value,
    x: Value,
    y: Value,
    x2: Value,
    y2: Value,
    xy: Value,
}

impl Sums {
    /// Σ(x − x̄)² style sums: Sxx, Syy and Sxy.
    fn centered(&self, a: &Value, b: &Value, ab: &Value) -> CalcResult<Value> {
        ab.sub(&a.mul(b)?.div(&self.n)?)
    }
}

/// √ of a variance, treating rounding-level negatives as zero.
fn sqrt_nonnegative(v: Value) -> CalcResult<Value> {
    if v.signum() < 0 { Ok(Value::zero()) } else { v.sqrt() }
}

impl StatData {
    pub fn new(kind: StatKind, freq: bool) -> Self {
        Self { kind, rows: Vec::new(), freq }
    }

    pub fn columns(&self) -> Vec<Column> {
        let mut cols = vec![Column::X];
        if self.kind.paired() {
            cols.push(Column::Y);
        }
        if self.freq {
            cols.push(Column::Freq);
        }
        cols
    }

    pub fn get(&self, row: usize, col: Column) -> Option<&Value> {
        let r = self.rows.get(row)?;
        Some(match col {
            Column::X => &r.x,
            Column::Y => &r.y,
            Column::Freq => &r.freq,
        })
    }

    /// Sets a cell. Writing just past the last row appends a new row.
    pub fn set(&mut self, row: usize, col: Column, value: Value) -> CalcResult<()> {
        if row == self.rows.len() {
            if self.rows.len() >= MAX_ROWS {
                return Err(CalcError::Memory);
            }
            self.rows.push(StatRow::default());
        }
        let r = self.rows.get_mut(row).ok_or(CalcError::Argument)?;
        match col {
            Column::X => r.x = value,
            Column::Y => r.y = value,
            Column::Freq => r.freq = value,
        }
        Ok(())
    }

    pub fn delete_row(&mut self, row: usize) {
        if row < self.rows.len() {
            self.rows.remove(row);
        }
    }

    fn sums(&self) -> CalcResult<Sums> {
        let mut s = Sums {
            n: Value::zero(),
            x: Value::zero(),
            y: Value::zero(),
            x2: Value::zero(),
            y2: Value::zero(),
            xy: Value::zero(),
        };
        for r in &self.rows {
            let f = if self.freq { r.freq.clone() } else { Value::integer(1) };
            if f.signum() < 0 {
                return Err(CalcError::Math);
            }
            let fx = f.mul(&r.x)?;
            let fy = f.mul(&r.y)?;
            s.n = s.n.add(&f)?;
            s.x2 = s.x2.add(&fx.mul(&r.x)?)?;
            s.y2 = s.y2.add(&fy.mul(&r.y)?)?;
            s.xy = s.xy.add(&fx.mul(&r.y)?)?;
            s.x = s.x.add(&fx)?;
            s.y = s.y.add(&fy)?;
        }
        Ok(s)
    }

    fn extreme(&self, pick_y: bool, max: bool) -> CalcResult<Value> {
        let values = self.rows.iter().map(|r| if pick_y { &r.y } else { &r.x });
        let best = values.reduce(|a, b| {
            let b_wins = if max { b.to_f64() > a.to_f64() } else { b.to_f64() < a.to_f64() };
            if b_wins { b } else { a }
        });
        best.cloned().ok_or(CalcError::Math)
    }

    pub fn value(&self, var: StatVar) -> CalcResult<Value> {
        let s = self.sums()?;
        let n_minus_1 = || s.n.sub(&Value::integer(1));
        let sxx = || s.centered(&s.x, &s.x, &s.x2);
        let syy = || s.centered(&s.y, &s.y, &s.y2);
        let sxy = || s.centered(&s.x, &s.y, &s.xy);
        match var {
            StatVar::N => Ok(s.n),
            StatVar::SumX => Ok(s.x),
            StatVar::SumX2 => Ok(s.x2),
            StatVar::SumY => Ok(s.y),
            StatVar::SumY2 => Ok(s.y2),
            StatVar::SumXY => Ok(s.xy),
            StatVar::MeanX => s.x.div(&s.n),
            StatVar::MeanY => s.y.div(&s.n),
            StatVar::PopSdX => sqrt_nonnegative(sxx()?.div(&s.n)?),
            StatVar::PopSdY => sqrt_nonnegative(syy()?.div(&s.n)?),
            StatVar::SampleSdX => sqrt_nonnegative(sxx()?.div(&n_minus_1()?)?),
            StatVar::SampleSdY => sqrt_nonnegative(syy()?.div(&n_minus_1()?)?),
            StatVar::MinX => self.extreme(false, false),
            StatVar::MaxX => self.extreme(false, true),
            StatVar::MinY => self.extreme(true, false),
            StatVar::MaxY => self.extreme(true, true),
            StatVar::RegB => sxy()?.div(&sxx()?),
            StatVar::RegA => {
                let b = sxy()?.div(&sxx()?)?;
                s.y.sub(&b.mul(&s.x)?)?.div(&s.n)
            }
            StatVar::RegR => sxy()?.div(&sxx()?.mul(&syy()?)?.sqrt()?),
        }
    }

    /// ŷ: the regression estimate of y for `x`.
    pub fn estimate_y(&self, x: &Value) -> CalcResult<Value> {
        self.value(StatVar::RegA)?.add(&self.value(StatVar::RegB)?.mul(x)?)
    }

    /// x̂: the regression estimate of x for `y`.
    pub fn estimate_x(&self, y: &Value) -> CalcResult<Value> {
        y.sub(&self.value(StatVar::RegA)?)?.div(&self.value(StatVar::RegB)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Rational;

    fn int(n: i64) -> Value {
        Value::integer(n)
    }

    fn data(kind: StatKind, points: &[(i64, i64)]) -> StatData {
        let mut d = StatData::new(kind, false);
        for (i, &(x, y)) in points.iter().enumerate() {
            d.set(i, Column::X, int(x)).unwrap();
            if kind.paired() {
                d.set(i, Column::Y, int(y)).unwrap();
            }
        }
        d
    }

    fn close(v: CalcResult<Value>, want: f64) -> bool {
        (v.unwrap().to_f64() - want).abs() < 1e-12
    }

    #[test]
    fn one_variable() {
        let d = data(StatKind::OneVar, &[(1, 0), (2, 0), (3, 0), (4, 0)]);
        assert_eq!(d.value(StatVar::N), Ok(int(4)));
        assert_eq!(d.value(StatVar::MeanX), Ok(Value::rational(Rational::new(5, 2).unwrap())));
        assert_eq!(d.value(StatVar::SumX2), Ok(int(30)));
        assert!(close(d.value(StatVar::PopSdX), 1.25f64.sqrt()));
        assert!(close(d.value(StatVar::SampleSdX), (5.0f64 / 3.0).sqrt()));
        assert_eq!(d.value(StatVar::MinX), Ok(int(1)));
        assert_eq!(d.value(StatVar::MaxX), Ok(int(4)));
    }

    #[test]
    fn frequencies_weight_rows() {
        let mut d = data(StatKind::OneVar, &[(1, 0), (5, 0)]);
        d.freq = true;
        d.set(1, Column::Freq, int(3)).unwrap();
        assert_eq!(d.value(StatVar::N), Ok(int(4)));
        assert_eq!(d.value(StatVar::MeanX), Ok(int(4)));
        // Switching FREQ off counts each row once again.
        d.freq = false;
        assert_eq!(d.value(StatVar::MeanX), Ok(int(3)));
    }

    #[test]
    fn linear_regression() {
        // y = 1 + 2x exactly.
        let d = data(StatKind::Linear, &[(1, 3), (2, 5), (3, 7), (4, 9)]);
        assert_eq!(d.value(StatVar::RegA), Ok(int(1)));
        assert_eq!(d.value(StatVar::RegB), Ok(int(2)));
        assert_eq!(d.value(StatVar::RegR), Ok(int(1)));
        assert_eq!(d.estimate_y(&int(10)), Ok(int(21)));
        assert_eq!(d.estimate_x(&int(21)), Ok(int(10)));
        assert_eq!(d.value(StatVar::SumXY), Ok(int(70)));
        let noisy = data(StatKind::Linear, &[(1, 2), (2, 3), (3, 5), (4, 4)]);
        assert!(close(noisy.value(StatVar::RegR), 0.8));
    }

    #[test]
    fn errors_and_limits() {
        let empty = StatData::new(StatKind::OneVar, false);
        assert_eq!(empty.value(StatVar::N), Ok(Value::zero()));
        assert_eq!(empty.value(StatVar::MeanX), Err(CalcError::Math));
        assert_eq!(empty.value(StatVar::MinX), Err(CalcError::Math));
        let one = data(StatKind::OneVar, &[(7, 0)]);
        assert_eq!(one.value(StatVar::SampleSdX), Err(CalcError::Math));
        let flat = data(StatKind::Linear, &[(2, 1), (2, 5)]);
        assert_eq!(flat.value(StatVar::RegB), Err(CalcError::Math));
        let mut full = StatData::new(StatKind::OneVar, false);
        for i in 0..MAX_ROWS {
            full.set(i, Column::X, int(1)).unwrap();
        }
        assert_eq!(full.set(MAX_ROWS, Column::X, int(1)), Err(CalcError::Memory));
    }
}
