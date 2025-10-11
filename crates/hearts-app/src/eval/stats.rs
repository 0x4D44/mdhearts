// Statistical analysis functions for mixed evaluation
#![allow(dead_code)]

use super::mixed::EvalError;
use super::types::*;

/// Compute comparison between test policy and baseline policies
pub fn compute_comparison(
    results: &[GameResult],
    test_policy_index: usize,
) -> Result<ComparisonResults, EvalError> {
    if test_policy_index >= 4 {
        return Err(EvalError::InvalidConfig(format!(
            "test_policy_index {} out of range [0,3]",
            test_policy_index
        )));
    }

    // Extract per-game scores for test and baseline policies
    let mut test_scores: Vec<f64> = Vec::new();
    let mut baseline_scores: Vec<Vec<f64>> = vec![Vec::new(); 3];

    for game_result in results {
        test_scores.push(game_result.points[test_policy_index] as f64);

        let mut baseline_idx = 0;
        for policy_idx in 0..4 {
            if policy_idx != test_policy_index {
                baseline_scores[baseline_idx].push(game_result.points[policy_idx] as f64);
                baseline_idx += 1;
            }
        }
    }

    // Compute averages
    let test_avg = mean(&test_scores);
    let baseline_avg = mean(
        &baseline_scores
            .iter()
            .flat_map(|scores| scores.iter())
            .copied()
            .collect::<Vec<f64>>(),
    );

    // Compute difference and percent improvement
    let difference = test_avg - baseline_avg;
    let percent_improvement = if baseline_avg > 0.0 {
        (baseline_avg - test_avg) / baseline_avg * 100.0
    } else {
        0.0
    };

    // Statistical significance: Mann-Whitney U test
    // Compare test scores against pooled baseline scores
    let pooled_baseline: Vec<f64> = baseline_scores
        .iter()
        .flat_map(|scores| scores.iter())
        .copied()
        .collect();

    let statistical_significance = if test_scores.len() >= 20 && pooled_baseline.len() >= 20 {
        Some(mann_whitney_u_test(&test_scores, &pooled_baseline))
    } else {
        None
    };

    Ok(ComparisonResults {
        test_policy_index,
        test_avg,
        baseline_avg,
        difference,
        percent_improvement,
        statistical_significance,
        statistical_test: "mann_whitney_u".to_string(),
    })
}

/// Mann-Whitney U test (non-parametric test for comparing two samples)
/// Returns p-value (two-tailed)
///
/// This test does NOT assume normal distribution, making it appropriate for Hearts scores.
/// Uses normal approximation which is valid for n1, n2 >= 20.
fn mann_whitney_u_test(sample1: &[f64], sample2: &[f64]) -> f64 {
    let n1 = sample1.len();
    let n2 = sample2.len();

    if n1 < 20 || n2 < 20 {
        // Normal approximation not valid for small samples
        return 1.0; // Conservative: report no significance
    }

    // Combine and rank all observations
    let mut combined: Vec<(f64, usize)> = Vec::new();
    for &val in sample1 {
        combined.push((val, 1)); // Group 1
    }
    for &val in sample2 {
        combined.push((val, 2)); // Group 2
    }

    // Sort by value
    combined.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Assign ranks (handle ties with average rank)
    let mut ranks: Vec<(f64, usize)> = Vec::new();
    let mut i = 0;
    while i < combined.len() {
        let mut j = i;
        // Find all tied values
        while j < combined.len() && combined[j].0 == combined[i].0 {
            j += 1;
        }

        // Average rank for tied values
        let rank = ((i + 1) + j) as f64 / 2.0;

        // Assign average rank to all tied values
        for item in combined.iter().take(j).skip(i) {
            ranks.push((rank, item.1));
        }

        i = j;
    }

    // Sum of ranks for sample 1
    let r1: f64 = ranks
        .iter()
        .filter(|(_, group)| *group == 1)
        .map(|(rank, _)| rank)
        .sum();

    // Compute U statistic for sample 1
    let u1 = r1 - (n1 * (n1 + 1)) as f64 / 2.0;

    // Compute U statistic for sample 2
    let u2 = (n1 * n2) as f64 - u1;

    // Use smaller U for test statistic
    let u = u1.min(u2);

    // Mean and standard deviation under null hypothesis
    let mean_u = (n1 * n2) as f64 / 2.0;
    let std_u = ((n1 * n2 * (n1 + n2 + 1)) as f64 / 12.0).sqrt();

    // Z-score
    let z = (u - mean_u) / std_u;

    // Two-tailed p-value
    2.0 * (1.0 - standard_normal_cdf(z.abs()))
}

/// Standard normal cumulative distribution function
/// P(Z <= x) where Z ~ N(0,1)
fn standard_normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Error function approximation
/// Uses Abramowitz and Stegun approximation (maximum error: 1.5e-7)
/// Reference: Handbook of Mathematical Functions, formula 7.1.26
fn erf(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

/// Compute arithmetic mean
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(mean(&[5.0]), 5.0);
    }

    #[test]
    fn test_erf() {
        // Known values
        assert!((erf(0.0) - 0.0).abs() < 1e-6);
        assert!((erf(1.0) - 0.8427).abs() < 1e-3);
        assert!((erf(-1.0) + 0.8427).abs() < 1e-3);
    }

    #[test]
    fn test_standard_normal_cdf() {
        // Known values
        assert!((standard_normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((standard_normal_cdf(1.96) - 0.975).abs() < 1e-3);
        assert!((standard_normal_cdf(-1.96) - 0.025).abs() < 1e-3);
    }

    #[test]
    fn test_mann_whitney_identical_samples() {
        let sample1 = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
            17.0, 18.0, 19.0, 20.0,
        ];
        let sample2 = sample1.clone();

        let p_value = mann_whitney_u_test(&sample1, &sample2);

        // Identical samples should have high p-value (no significant difference)
        assert!(p_value > 0.9);
    }

    #[test]
    fn test_mann_whitney_different_samples() {
        let sample1 = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
            17.0, 18.0, 19.0, 20.0,
        ];
        let sample2 = vec![
            21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0, 30.0, 31.0, 32.0, 33.0, 34.0,
            35.0, 36.0, 37.0, 38.0, 39.0, 40.0,
        ];

        let p_value = mann_whitney_u_test(&sample1, &sample2);

        // Clearly different samples should have low p-value (significant difference)
        assert!(p_value < 0.001);
    }

    #[test]
    fn test_compute_comparison() {
        // Create test data: test policy (idx=3) performs better than baseline
        let mut results = Vec::new();
        for _ in 0..100 {
            results.push(GameResult {
                points: [8, 8, 8, 5], // Test policy at index 3 gets 5, baseline gets 8
                moon_shooter: None,
            });
        }

        let comparison = compute_comparison(&results, 3).unwrap();

        assert_eq!(comparison.test_policy_index, 3);
        assert_eq!(comparison.test_avg, 5.0);
        assert_eq!(comparison.baseline_avg, 8.0);
        assert_eq!(comparison.difference, -3.0); // Negative is better in Hearts
        assert!((comparison.percent_improvement - 37.5).abs() < 0.1);
        assert!(comparison.statistical_significance.is_some());
        assert!(comparison.statistical_significance.unwrap() < 0.001);
    }
}
