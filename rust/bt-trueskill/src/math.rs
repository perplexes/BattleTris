//! Standard-normal helpers and the truncated-Gaussian `v`/`w` correction
//! functions used by the TrueSkill update (Herbrich et al., 2007).
//!
//! The cdf (via an `erfc` approximation) and its inverse (Acklam's rational
//! approximation) are computed in-crate rather than pulled from a dependency, so
//! the crate stays self-contained; their accuracy (~1e-7 / ~1e-9) is far finer
//! than rating math needs. The `v`/`w` functions are the heart of
//! the update: each maps a normalized skill gap to "how much should this result
//! move the rating," derived from a Gaussian truncated at the win/draw boundary.

use std::f64::consts::PI;

/// Standard normal pdf `N(x; 0, 1)`.
#[inline]
pub fn pdf(x: f64) -> f64 {
    (-(x * x) / 2.0).exp() / (2.0 * PI).sqrt()
}

/// Standard normal cdf `Phi(x)`: the probability mass below `x`, computed via `erfc`.
#[inline]
pub fn cdf(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

/// Complementary error function (Numerical Recipes `erfcc`, ~1.2e-7 accuracy).
pub fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let ans = t
        * (-z * z - 1.26551223
            + t * (1.00002368
                + t * (0.37409196
                    + t * (0.09678418
                        + t * (-0.18628806
                            + t * (0.27886807
                                + t * (-1.13520398
                                    + t * (1.48851587
                                        + t * (-0.82215223 + t * 0.17087277)))))))))
            .exp();
    if x >= 0.0 {
        ans
    } else {
        2.0 - ans
    }
}

/// Inverse standard-normal cdf (probit), Acklam's algorithm (~1e-9 rel error).
pub fn inv_cdf(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383_577_518_672_69e2,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];

    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= P_HIGH {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

/// `V` correction for a win: the mean-shift factor `pdf/cdf` of a Gaussian
/// truncated at the win boundary. `t` is the normalized skill difference, `e` the
/// normalized draw margin. Largest when the winner was the underdog (`t` very
/// negative), which is the surprising result that should move ratings most.
pub fn v_win(t: f64, e: f64) -> f64 {
    let x = t - e;
    let denom = cdf(x);
    if denom < 1e-300 {
        // For a deep upset the cdf underflows to 0; use the analytic limit
        // pdf(x)/cdf(x) -> -x as x -> -inf so the ratio stays finite.
        -x
    } else {
        pdf(x) / denom
    }
}

/// `W` correction for a win: the variance-shrink factor, clamped to `[0, 1]`.
/// How much of the player's uncertainty this result resolves; the true factor is
/// below 1, but it is clamped because rounding in the `v`/`pdf` approximations
/// could otherwise nudge it just outside the valid range (a negative shrink would
/// grow variance, which would be physically wrong).
pub fn w_win(t: f64, e: f64) -> f64 {
    let x = t - e;
    let v = v_win(t, e);
    let w = v * (v + x);
    w.clamp(0.0, 1.0)
}

/// `V` correction for a draw: the mean shift toward the opponent. Uses the
/// two-sided truncation (the result fell *within* `±e` of a tie) rather than the
/// one-sided win boundary.
pub fn v_draw(t: f64, e: f64) -> f64 {
    let denom = cdf(e - t) - cdf(-e - t);
    let numer = pdf(-e - t) - pdf(e - t);
    if denom < 1e-300 {
        // The truncation interval has underflowed to zero mass; fall back to the
        // win correction's asymptotics so the factor stays finite.
        if t < 0.0 {
            -t - e
        } else {
            -t + e
        }
    } else {
        numer / denom
    }
}

/// `W` correction for a draw, clamped to `[0, 1]`. Variance-shrink counterpart of
/// [`v_draw`]; the degenerate interval returns the maximal shrink (1.0), and the
/// result is clamped for the same reason as [`w_win`].
pub fn w_draw(t: f64, e: f64) -> f64 {
    let denom = cdf(e - t) - cdf(-e - t);
    let v = v_draw(t, e);
    if denom < 1e-300 {
        return 1.0;
    }
    let w = v * v + ((e - t) * pdf(e - t) - (-e - t) * pdf(-e - t)) / denom;
    w.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "expected {a} ≈ {b}");
    }

    #[test]
    fn cdf_known_points() {
        approx(cdf(0.0), 0.5, 1e-6); // erfcc approximation is ~1e-7 accurate
        approx(cdf(1.96), 0.9750021, 1e-5);
        approx(cdf(-1.96), 0.0249979, 1e-5);
        approx(cdf(1.0), 0.8413447, 1e-5);
    }

    #[test]
    fn inv_cdf_is_inverse_of_cdf() {
        for &p in &[0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99] {
            approx(cdf(inv_cdf(p)), p, 1e-6);
        }
    }

    #[test]
    fn inv_cdf_known_points() {
        approx(inv_cdf(0.5), 0.0, 1e-9);
        approx(inv_cdf(0.975), 1.959964, 1e-4);
        approx(inv_cdf(0.55), 0.1256613, 1e-4);
    }

    #[test]
    fn pdf_known_points() {
        approx(pdf(0.0), 0.3989423, 1e-6);
        approx(pdf(1.0), 0.2419707, 1e-6);
    }

    #[test]
    fn w_functions_are_bounded() {
        for t in [-3.0, -1.0, 0.0, 1.0, 3.0] {
            for e in [0.0, 0.1, 0.5] {
                let ww = w_win(t, e);
                assert!((0.0..=1.0).contains(&ww));
                let wd = w_draw(t, e);
                assert!((0.0..=1.0).contains(&wd));
            }
        }
    }
}
