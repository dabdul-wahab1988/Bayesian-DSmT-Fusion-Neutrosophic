//! Robust normalization (plan §5.6):  median + IQR -> squash to [0,1].

pub fn normalize(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let mut s = values.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    let median = if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    };
    let q1 = crate::math::stats::quantile(&mut s.clone(), 0.25).unwrap_or(median);
    let q3 = crate::math::stats::quantile(&mut s.clone(), 0.75).unwrap_or(median);
    let iqr = (q3 - q1).max(1e-12);
    let denom = if iqr > 0.0 { iqr } else { 1.0 };
    values
        .iter()
        .map(|x| {
            let z = (x - median) / denom;
            1.0 / (1.0 + (-z).exp())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squash_in_unit_range() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        for y in normalize(&xs) {
            assert!((0.0..=1.0).contains(&y), "y={y} out of range");
        }
    }

    #[test]
    fn median_is_halfway() {
        let xs = vec![0.0, 1.0, 2.0];
        // For 0,1,2, median=1, IQR approx [0,2], so 1 -> sigmoid(0)=0.5.
        let y = normalize(&xs);
        assert!((y[1] - 0.5).abs() < 1e-9, "got {}", y[1]);
    }
}
