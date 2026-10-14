//! Turning values into what the display shows.
//!
//! The output is structured (fraction parts, surd terms, mantissa and
//! exponent) so the natural display can draw it. The `Display` impls give a
//! plain linear form for tests and the line display.

use std::fmt;

use crate::value::{Rational, Surd, Value, sig_digits};

/// Significant digits shown for a decimal result.
pub const DISPLAY_DIGITS: usize = 10;

/// Norm1 switches to scientific notation below 10^-2, Norm2 below 10^-9.
const NORM1_SCI_BELOW: i32 = -2;
const NORM2_SCI_BELOW: i32 = -9;

// Size limits for exact forms; larger results are shown as decimals. These
// approximate the usual calculator rules: ±a√b/c with a < 100, b < 1000, c < 100.
const SURD_NUM_LIMIT: u128 = 100;
const SURD_DEN_LIMIT: u128 = 100;
const SURD_RADICAND_LIMIT: u64 = 1000;
const PI_LIMIT: u128 = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NumberFormat {
    #[default]
    Norm1,
    Norm2,
    /// Fixed decimal places, 0–9.
    Fix(u8),
    /// Significant digits in scientific notation, 1–10.
    Sci(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FractionStyle {
    /// `d/c`
    #[default]
    Improper,
    /// `a b/c`
    Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatSettings {
    pub number: NumberFormat,
    pub fraction: FractionStyle,
    /// MathIO (true) or LineIO (false). LineIO shows √ and π results as decimals.
    pub natural: bool,
}

impl Default for FormatSettings {
    fn default() -> Self {
        Self { number: NumberFormat::default(), fraction: FractionStyle::default(), natural: true }
    }
}

/// Which side of the S⇔D toggle to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResultForm {
    /// Exact form when it fits the display, otherwise decimal.
    #[default]
    Standard,
    Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Formatted {
    Decimal(DecimalText),
    Fraction(FractionText),
    Surd(SurdText),
    Pi(PiText),
    Dms(DmsText),
}

/// Degrees, minutes and seconds: `±d°m′s″`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DmsText {
    pub negative: bool,
    pub degrees: u64,
    pub minutes: u8,
    /// Seconds with up to two decimals, e.g. `28.8`.
    pub seconds: String,
}

/// Largest magnitude shown in degrees-minutes-seconds form.
const DMS_LIMIT: f64 = 1e7;

/// `x` in degrees as d°m′s″, with seconds rounded to hundredths. None if
/// the value is too large for the form.
pub fn format_dms(x: f64) -> Option<DmsText> {
    if !x.is_finite() || x.abs() >= DMS_LIMIT {
        return None;
    }
    let hundredths = (x.abs() * 360_000.0).round() as u64;
    let (degrees, rest) = (hundredths / 360_000, hundredths % 360_000);
    let (minutes, seconds) = ((rest / 6000) as u8, rest % 6000);
    let seconds = if seconds % 100 == 0 {
        (seconds / 100).to_string()
    } else {
        format!("{}.{:02}", seconds / 100, seconds % 100).trim_end_matches('0').to_owned()
    };
    Some(DmsText { negative: x < 0.0 && hundredths != 0, degrees, minutes, seconds })
}

/// `±mantissa × 10^exponent`; the exponent is absent in positional notation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecimalText {
    pub negative: bool,
    pub mantissa: String,
    pub exponent: Option<i32>,
}

/// `±whole num/den`, where `whole` is only used in mixed style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FractionText {
    pub negative: bool,
    pub whole: Option<u128>,
    pub num: u128,
    pub den: u128,
}

/// A sum of terms `±num·√radicand / den`; radicand 1 is a rational term.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurdText {
    pub terms: Vec<SurdTermText>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurdTermText {
    pub negative: bool,
    pub num: u128,
    pub radicand: u64,
    pub den: u128,
}

/// `±num·π / den`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiText {
    pub negative: bool,
    pub num: u128,
    pub den: u128,
}

pub fn format_value(value: &Value, settings: &FormatSettings, form: ResultForm) -> Formatted {
    let exact = match (form, value) {
        (ResultForm::Decimal, _) | (_, Value::Decimal(_)) => None,
        (_, Value::Surd(s)) => match s.as_rational() {
            // Integers go through decimal formatting so Fix/Sci apply to them.
            Some(r) if r.is_integer() => None,
            Some(r) => fraction_form(r, settings.fraction).map(Formatted::Fraction),
            None if settings.natural => surd_form(s).map(Formatted::Surd),
            None => None,
        },
        (_, Value::Pi(q)) if settings.natural => pi_form(*q).map(Formatted::Pi),
        (_, Value::Pi(_)) => None,
    };
    exact.unwrap_or_else(|| Formatted::Decimal(format_decimal(value.to_f64(), settings.number)))
}

fn digit_count(n: u128) -> usize {
    n.checked_ilog10().map_or(1, |d| d as usize + 1)
}

fn fraction_form(r: Rational, style: FractionStyle) -> Option<FractionText> {
    let negative = r.signum() < 0;
    let (num, den) = (r.numer().unsigned_abs(), r.denom() as u128);
    let (whole, num) = match style {
        FractionStyle::Mixed if num > den => (Some(num / den), num % den),
        _ => (None, num),
    };
    // The device's limit: 10 characters, counting each fraction separator.
    let width = whole.map_or(0, |w| digit_count(w) + 1) + digit_count(num) + 1 + digit_count(den);
    (width <= DISPLAY_DIGITS).then_some(FractionText { negative, whole, num, den })
}

fn surd_form(s: &Surd) -> Option<SurdText> {
    let terms =
        s.terms()
            .iter()
            .map(|t| {
                let (num, den) = (t.coef.numer().unsigned_abs(), t.coef.denom() as u128);
                (num < SURD_NUM_LIMIT && den < SURD_DEN_LIMIT && t.radicand < SURD_RADICAND_LIMIT)
                    .then_some(SurdTermText { negative: t.coef.signum() < 0, num, radicand: t.radicand, den })
            })
            .collect::<Option<Vec<_>>>()?;
    Some(SurdText { terms })
}

fn pi_form(q: Rational) -> Option<PiText> {
    let (num, den) = (q.numer().unsigned_abs(), q.denom() as u128);
    (num < PI_LIMIT && den < PI_LIMIT).then_some(PiText { negative: q.signum() < 0, num, den })
}

pub fn format_decimal(x: f64, number: NumberFormat) -> DecimalText {
    match number {
        NumberFormat::Norm1 => norm(x, NORM1_SCI_BELOW),
        NumberFormat::Norm2 => norm(x, NORM2_SCI_BELOW),
        NumberFormat::Fix(n) => fix(x, usize::from(n.min(9))),
        NumberFormat::Sci(n) => sci(x, usize::from(n.clamp(1, 10))),
    }
}

/// Engineering notation: the exponent is a multiple of 3. Each `step` moves
/// it up by 3 (the ENG key) or down by 3 (SHIFT ENG).
pub fn format_eng(x: f64, step: i32) -> DecimalText {
    if x == 0.0 {
        return DecimalText { negative: false, mantissa: "0".into(), exponent: Some(0) };
    }
    let (negative, digits, exp) = rounded(x, DISPLAY_DIGITS);
    let eng_exp = exp.div_euclid(3) * 3 + 3 * step;
    DecimalText { negative, mantissa: positional(&digits, exp - eng_exp, 0), exponent: Some(eng_exp) }
}

fn norm(x: f64, sci_below: i32) -> DecimalText {
    if x == 0.0 {
        return DecimalText { negative: false, mantissa: "0".into(), exponent: None };
    }
    let (negative, digits, exp) = rounded(x, DISPLAY_DIGITS);
    if exp >= DISPLAY_DIGITS as i32 || exp < sci_below {
        DecimalText { negative, mantissa: positional(&digits, 0, 0), exponent: Some(exp) }
    } else {
        DecimalText { negative, mantissa: positional(&digits, exp, 0), exponent: None }
    }
}

fn sci(x: f64, sig: usize) -> DecimalText {
    let (negative, digits, exp) = if x == 0.0 { (false, vec![0; sig], 0) } else { rounded(x, sig) };
    DecimalText { negative, mantissa: positional(&digits, 0, sig - 1), exponent: Some(exp) }
}

fn fix(x: f64, places: usize) -> DecimalText {
    let (negative, digits, exp) = sig_digits(x);
    if x != 0.0 && exp >= DISPLAY_DIGITS as i32 {
        return norm(x, NORM1_SCI_BELOW);
    }
    // Keep every digit down to the 10^-places position.
    let keep = exp + 1 + places as i32;
    let (digits, exp) = if x == 0.0 || keep < 0 { (Vec::new(), 0) } else { round_digits(&digits, keep as usize, exp) };
    if digits.iter().all(|&d| d == 0) {
        return DecimalText { negative: false, mantissa: positional(&[0], 0, places), exponent: None };
    }
    if exp >= DISPLAY_DIGITS as i32 {
        return norm(x, NORM1_SCI_BELOW);
    }
    DecimalText { negative, mantissa: positional(&digits, exp, places), exponent: None }
}

/// `x` rounded half-up to `sig` significant digits.
fn rounded(x: f64, sig: usize) -> (bool, Vec<u8>, i32) {
    let (negative, digits, exp) = sig_digits(x);
    let (digits, exp) = round_digits(&digits, sig, exp);
    (negative, digits, exp)
}

/// Rounds `d₀.d₁d₂… × 10^exp` half-up to `keep` digits, like the device
/// (rather than the round-half-even of float formatting).
fn round_digits(digits: &[u8], keep: usize, exp: i32) -> (Vec<u8>, i32) {
    let mut out: Vec<u8> = digits.iter().copied().chain(std::iter::repeat(0)).take(keep).collect();
    if digits.get(keep).is_some_and(|&d| d >= 5) {
        for i in (0..out.len()).rev() {
            if out[i] < 9 {
                out[i] += 1;
                return (out, exp);
            }
            out[i] = 0;
        }
        // Carried past the leading digit: 9.99 → 10.0
        out.insert(0, 1);
        out.truncate(keep.max(1));
        return (out, exp + 1);
    }
    (out, exp)
}

/// Writes `d₀.d₁d₂… × 10^exp` positionally with at least `min_frac`
/// fractional digits and no trailing zeros beyond that.
fn positional(digits: &[u8], exp: i32, min_frac: usize) -> String {
    let digit = |i: usize| char::from(b'0' + digits.get(i).copied().unwrap_or(0));
    let (int, mut frac): (String, String) = if exp >= 0 {
        let int_len = exp as usize + 1;
        ((0..int_len).map(digit).collect(), (int_len..digits.len()).map(digit).collect())
    } else {
        let leading_zeros = std::iter::repeat_n('0', (-exp - 1) as usize);
        ("0".into(), leading_zeros.chain((0..digits.len()).map(digit)).collect())
    };
    while frac.len() > min_frac && frac.ends_with('0') {
        frac.pop();
    }
    while frac.len() < min_frac {
        frac.push('0');
    }
    if frac.is_empty() { int } else { format!("{int}.{frac}") }
}

impl fmt::Display for Formatted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decimal(d) => d.fmt(f),
            Self::Fraction(fr) => fr.fmt(f),
            Self::Surd(s) => s.fmt(f),
            Self::Pi(p) => p.fmt(f),
            Self::Dms(d) => d.fmt(f),
        }
    }
}

impl fmt::Display for DmsText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        write!(f, "{sign}{}°{}′{}″", self.degrees, self.minutes, self.seconds)
    }
}

impl fmt::Display for DecimalText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        write!(f, "{sign}{}", self.mantissa)?;
        if let Some(e) = self.exponent {
            write!(f, "×10^{e}")?;
        }
        Ok(())
    }
}

impl fmt::Display for FractionText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        if let Some(w) = self.whole {
            write!(f, "{w} ")?;
        }
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl fmt::Display for SurdText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.terms.is_empty() {
            return f.write_str("0");
        }
        for (i, t) in self.terms.iter().enumerate() {
            match (t.negative, i) {
                (true, _) => f.write_str("-")?,
                (false, 0) => {}
                (false, _) => f.write_str("+")?,
            }
            if t.radicand == 1 || t.num != 1 {
                write!(f, "{}", t.num)?;
            }
            if t.radicand != 1 {
                write!(f, "√{}", t.radicand)?;
            }
            if t.den != 1 {
                write!(f, "/{}", t.den)?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for PiText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        if self.num != 1 {
            write!(f, "{}", self.num)?;
        }
        f.write_str("π")?;
        if self.den != 1 {
            write!(f, "/{}", self.den)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn with(number: NumberFormat, fraction: FractionStyle) -> FormatSettings {
        FormatSettings { number, fraction, natural: true }
    }

    fn show(v: &Value) -> String {
        format_value(v, &FormatSettings::default(), ResultForm::Standard).to_string()
    }

    fn show_dec(x: f64, number: NumberFormat) -> String {
        format_decimal(x, number).to_string()
    }

    fn int(n: i64) -> Value {
        Value::integer(n)
    }

    fn frac(n: i128, d: i128) -> Value {
        Value::rational(Rational::new(n, d).unwrap())
    }

    #[test]
    fn norm_modes() {
        use NumberFormat::{Norm1, Norm2};
        assert_eq!(show_dec(1.0 / 70.0, Norm1), "0.01428571429");
        assert_eq!(show_dec(2.0 / 3.0, Norm1), "0.6666666667");
        assert_eq!(show_dec(0.001, Norm1), "1×10^-3");
        assert_eq!(show_dec(0.001, Norm2), "0.001");
        assert_eq!(show_dec(1.5e-10, Norm2), "1.5×10^-10");
        assert_eq!(show_dec(9999999999.0, Norm1), "9999999999");
        assert_eq!(show_dec(12345678901.0, Norm1), "1.23456789×10^10");
        assert_eq!(show_dec(99999999999.5, Norm1), "1×10^11");
        assert_eq!(show_dec(-2.5, Norm1), "-2.5");
        assert_eq!(show_dec(0.0, Norm1), "0");
        assert_eq!(show_dec(1e99, Norm1), "1×10^99");
    }

    #[test]
    fn fix_and_sci_round_half_up() {
        use NumberFormat::{Fix, Sci};
        assert_eq!(show_dec(2.5, Fix(0)), "3");
        assert_eq!(show_dec(2.5, Fix(2)), "2.50");
        assert_eq!(show_dec(1.0 / 3.0, Fix(3)), "0.333");
        assert_eq!(show_dec(0.0005, Fix(3)), "0.001");
        assert_eq!(show_dec(0.0004, Fix(3)), "0.000");
        assert_eq!(show_dec(-0.0004, Fix(3)), "0.000");
        assert_eq!(show_dec(9.9999, Fix(2)), "10.00");
        assert_eq!(show_dec(0.0, Fix(2)), "0.00");
        assert_eq!(show_dec(1.5e12, Fix(2)), "1.5×10^12");
        assert_eq!(show_dec(1.0 / 3.0, Sci(3)), "3.33×10^-1");
        assert_eq!(show_dec(123456.0, Sci(1)), "1×10^5");
        assert_eq!(show_dec(9.96, Sci(2)), "1.0×10^1");
        assert_eq!(show_dec(0.0, Sci(3)), "0.00×10^0");
    }

    #[test]
    fn engineering() {
        assert_eq!(format_eng(1234.0, 0).to_string(), "1.234×10^3");
        assert_eq!(format_eng(1234.0, 1).to_string(), "0.001234×10^6");
        assert_eq!(format_eng(1234.0, -1).to_string(), "1234×10^0");
        assert_eq!(format_eng(0.00056, 0).to_string(), "560×10^-6");
    }

    #[test]
    fn fractions() {
        assert_eq!(show(&frac(1, 3)), "1/3");
        assert_eq!(show(&frac(-3, 4)), "-3/4");
        assert_eq!(show(&int(5)), "5");
        let mixed = with(NumberFormat::Norm1, FractionStyle::Mixed);
        assert_eq!(format_value(&frac(7, 3), &mixed, ResultForm::Standard).to_string(), "2 1/3");
        assert_eq!(format_value(&frac(-1, 3), &mixed, ResultForm::Standard).to_string(), "-1/3");
        // Too wide for 10 characters, so shown as a decimal.
        assert_eq!(show(&frac(1, 1_234_567_891)), "8.100000067×10^-10");
        assert_eq!(show(&frac(1, 123_456_789)), "8.100000074×10^-9");
        assert_eq!(show(&frac(1, 12_345_678)), "1/12345678");
    }

    #[test]
    fn surds_and_pi() {
        let root2 = int(2).sqrt().unwrap();
        assert_eq!(show(&root2.div(&int(2)).unwrap()), "√2/2");
        assert_eq!(show(&int(12).sqrt().unwrap()), "2√3");
        let golden = int(1).add(&int(5).sqrt().unwrap()).unwrap().div(&int(2)).unwrap();
        assert_eq!(show(&golden), "1/2+√5/2");
        assert_eq!(show(&int(1).sub(&root2).unwrap()), "1-√2");
        assert_eq!(show(&root2.mul(&int(150)).unwrap()), "212.1320344");
        assert_eq!(show(&Value::pi()), "π");
        assert_eq!(show(&Value::pi().mul(&frac(-3, 4)).unwrap()), "-3π/4");
        assert_eq!(show(&Value::pi().mul(&int(2)).unwrap()), "2π");
    }

    #[test]
    fn degrees_minutes_seconds() {
        let show = |x: f64| format_dms(x).unwrap().to_string();
        assert_eq!(show(1.5), "1°30′0″");
        assert_eq!(show(2.258), "2°15′28.8″");
        assert_eq!(show(-0.1), "-0°6′0″");
        assert_eq!(show(1.0 / 3.0), "0°20′0″");
        assert_eq!(show(10.999999999), "11°0′0″");
        assert_eq!(format_dms(1e7), None);
    }

    #[test]
    fn line_io_has_no_surd_or_pi_forms() {
        let line = FormatSettings { natural: false, ..FormatSettings::default() };
        let root2 = Value::integer(2).sqrt().unwrap();
        assert_eq!(format_value(&root2, &line, ResultForm::Standard).to_string(), "1.414213562");
        assert_eq!(format_value(&Value::pi(), &line, ResultForm::Standard).to_string(), "3.141592654");
        assert_eq!(format_value(&frac(3, 4), &line, ResultForm::Standard).to_string(), "3/4");
    }

    #[test]
    fn standard_decimal_toggle() {
        let s = FormatSettings::default();
        let v = int(2).sqrt().unwrap().div(&int(2)).unwrap();
        assert_eq!(format_value(&v, &s, ResultForm::Decimal).to_string(), "0.7071067812");
        assert_eq!(format_value(&frac(1, 4), &s, ResultForm::Decimal).to_string(), "0.25");
        assert_eq!(format_value(&Value::pi(), &s, ResultForm::Decimal).to_string(), "3.141592654");
    }

    proptest! {
        #[test]
        fn norm1_round_trips_to_ten_digits(mantissa in 1.0f64..10.0, exp in -99i32..100, neg: bool) {
            let x = Value::decimal(if neg { -mantissa } else { mantissa } * 10f64.powi(exp));
            prop_assume!(x.is_ok());
            let x = x.unwrap().to_f64();
            prop_assume!(x != 0.0);
            let text = format_decimal(x, NumberFormat::Norm1);
            let sign = if text.negative { -1.0 } else { 1.0 };
            let back = sign * text.mantissa.parse::<f64>().unwrap() * 10f64.powi(text.exponent.unwrap_or(0));
            prop_assert!(((back - x) / x).abs() <= 5e-10, "{x} shown as {text}");
            let sig_digits = text.mantissa.bytes().filter(u8::is_ascii_digit).skip_while(|&b| b == b'0').count();
            prop_assert!(sig_digits <= DISPLAY_DIGITS, "{x} shown as {text}");
        }
    }
}
