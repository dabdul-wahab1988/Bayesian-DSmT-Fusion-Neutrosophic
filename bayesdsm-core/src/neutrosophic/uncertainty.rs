//! Uncertainty propagation into the final rank (plan §15).

use std::collections::HashMap;

/// `mode_band` is the modal band of a vector of band strings.
pub fn mode_band(bands: &[String]) -> String {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for b in bands {
        *counts.entry(b.clone()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(k, _)| k)
        .unwrap_or_else(|| "Low".to_string())
}

pub fn stability(bands: &[String], mode: &str) -> f64 {
    if bands.is_empty() {
        return 0.0;
    }
    let n_match = bands.iter().filter(|b| b.as_str() == mode).count();
    n_match as f64 / bands.len() as f64
}

pub fn rank_ci(ranks: &mut [f64]) -> (f64, f64) {
    use crate::math::stats::quantile;
    (
        quantile(ranks, 0.025).unwrap_or(0.0),
        quantile(ranks, 0.975).unwrap_or(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_band_picks_most_frequent() {
        let bands = vec![
            "Critical".to_string(),
            "Critical".to_string(),
            "High".to_string(),
            "Low".to_string(),
        ];
        assert_eq!(mode_band(&bands), "Critical");
    }

    #[test]
    fn mode_band_empty_returns_low() {
        let bands: Vec<String> = vec![];
        assert_eq!(mode_band(&bands), "Low");
    }

    #[test]
    fn stability_full_match_is_one() {
        let bands = vec!["Critical".to_string(); 10];
        let st = stability(&bands, "Critical");
        assert!((st - 1.0).abs() < 1e-9);
    }

    #[test]
    fn stability_partial_match() {
        let bands = vec![
            "Critical".to_string(),
            "Critical".to_string(),
            "High".to_string(),
            "High".to_string(),
        ];
        // Mode is either Critical or High (tie at 2 each).  The first
        // to be inserted as the max in HashMap iteration order is
        // not deterministic, but the stability for the *mode* string
        // is always 0.5.
        let st_c = stability(&bands, "Critical");
        let st_h = stability(&bands, "High");
        let st_other = stability(&bands, "Low");
        assert!((st_c - 0.5).abs() < 1e-9 || (st_h - 0.5).abs() < 1e-9);
        assert_eq!(st_other, 0.0);
    }

    #[test]
    fn stability_empty_is_zero() {
        let bands: Vec<String> = vec![];
        assert_eq!(stability(&bands, "Critical"), 0.0);
    }

    #[test]
    fn rank_ci_within_actual_extremes() {
        // 15 draws of ranks in [1, 10]: 2.5% and 97.5% percentiles
        // should land inside the actual range of values.
        let mut ranks: Vec<f64> = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 3.0, 4.0, 5.0, 6.0, 7.0,
        ];
        let (lo, hi) = rank_ci(&mut ranks);
        // Quantile(0.025) ≈ 1.35, quantile(0.975) ≈ 9.65 for these
        // inputs; both should be inside [1, 10] (the actual extremes).
        assert!(lo >= 1.0 && lo <= 5.0, "lo={lo}");
        assert!(hi >= 5.0 && hi <= 10.0, "hi={hi}");
    }
}
