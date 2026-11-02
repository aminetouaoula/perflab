use crate::measure::RunSample;
use crate::stats::median;

#[derive(Debug, Clone, Copy)]
pub enum Metric {
    Cycles,
    Instructions,
    CacheMisses,
    BranchMisses,
    WallMs,
}

impl Metric {
    pub fn value(&self, s: &RunSample) -> Option<f64> {
        match self {
            Metric::Cycles => s.cycles.map(|v| v as f64),
            Metric::Instructions => s.instructions.map(|v| v as f64),
            Metric::CacheMisses => s.cache_misses.map(|v| v as f64),
            Metric::BranchMisses => s.branch_misses.map(|v| v as f64),
            Metric::WallMs => Some(s.wall_ms),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Metric::Cycles => "cycles",
            Metric::Instructions => "instructions",
            Metric::CacheMisses => "cache-misses",
            Metric::BranchMisses => "branch-misses",
            Metric::WallMs => "wall_ms",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricSummary {
    pub median: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub valid: usize,
    pub total: usize,
}

pub fn metric_summary(samples: &[RunSample], metric: Metric) -> MetricSummary {
    let mut values: Vec<f64> = samples.iter().filter_map(|s| metric.value(s)).collect();

    let valid = values.len();
    let total = samples.len();
    let min = values.iter().copied().reduce(f64::min);
    let max = values.iter().copied().reduce(f64::max);
    let med = median(&mut values);

    MetricSummary {
        median: med,
        min,
        max,
        valid,
        total,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cycles: Option<u64>, wall_ms: f64) -> RunSample {
        RunSample {
            cycles,
            instructions: Some(100),
            cache_misses: Some(10),
            branch_misses: Some(5),
            wall_ms,
        }
    }

    fn full(cycles: u64) -> RunSample {
        RunSample {
            cycles: Some(cycles),
            instructions: Some(cycles * 2),
            cache_misses: Some(cycles / 10),
            branch_misses: Some(cycles / 100),
            wall_ms: cycles as f64,
        }
    }

    // ---------- Metric::value ----------

    #[test]
    fn value_returns_cycles_as_f64() {
        let s = full(1000);
        assert_eq!(Metric::Cycles.value(&s), Some(1000.0));
    }

    #[test]
    fn value_returns_none_when_counter_missing() {
        let s = sample(None, 1.0);
        assert_eq!(Metric::Cycles.value(&s), None);
    }

    #[test]
    fn value_zero_is_some_zero_not_none() {
        let s = sample(Some(0), 1.0);
        assert_eq!(Metric::Cycles.value(&s), Some(0.0));
    }

    #[test]
    fn value_instructions_reads_instructions_field() {
        let s = full(1000);
        assert_eq!(Metric::Instructions.value(&s), Some(2000.0));
    }

    #[test]
    fn value_cache_misses_reads_cache_misses_field() {
        let s = full(1000);
        assert_eq!(Metric::CacheMisses.value(&s), Some(100.0));
    }

    #[test]
    fn value_branch_misses_reads_branch_misses_field() {
        let s = full(1000);
        assert_eq!(Metric::BranchMisses.value(&s), Some(10.0));
    }

    #[test]
    fn value_wall_ms_is_always_some() {
        let s = sample(None, 12.5);
        assert_eq!(Metric::WallMs.value(&s), Some(12.5));
    }

    #[test]
    fn value_large_counter_keeps_precision_within_f64() {
        let big = 1u64 << 52;
        let s = sample(Some(big), 1.0);
        assert_eq!(Metric::Cycles.value(&s), Some(big as f64));
    }

    // ---------- Metric::name ----------

    #[test]
    fn name_for_each_metric() {
        assert_eq!(Metric::Cycles.name(), "cycles");
        assert_eq!(Metric::Instructions.name(), "instructions");
        assert_eq!(Metric::CacheMisses.name(), "cache-misses");
        assert_eq!(Metric::BranchMisses.name(), "branch-misses");
        assert_eq!(Metric::WallMs.name(), "wall_ms");
    }

    // ---------- metric_summary: expected behavior ----------

    #[test]
    fn all_runs_valid_odd_count() {
        let samples = vec![full(300), full(100), full(200)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.median, Some(200.0));
        assert_eq!(s.min, Some(100.0));
        assert_eq!(s.max, Some(300.0));
        assert_eq!(s.valid, 3);
        assert_eq!(s.total, 3);
    }

    #[test]
    fn all_runs_valid_even_count_averages_middle_pair() {
        let samples = vec![full(100), full(200), full(300), full(400)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.median, Some(250.0));
        assert_eq!(s.valid, 4);
        assert_eq!(s.total, 4);
    }

    #[test]
    fn input_order_does_not_change_result() {
        let a = metric_summary(&[full(300), full(100), full(200)], Metric::Cycles);
        let b = metric_summary(&[full(100), full(200), full(300)], Metric::Cycles);
        assert_eq!(a, b);
    }

    #[test]
    fn one_missing_value_lowers_valid_count() {
        let samples = vec![full(100), sample(None, 1.0), full(300)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.valid, 2);
        assert_eq!(s.total, 3);
        assert_eq!(s.median, Some(200.0));
        assert_eq!(s.min, Some(100.0));
        assert_eq!(s.max, Some(300.0));
    }

    #[test]
    fn missing_values_are_not_treated_as_zero() {
        let samples = vec![full(100), sample(None, 1.0), full(300)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.min, Some(100.0));
    }

    #[test]
    fn single_run_median_min_max_are_that_value() {
        let s = metric_summary(&[full(42)], Metric::Cycles);
        assert_eq!(s.median, Some(42.0));
        assert_eq!(s.min, Some(42.0));
        assert_eq!(s.max, Some(42.0));
        assert_eq!(s.valid, 1);
        assert_eq!(s.total, 1);
    }

    #[test]
    fn duplicate_values_are_handled() {
        let samples = vec![full(5), full(5), full(5), full(9)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.median, Some(5.0));
        assert_eq!(s.min, Some(5.0));
        assert_eq!(s.max, Some(9.0));
    }

    #[test]
    fn negative_wall_time_is_summarized_as_given() {
        let samples = vec![sample(Some(1), -2.0), sample(Some(1), 4.0)];
        let s = metric_summary(&samples, Metric::WallMs);
        assert_eq!(s.min, Some(-2.0));
        assert_eq!(s.max, Some(4.0));
        assert_eq!(s.median, Some(1.0));
    }

    #[test]
    fn wall_ms_is_summarized_from_every_run() {
        let samples = vec![sample(None, 10.0), sample(None, 20.0), sample(None, 30.0)];
        let s = metric_summary(&samples, Metric::WallMs);
        assert_eq!(s.median, Some(20.0));
        assert_eq!(s.valid, 3);
        assert_eq!(s.total, 3);
    }

    #[test]
    fn summary_only_reads_requested_metric() {
        let samples = vec![full(100), full(200)];
        let cycles = metric_summary(&samples, Metric::Cycles);
        let instructions = metric_summary(&samples, Metric::Instructions);
        assert_eq!(cycles.median, Some(150.0));
        assert_eq!(instructions.median, Some(300.0));
    }

    // ---------- metric_summary: edge cases ----------

    #[test]
    fn empty_samples_gives_no_statistics() {
        let s = metric_summary(&[], Metric::Cycles);
        assert_eq!(s.median, None);
        assert_eq!(s.min, None);
        assert_eq!(s.max, None);
        assert_eq!(s.valid, 0);
        assert_eq!(s.total, 0);
    }

    #[test]
    fn all_runs_missing_gives_no_statistics_but_keeps_total() {
        let samples = vec![sample(None, 1.0), sample(None, 2.0)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.median, None);
        assert_eq!(s.min, None);
        assert_eq!(s.max, None);
        assert_eq!(s.valid, 0);
        assert_eq!(s.total, 2);
    }

    #[test]
    fn only_one_valid_run_among_many() {
        let samples = vec![sample(None, 1.0), full(77), sample(None, 3.0)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.median, Some(77.0));
        assert_eq!(s.min, Some(77.0));
        assert_eq!(s.max, Some(77.0));
        assert_eq!(s.valid, 1);
        assert_eq!(s.total, 3);
    }

    #[test]
    fn zero_values_are_valid_and_counted() {
        let samples = vec![sample(Some(0), 1.0), sample(Some(0), 1.0)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.median, Some(0.0));
        assert_eq!(s.valid, 2);
    }

    #[test]
    fn valid_count_never_exceeds_total() {
        let samples = vec![full(1), sample(None, 1.0), full(2)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert!(s.valid <= s.total);
    }

    #[test]
    fn min_is_less_than_or_equal_to_median_and_max() {
        let samples = vec![full(9), full(1), full(5), full(7)];
        let s = metric_summary(&samples, Metric::Cycles);
        assert!(s.min.unwrap() <= s.median.unwrap());
        assert!(s.median.unwrap() <= s.max.unwrap());
    }

    #[test]
    fn large_number_of_runs_is_handled() {
        let samples: Vec<RunSample> = (1..=1000).map(full).collect();
        let s = metric_summary(&samples, Metric::Cycles);
        assert_eq!(s.valid, 1000);
        assert_eq!(s.median, Some(500.5));
        assert_eq!(s.min, Some(1.0));
        assert_eq!(s.max, Some(1000.0));
    }

    #[test]
    fn summary_derives_equality_for_tests() {
        let a = metric_summary(&[full(3)], Metric::Cycles);
        let b = metric_summary(&[full(3)], Metric::Cycles);
        assert_eq!(a, b);
    }
}
