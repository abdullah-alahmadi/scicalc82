//! TABLE mode: f(X) evaluated over a range of X values.

use crate::error::{CalcError, CalcResult};
use crate::input::Row;
use crate::value::Value;

/// Most rows a table may have (the device's limit).
pub const MAX_TABLE_ROWS: usize = 30;

/// The steps of TABLE mode, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableStage {
    #[default]
    Function,
    Start,
    End,
    Step,
    View,
}

impl TableStage {
    pub fn prompt(self) -> &'static str {
        match self {
            Self::Function => "f(X)=",
            Self::Start => "Start?",
            Self::End => "End?",
            Self::Step => "Step?",
            Self::View => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub x: Value,
    pub fx: Result<Value, CalcError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub stage: TableStage,
    pub function: Row,
    pub start: Value,
    pub end: Value,
    pub step: Value,
    pub rows: Vec<TableRow>,
    /// Highlighted row and column (0 = X, 1 = F(X)) in the table view.
    pub selected: usize,
    pub column: usize,
    /// An error from the range or table generation, shown until the next key.
    pub error: Option<CalcError>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            stage: TableStage::Function,
            function: Row::default(),
            start: Value::integer(1),
            end: Value::integer(5),
            step: Value::integer(1),
            rows: Vec::new(),
            selected: 0,
            column: 0,
            error: None,
        }
    }
}

impl Table {
    /// The range value a prompt edits, for showing its current setting.
    pub fn range_value(&self, stage: TableStage) -> Option<&Value> {
        match stage {
            TableStage::Start => Some(&self.start),
            TableStage::End => Some(&self.end),
            TableStage::Step => Some(&self.step),
            _ => None,
        }
    }
}

/// The X values from `start` to `end` inclusive, `step` apart.
pub fn x_values(start: &Value, end: &Value, step: &Value) -> CalcResult<Vec<Value>> {
    if step.is_zero() {
        return Err(CalcError::Math);
    }
    // A small allowance so decimal steps such as 0.1 reach the end value.
    let steps = (end.sub(start)?.div(step)?.to_f64() + 1e-9).floor();
    if steps < 0.0 {
        return Err(CalcError::Math);
    }
    if steps + 1.0 > MAX_TABLE_ROWS as f64 {
        return Err(CalcError::Memory);
    }
    (0..=steps as i64).map(|i| start.add(&step.mul(&Value::integer(i))?)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Rational;

    fn int(n: i64) -> Value {
        Value::integer(n)
    }

    #[test]
    fn ranges() {
        assert_eq!(x_values(&int(1), &int(5), &int(1)).unwrap().len(), 5);
        assert_eq!(x_values(&int(5), &int(1), &int(-2)).unwrap(), vec![int(5), int(3), int(1)]);
        let tenth = Value::rational(Rational::new(1, 10).unwrap());
        let xs = x_values(&Value::zero(), &int(1), &tenth).unwrap();
        assert_eq!(xs.len(), 11);
        assert_eq!(xs[10], int(1));
        let decimal_step = Value::decimal(0.1).unwrap();
        assert_eq!(x_values(&Value::zero(), &int(1), &decimal_step).unwrap().len(), 11);
    }

    #[test]
    fn range_errors() {
        assert_eq!(x_values(&int(1), &int(5), &Value::zero()), Err(CalcError::Math));
        assert_eq!(x_values(&int(5), &int(1), &int(1)), Err(CalcError::Math));
        assert_eq!(x_values(&int(1), &int(31), &int(1)), Err(CalcError::Memory));
        assert_eq!(x_values(&int(1), &int(30), &int(1)).unwrap().len(), 30);
    }
}
