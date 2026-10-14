//! Settings, memory and the random number source.

use serde::{Deserialize, Serialize};

use crate::format::FormatSettings;
use crate::input::Var;
use crate::number::Number;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AngleUnit {
    #[default]
    Deg,
    Rad,
    Gra,
}

/// The calculation mode chosen with MODE.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalcMode {
    #[default]
    Comp,
    /// Complex numbers: i, ∠, and complex results.
    Cmplx,
    Stat,
    Table,
    /// Matrices up to 3×3: MatA–MatC and MatAns.
    Matrix,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub angle: AngleUnit,
    /// Number format, fraction style, and MathIO/LineIO (`format.natural`).
    pub format: FormatSettings,
    /// Whether the STAT editor has a FREQ column.
    pub stat_freq: bool,
    /// CMPLX results in polar form r∠θ rather than a+bi.
    #[serde(default)]
    pub complex_polar: bool,
}

/// Ans plus the variables A–F, X, Y and the independent memory M. In CMPLX
/// mode these can hold complex numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    vars: [Number; 9],
    pub ans: Number,
}

impl Default for Memory {
    fn default() -> Self {
        Self { vars: std::array::from_fn(|_| Number::zero()), ans: Number::zero() }
    }
}

impl Memory {
    pub fn get(&self, var: Var) -> &Number {
        &self.vars[var as usize]
    }

    pub fn set(&mut self, var: Var, value: Number) {
        self.vars[var as usize] = value;
    }
}

/// A small deterministic generator (SplitMix64) for Ran# and RanInt#.
/// The app seeds it; tests can fix the seed.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`; `n` must be non-zero.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self::seeded(0x5EED)
    }
}
