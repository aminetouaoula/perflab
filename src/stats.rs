pub fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        values.sort_by(|a, b| a.total_cmp(b));
        let mid = values.len() / 2;
        // if leng is even
        if values.len().is_multiple_of(2) {
            let result = (values[mid - 1] + values[mid]) / 2.0;
            Some(result)
        } else {
            let result = values[mid];
            Some(result)
        }
    }
}
/// Advances a splitmix64 state and returns the next pseudo-random number.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Returns the value at the given fraction (0.0 to 1.0) of a sorted slice.
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let idx = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[idx]
}

/// Bootstrap confidence interval for the median.
pub fn bootstrap_median_ci(values: &[f64], resamples: u32, seed: u64) -> Option<(f64, f64)> {
    if values.is_empty() || resamples == 0 {
        return None;
    }

    let n = values.len();
    let mut state = seed;
    let mut buffer: Vec<f64> = Vec::with_capacity(n);
    let mut medians: Vec<f64> = Vec::with_capacity(resamples as usize);

    for _ in 0..resamples {
        buffer.clear();
        for _ in 0..n {
            let idx = (splitmix64(&mut state) % n as u64) as usize;
            buffer.push(values[idx]);
        }
        medians.push(median(&mut buffer)?);
    }

    medians.sort_by(|a, b| a.total_cmp(b));
    Some((percentile(&medians, 0.025), percentile(&medians, 0.975)))
}
#[cfg(test)]
mod tests {
    use super::*;

    // ---------- median ----------

    #[test]
    fn median_odd_count() {
        let mut v = vec![3.0, 1.0, 2.0];
        assert_eq!(median(&mut v), Some(2.0));
    }

    #[test]
    fn median_even_count_averages_middle_pair() {
        let mut v = vec![4.0, 1.0, 3.0, 2.0];
        assert_eq!(median(&mut v), Some(2.5));
    }

    #[test]
    fn median_empty_is_none() {
        let mut v: Vec<f64> = vec![];
        assert_eq!(median(&mut v), None);
    }

    #[test]
    fn median_single_value() {
        let mut v = vec![7.0];
        assert_eq!(median(&mut v), Some(7.0));
    }

    #[test]
    fn median_with_duplicates() {
        let mut v = vec![5.0, 5.0, 1.0, 5.0];
        assert_eq!(median(&mut v), Some(5.0));
    }

    #[test]
    fn median_negative_values() {
        let mut v = vec![-3.0, 0.0, -1.0];
        assert_eq!(median(&mut v), Some(-1.0));
    }

    #[test]
    fn median_sorts_its_input() {
        let mut v = vec![9.0, 1.0, 5.0];
        let _ = median(&mut v);
        assert_eq!(v, vec![1.0, 5.0, 9.0]);
    }

    // ---------- splitmix64 ----------

    #[test]
    fn splitmix_same_seed_gives_same_sequence() {
        let (mut a, mut b) = (42u64, 42u64);
        for _ in 0..100 {
            assert_eq!(splitmix64(&mut a), splitmix64(&mut b));
        }
    }

    #[test]
    fn splitmix_different_seeds_give_different_first_values() {
        let (mut a, mut b) = (1u64, 2u64);
        assert_ne!(splitmix64(&mut a), splitmix64(&mut b));
    }

    #[test]
    fn splitmix_advances_state() {
        let mut state = 0u64;
        let before = state;
        let _ = splitmix64(&mut state);
        assert_ne!(state, before);
    }

    #[test]
    fn splitmix_handles_zero_seed() {
        let mut state = 0u64;
        let first = splitmix64(&mut state);
        assert_ne!(first, 0);
    }

    #[test]
    fn splitmix_handles_max_seed_without_panic() {
        let mut state = u64::MAX;
        let _ = splitmix64(&mut state);
        let _ = splitmix64(&mut state);
    }

    // ---------- percentile ----------

    #[test]
    fn percentile_zero_is_first_element() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 0.0), 1.0);
    }

    #[test]
    fn percentile_one_is_last_element() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 1.0), 3.0);
    }

    #[test]
    fn percentile_middle_of_odd_slice() {
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 0.5), 2.0);
    }

    #[test]
    fn percentile_single_element_always_that_element() {
        assert_eq!(percentile(&[7.0], 0.0), 7.0);
        assert_eq!(percentile(&[7.0], 0.5), 7.0);
        assert_eq!(percentile(&[7.0], 1.0), 7.0);
    }

    #[test]
    fn percentile_2_5_and_97_5_of_ten_thousand_are_inside_range() {
        let sorted: Vec<f64> = (0..10_000).map(|i| i as f64).collect();
        let low = percentile(&sorted, 0.025);
        let high = percentile(&sorted, 0.975);
        assert!(low > 200.0 && low < 300.0, "low was {low}");
        assert!(high > 9_700.0 && high < 9_800.0, "high was {high}");
    }

    // ---------- bootstrap: expected behavior ----------

    #[test]
    fn bootstrap_identical_values_give_zero_width_interval() {
        let v = vec![5.0; 10];
        assert_eq!(bootstrap_median_ci(&v, 1000, 1), Some((5.0, 5.0)));
    }

    #[test]
    fn bootstrap_single_value_gives_that_value_on_both_ends() {
        assert_eq!(bootstrap_median_ci(&[3.5], 1000, 1), Some((3.5, 3.5)));
    }

    #[test]
    fn bootstrap_low_is_never_above_high() {
        let v: Vec<f64> = (1..=14).map(|i| i as f64).collect();
        let (low, high) = bootstrap_median_ci(&v, 2000, 7).unwrap();
        assert!(low <= high, "low={low}, high={high}");
    }

    #[test]
    fn bootstrap_interval_contains_median_for_well_behaved_data() {
        let v: Vec<f64> = (1..=14).map(|i| i as f64).collect();
        let mut copy = v.clone();
        let m = median(&mut copy).unwrap();
        let (low, high) = bootstrap_median_ci(&v, 5000, 11).unwrap();
        assert!(low <= m && m <= high, "median {m} not in [{low}, {high}]");
    }

    #[test]
    fn bootstrap_interval_stays_inside_data_range() {
        let v = vec![10.0, 12.0, 11.0, 13.0, 9.0, 14.0, 10.5];
        let (low, high) = bootstrap_median_ci(&v, 2000, 3).unwrap();
        assert!(
            low >= 9.0 && high <= 14.0,
            "[{low}, {high}] outside data range"
        );
    }

    #[test]
    fn bootstrap_tight_data_gives_narrower_interval_than_scattered_data() {
        let tight = vec![100.0, 100.5, 99.5, 100.2, 99.8, 100.1, 99.9];
        let scattered = vec![50.0, 150.0, 80.0, 120.0, 60.0, 140.0, 100.0];
        let (tl, th) = bootstrap_median_ci(&tight, 3000, 5).unwrap();
        let (sl, sh) = bootstrap_median_ci(&scattered, 3000, 5).unwrap();
        assert!(
            th - tl < sh - sl,
            "tight width {} vs scattered width {}",
            th - tl,
            sh - sl
        );
    }

    #[test]
    fn bootstrap_same_seed_gives_same_interval() {
        let v: Vec<f64> = (1..=14).map(|i| i as f64 * 1.5).collect();
        let a = bootstrap_median_ci(&v, 2000, 99);
        let b = bootstrap_median_ci(&v, 2000, 99);
        assert_eq!(a, b);
    }

    #[test]
    fn bootstrap_different_seeds_both_bracket_the_median() {
        let v: Vec<f64> = (1..=14).map(|i| i as f64).collect();
        let mut copy = v.clone();
        let m = median(&mut copy).unwrap();
        for seed in [1u64, 2, 3, 4, 5] {
            let (low, high) = bootstrap_median_ci(&v, 3000, seed).unwrap();
            assert!(
                low <= m && m <= high,
                "seed {seed}: [{low}, {high}] vs median {m}"
            );
        }
    }

    #[test]
    fn bootstrap_two_values_stay_between_them() {
        let (low, high) = bootstrap_median_ci(&[1.0, 3.0], 1000, 2).unwrap();
        assert!(low >= 1.0 && high <= 3.0);
    }

    #[test]
    fn bootstrap_with_one_resample_returns_a_point() {
        let v = vec![1.0, 2.0, 3.0];
        let (low, high) = bootstrap_median_ci(&v, 1, 4).unwrap();
        assert_eq!(low, high);
    }

    // ---------- bootstrap: edge cases ----------

    #[test]
    fn bootstrap_empty_values_is_none() {
        assert_eq!(bootstrap_median_ci(&[], 1000, 1), None);
    }

    #[test]
    fn bootstrap_zero_resamples_is_none() {
        assert_eq!(bootstrap_median_ci(&[1.0, 2.0, 3.0], 0, 1), None);
    }

    #[test]
    fn bootstrap_empty_and_zero_resamples_is_none() {
        assert_eq!(bootstrap_median_ci(&[], 0, 1), None);
    }

    #[test]
    fn bootstrap_negative_values_work() {
        let v = vec![-10.0, -12.0, -11.0, -9.0, -13.0];
        let (low, high) = bootstrap_median_ci(&v, 2000, 6).unwrap();
        assert!(low >= -13.0 && high <= -9.0 && low <= high);
    }

    #[test]
    fn bootstrap_large_values_do_not_overflow_or_lose_order() {
        let v = vec![1e15, 1e15 + 1.0, 1e15 + 2.0, 1e15 + 3.0];
        let (low, high) = bootstrap_median_ci(&v, 1000, 8).unwrap();
        assert!(low <= high);
        assert!(low >= 1e15 && high <= 1e15 + 3.0);
    }

    #[test]
    fn bootstrap_zeros_give_zero_interval() {
        assert_eq!(
            bootstrap_median_ci(&[0.0, 0.0, 0.0], 500, 1),
            Some((0.0, 0.0))
        );
    }

    #[test]
    fn bootstrap_very_few_runs_is_still_ordered() {
        let (low, high) = bootstrap_median_ci(&[1.0, 9.0, 4.0], 1000, 12).unwrap();
        assert!(low <= high);
    }

    #[test]
    fn bootstrap_many_runs_completes() {
        let v: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let (low, high) = bootstrap_median_ci(&v, 500, 13).unwrap();
        assert!(low <= high);
    }

    #[test]
    fn bootstrap_does_not_modify_input() {
        let v = vec![3.0, 1.0, 2.0];
        let before = v.clone();
        let _ = bootstrap_median_ci(&v, 100, 1);
        assert_eq!(v, before);
    }
}
