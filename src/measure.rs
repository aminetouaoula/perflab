use crate::memory::mem_usage_reader;
use crate::{perf::perf, warmup::warm_up};
use anyhow::{Ok, Result};
use std::time::Instant;

pub struct RunSample {
    pub cycles: Option<u64>,
    pub instructions: Option<u64>,
    pub cache_misses: Option<u64>,
    pub branch_misses: Option<u64>,
    pub wall_ms: f64,
    pub peak_rss_kb: Option<u64>,
}
pub fn measure(warmup: u8, n: u16, args: &[String], core: Option<u32>) -> Result<Vec<RunSample>> {
    let mut samples_holder: Vec<RunSample> = vec![];
    warm_up(warmup, args, core)?;
    for _ in 0..n {
        let start = Instant::now();
        let result = perf(args, core)?;
        let wll_ms = start.elapsed().as_secs_f64() * 1000.0;
        let sample: RunSample = RunSample {
            cycles: result.get("cycles").copied().flatten(),
            instructions: result.get("instructions").copied().flatten(),
            cache_misses: result.get("cache-misses").copied().flatten(),
            branch_misses: result.get("branch-misses").copied().flatten(),
            wall_ms: wll_ms,
            peak_rss_kb: Some(mem_usage_reader(args, core)?),
        };
        samples_holder.push(sample);
    }
    Ok(samples_holder)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    fn s(v: &str) -> String {
        v.to_string()
    }

    // Each run of the measured command appends one line to a unique file.
    // Every perf call and every memory call runs the command once, so
    // line counts show exactly how many times the command ran.
    fn counter_file(name: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "perflab_measure_{}_{}_{}.txt",
            name,
            std::process::id(),
            id
        ));
        let _ = fs::remove_file(&p);
        p
    }

    fn counting_args(path: &Path) -> Vec<String> {
        vec![s("sh"), s("-c"), format!("echo run >> {}", path.display())]
    }

    // A command that appends a line and then fails, to test error propagation.
    fn counting_failing_args(path: &Path) -> Vec<String> {
        vec![
            s("sh"),
            s("-c"),
            format!("echo run >> {}; exit 1", path.display()),
        ]
    }

    fn lines_in(path: &Path) -> usize {
        fs::read_to_string(path)
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }

    // ---------- guards and failures: run everywhere ----------

    #[test]
    fn zero_measured_runs_with_zero_warmup_returns_empty() {
        // Nothing runs, so no perf call is made and nothing can fail.
        let samples = measure(0, 0, &[s("true")], None).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn zero_measured_runs_with_empty_args_is_ok() {
        let samples = measure(0, 0, &[], None).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn empty_args_with_measured_runs_is_error() {
        assert!(measure(0, 1, &[], None).is_err());
    }

    #[test]
    fn missing_program_is_error() {
        assert!(measure(0, 1, &[s("definitely_not_a_real_command_xyz")], None).is_err());
    }

    #[test]
    fn missing_program_with_warmup_is_error() {
        assert!(measure(2, 1, &[s("definitely_not_a_real_command_xyz")], None).is_err());
    }

    #[test]
    fn failing_measured_command_is_error() {
        assert!(measure(0, 2, &[s("false")], None).is_err());
    }

    #[test]
    fn nonzero_exit_is_error() {
        assert!(measure(0, 2, &[s("sh"), s("-c"), s("exit 3")], None).is_err());
    }

    #[test]
    fn failing_warmup_is_error() {
        assert!(measure(2, 3, &[s("false")], None).is_err());
    }

    #[test]
    fn impossible_core_is_error() {
        assert!(measure(0, 1, &[s("true")], Some(9999)).is_err());
    }

    #[test]
    fn impossible_core_with_zero_runs_is_ok() {
        // No perf or taskset call happens, so the core is never checked.
        let samples = measure(0, 0, &[s("true")], Some(9999)).unwrap();
        assert!(samples.is_empty());
    }

    // ---------- expected counts (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn returns_one_sample_per_measured_run() {
        let samples = measure(0, 3, &[s("true")], None).unwrap();
        assert_eq!(samples.len(), 3);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn warmup_runs_are_not_in_samples() {
        let samples = measure(5, 2, &[s("true")], None).unwrap();
        assert_eq!(samples.len(), 2);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn zero_warmup_runs_command_twice_per_measured_run() {
        // Each measured run calls perf once and memory measurement once.
        let path = counter_file("zero_warm");
        let samples = measure(0, 4, &counting_args(&path), None).unwrap();
        assert_eq!(samples.len(), 4);
        assert_eq!(lines_in(&path), 2 * 4);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn warmup_runs_command_once_each_and_measured_runs_twice() {
        // Warm-up calls perf only, so each warm-up run is one execution.
        let path = counter_file("total");
        let samples = measure(2, 3, &counting_args(&path), None).unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(lines_in(&path), 2 + 2 * 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn zero_measured_runs_still_runs_warmup() {
        let path = counter_file("warm_only");
        let samples = measure(3, 0, &counting_args(&path), None).unwrap();
        assert!(samples.is_empty());
        assert_eq!(lines_in(&path), 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn failure_on_first_warmup_stops_before_any_measured_run() {
        let path = counter_file("fail_warm");
        assert!(measure(3, 3, &counting_failing_args(&path), None).is_err());
        assert_eq!(lines_in(&path), 1);
        let _ = fs::remove_file(&path);
    }

    // ---------- sample contents (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn every_counter_is_present_for_real_command() {
        let samples = measure(0, 2, &[s("sleep"), s("0.05")], None).unwrap();
        for sample in &samples {
            assert!(sample.cycles.is_some());
            assert!(sample.instructions.is_some());
            assert!(sample.cache_misses.is_some());
            assert!(sample.branch_misses.is_some());
        }
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn cycles_are_positive_for_real_command() {
        let samples = measure(0, 2, &[s("sleep"), s("0.05")], None).unwrap();
        for sample in &samples {
            assert!(sample.cycles.unwrap() > 0);
        }
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn peak_memory_is_present_and_positive() {
        let samples = measure(0, 2, &[s("true")], None).unwrap();
        for sample in &samples {
            assert!(sample.peak_rss_kb.unwrap() > 0);
        }
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn wall_time_is_positive_in_milliseconds() {
        let samples = measure(0, 2, &[s("sleep"), s("0.1")], None).unwrap();
        for sample in &samples {
            assert!(sample.wall_ms > 0.0);
        }
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn wall_time_reflects_one_sleep_not_two() {
        // The memory run is outside the timer, so wall time should be about
        // one sleep, not two. If the memory run were timed, this would be ~400 ms.
        let samples = measure(0, 1, &[s("sleep"), s("0.2")], None).unwrap();
        let ms = samples[0].wall_ms;
        assert!(ms >= 200.0, "measured {ms} ms");
        assert!(ms < 350.0, "memory run seems to be timed: {ms} ms");
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn longer_command_has_longer_wall_time() {
        let short = measure(0, 2, &[s("sleep"), s("0.05")], None).unwrap();
        let long = measure(0, 2, &[s("sleep"), s("0.2")], None).unwrap();
        let avg = |v: &[RunSample]| v.iter().map(|x| x.wall_ms).sum::<f64>() / v.len() as f64;
        assert!(avg(&long) > avg(&short));
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn sample_order_matches_run_count() {
        let samples = measure(1, 5, &[s("true")], None).unwrap();
        assert_eq!(samples.len(), 5);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn command_with_arguments_is_measured() {
        let samples = measure(0, 2, &[s("sh"), s("-c"), s("exit 0")], None).unwrap();
        assert_eq!(samples.len(), 2);
    }

    // ---------- pinned path (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn pinned_run_counts_match_unpinned() {
        let path = counter_file("pinned_count");
        let samples = measure(1, 2, &counting_args(&path), Some(0)).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(lines_in(&path), 1 + 2 * 2);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn pinned_samples_have_counters_and_memory() {
        let samples = measure(0, 2, &[s("sleep"), s("0.05")], Some(0)).unwrap();
        for sample in &samples {
            assert!(sample.cycles.is_some());
            assert!(sample.peak_rss_kb.is_some());
        }
    }
}
