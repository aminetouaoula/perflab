use crate::{
    measure::RunSample,
    summary::{Metric, MetricSummary, metric_summary},
};
pub struct Summary {
    pub cycles: MetricSummary,
    pub instructions: MetricSummary,
    pub cache_misses: MetricSummary,
    pub branch_misses: MetricSummary,
    pub wall_ms: MetricSummary,
}
pub fn summarize_all(s: &[RunSample]) -> Summary {
    Summary {
        cycles: metric_summary(s, Metric::Cycles),
        instructions: metric_summary(s, Metric::Instructions),
        cache_misses: metric_summary(s, Metric::CacheMisses),
        branch_misses: metric_summary(s, Metric::BranchMisses),
        wall_ms: metric_summary(s, Metric::WallMs),
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

    // ---------- expected behavior ----------

    #[test]
    fn each_field_matches_direct_metric_summary_call() {
        let samples = vec![full(100), full(200), full(300)];
        let s = summarize_all(&samples);
        assert_eq!(s.cycles, metric_summary(&samples, Metric::Cycles));
        assert_eq!(
            s.instructions,
            metric_summary(&samples, Metric::Instructions)
        );
        assert_eq!(
            s.cache_misses,
            metric_summary(&samples, Metric::CacheMisses)
        );
        assert_eq!(
            s.branch_misses,
            metric_summary(&samples, Metric::BranchMisses)
        );
        assert_eq!(s.wall_ms, metric_summary(&samples, Metric::WallMs));
    }

    #[test]
    fn fields_are_not_swapped() {
        // Instructions are always twice cycles in full(), so swapped fields would show up here.
        let samples = vec![full(100), full(200), full(300)];
        let s = summarize_all(&samples);
        assert_eq!(s.cycles.median, Some(200.0));
        assert_eq!(s.instructions.median, Some(400.0));
        assert_eq!(s.cache_misses.median, Some(20.0));
        assert_eq!(s.branch_misses.median, Some(2.0));
        assert_eq!(s.wall_ms.median, Some(200.0));
    }

    #[test]
    fn valid_and_total_are_per_metric() {
        // Cycles is missing in one run, but the other counters are present in every run.
        let samples = vec![full(100), sample(None, 5.0), full(300)];
        let s = summarize_all(&samples);
        assert_eq!(s.cycles.valid, 2);
        assert_eq!(s.cycles.total, 3);
        assert_eq!(s.instructions.valid, 3);
        assert_eq!(s.instructions.total, 3);
        assert_eq!(s.wall_ms.valid, 3);
    }

    #[test]
    fn one_missing_metric_does_not_affect_others() {
        let samples = vec![sample(None, 1.0), sample(None, 2.0)];
        let s = summarize_all(&samples);
        assert_eq!(s.cycles.valid, 0);
        assert_eq!(s.cycles.median, None);
        assert_eq!(s.instructions.valid, 2);
        assert_eq!(s.instructions.median, Some(100.0));
        assert_eq!(s.wall_ms.median, Some(1.5));
    }

    #[test]
    fn single_run_produces_one_value_per_metric() {
        let s = summarize_all(&[full(40)]);
        assert_eq!(s.cycles.median, Some(40.0));
        assert_eq!(s.instructions.median, Some(80.0));
        assert_eq!(s.cache_misses.median, Some(4.0));
        assert_eq!(s.branch_misses.median, Some(0.0));
        assert_eq!(s.wall_ms.median, Some(40.0));
        assert_eq!(s.cycles.valid, 1);
        assert_eq!(s.wall_ms.total, 1);
    }

    // ---------- edge cases ----------

    #[test]
    fn empty_input_gives_empty_summary_for_every_metric() {
        let s = summarize_all(&[]);
        for m in [
            &s.cycles,
            &s.instructions,
            &s.cache_misses,
            &s.branch_misses,
            &s.wall_ms,
        ] {
            assert_eq!(m.median, None);
            assert_eq!(m.min, None);
            assert_eq!(m.max, None);
            assert_eq!(m.valid, 0);
            assert_eq!(m.total, 0);
        }
    }

    #[test]
    fn all_counters_missing_leaves_wall_time_intact() {
        let samples = vec![
            RunSample {
                cycles: None,
                instructions: None,
                cache_misses: None,
                branch_misses: None,
                wall_ms: 7.0,
            },
            RunSample {
                cycles: None,
                instructions: None,
                cache_misses: None,
                branch_misses: None,
                wall_ms: 9.0,
            },
        ];
        let s = summarize_all(&samples);
        assert_eq!(s.cycles.valid, 0);
        assert_eq!(s.instructions.valid, 0);
        assert_eq!(s.cache_misses.valid, 0);
        assert_eq!(s.branch_misses.valid, 0);
        assert_eq!(s.wall_ms.valid, 2);
        assert_eq!(s.wall_ms.median, Some(8.0));
    }

    #[test]
    fn total_is_the_same_for_every_metric() {
        let samples = vec![full(1), sample(None, 2.0), full(3), sample(None, 4.0)];
        let s = summarize_all(&samples);
        assert_eq!(s.cycles.total, 4);
        assert_eq!(s.instructions.total, 4);
        assert_eq!(s.cache_misses.total, 4);
        assert_eq!(s.branch_misses.total, 4);
        assert_eq!(s.wall_ms.total, 4);
    }

    #[test]
    fn summarize_does_not_modify_input() {
        let samples = vec![full(300), full(100), full(200)];
        let before: Vec<Option<u64>> = samples.iter().map(|s| s.cycles).collect();
        let _ = summarize_all(&samples);
        let after: Vec<Option<u64>> = samples.iter().map(|s| s.cycles).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn large_run_count_is_summarized_for_every_metric() {
        let samples: Vec<RunSample> = (1..=100).map(full).collect();
        let s = summarize_all(&samples);
        assert_eq!(s.cycles.valid, 100);
        assert_eq!(s.cycles.total, 100);
        assert_eq!(s.wall_ms.median, Some(50.5));
    }
}
