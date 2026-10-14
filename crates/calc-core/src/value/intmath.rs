//! Integer helpers for exact arithmetic.

/// Largest integer that [`split_square`] will factor by trial division.
pub(crate) const FACTOR_LIMIT: u128 = 1_000_000_000_000;

pub(crate) fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Splits `n` into `k²·s` with `s` squarefree, returning `(k, s)`.
///
/// Perfect squares of any size are handled; otherwise `n` must not exceed
/// [`FACTOR_LIMIT`], so the trial division stays fast.
pub(crate) fn split_square(n: u128) -> Option<(u128, u128)> {
    let root = n.isqrt();
    if root * root == n {
        return Some((root, 1));
    }
    if n > FACTOR_LIMIT {
        return None;
    }
    let (mut rem, mut k, mut s) = (n, 1, 1);
    let mut p: u128 = 2;
    while p * p <= rem {
        let mut e = 0;
        while rem % p == 0 {
            rem /= p;
            e += 1;
        }
        k *= p.pow(e / 2);
        if e % 2 == 1 {
            s *= p;
        }
        p += if p == 2 { 1 } else { 2 };
    }
    // Whatever remains is 1 or a prime.
    Some((k, s * rem))
}

/// The exact integer `n`-th root of `x`, if `x` is a perfect `n`-th power.
pub(crate) fn exact_root(x: u128, n: u32) -> Option<u128> {
    match n {
        0 => None,
        _ if x < 2 || n == 1 => Some(x),
        2 => {
            let r = x.isqrt();
            (r * r == x).then_some(r)
        }
        _ => {
            // For n ≥ 3 the root is below 2^43, so the f64 estimate is within ±1.
            let guess = (x as f64).powf(1.0 / f64::from(n)).round() as u128;
            (guess.saturating_sub(1)..=guess + 1).find(|r| r.checked_pow(n) == Some(x))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_squares() {
        assert_eq!(split_square(8), Some((2, 2)));
        assert_eq!(split_square(72), Some((6, 2)));
        assert_eq!(split_square(1), Some((1, 1)));
        assert_eq!(split_square(97), Some((1, 97)));
        assert_eq!(split_square(10u128.pow(30)), Some((10u128.pow(15), 1)));
        assert_eq!(split_square(FACTOR_LIMIT * 10 + 1), None);
    }

    #[test]
    fn exact_roots() {
        assert_eq!(exact_root(27, 3), Some(3));
        assert_eq!(exact_root(28, 3), None);
        assert_eq!(exact_root(1 << 100, 2), Some(1 << 50));
        assert_eq!(exact_root(1 << 100, 5), Some(1 << 20));
        assert_eq!(exact_root(5, 0), None);
    }
}
