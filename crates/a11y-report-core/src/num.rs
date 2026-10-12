//! Numeric helpers that keep the JSON output deterministic.

use serde::Serializer;

/// Rounds to 2 decimals (half away from zero).
pub fn round2(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let r = (x * 100.0).round() / 100.0;
    if r == 0.0 { 0.0 } else { r }
}

/// Truncates towards negative infinity at 2 decimals: 4.499 -> 4.49.
///
/// A tiny epsilon absorbs binary representation noise (4.47 * 100 = 446.999…).
pub fn floor2(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let r = (x * 100.0 + 1e-9).floor() / 100.0;
    if r == 0.0 { 0.0 } else { r }
}

/// `serialize_with` adapter: every float is rounded to 2 decimals before serialization.
pub fn ser_round2<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(round2(*x))
}

/// `serialize_with` adapter for optional floats.
pub fn ser_round2_opt<S: Serializer>(x: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match x {
        Some(v) => s.serialize_some(&round2(*v)),
        None => s.serialize_none(),
    }
}

/// Converts physical pixels to density-independent pixels (dp = px × 160 / dpi).
pub fn px_to_dp(px: f64, density_dpi: u32) -> f64 {
    if density_dpi == 0 {
        return px;
    }
    px * 160.0 / f64::from(density_dpi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        assert_eq!(round2(4.478), 4.48);
        assert_eq!(floor2(4.478), 4.47);
        assert_eq!(floor2(4.499), 4.49);
        assert_eq!(floor2(4.47), 4.47);
        assert_eq!(floor2(4.5), 4.5);
        assert_eq!(round2(-0.001), 0.0);
        assert_eq!(round2(f64::NAN), 0.0);
    }

    #[test]
    fn dp_conversion() {
        assert_eq!(px_to_dp(420.0, 420), 160.0);
        assert_eq!(px_to_dp(126.0, 420), 48.0);
        assert_eq!(px_to_dp(10.0, 0), 10.0);
    }
}
