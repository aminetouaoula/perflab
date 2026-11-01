use anyhow::{Ok, Result};
use std::time::Instant;

use crate::{perf::perf, warmup::warm_up};
pub struct RunSample {
    pub cycles: Option<u64>,
    pub instructions: Option<u64>,
    pub cache_misses: Option<u64>,
    pub branch_misses: Option<u64>,
    pub wall_ms: f64,
}
pub fn measure(warmup: u8, n: u16, args: &[String]) -> Result<Vec<RunSample>> {
    let mut samples_holder: Vec<RunSample> = vec![];
    warm_up(warmup, args)?;
    for _ in 0..n {
        let start = Instant::now();
        let result = perf(args)?;
        let sample: RunSample = RunSample {
            cycles: result.get("cycles").copied().flatten(),
            instructions: result.get("instructions").copied().flatten(),
            cache_misses: result.get("cache-misses").copied().flatten(),
            branch_misses: result.get("branch-misses").copied().flatten(),
            wall_ms: start.elapsed().as_secs_f64() * 1000.0,
        };
        samples_holder.push(sample);
    }
    Ok(samples_holder)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn s(v: &str) -> String {
        v.to_string()
    }

    fn counter_file(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "perflab_measure_{}_{}.txt",
            name,
            std::process::id()
        ));
        let _ = fs::remove_file(&p);
        p
    }

    fn counting_args(path: &PathBuf) -> Vec<String> {
        vec![s("sh"), s("-c"), format!("echo run >> {}", path.display())]
    }

    fn lines_in(path: &PathBuf) -> usize {
        fs::read_to_string(path)
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }

    // ---------- expected behavior ----------

    #[test]
    fn returns_one_sample_per_measured_run() {
        let samples = measure(0, 3, &[s("true")]).unwrap();
        assert_eq!(samples.len(), 3);
    }

    #[test]
    fn zero_measured_runs_returns_empty_vec() {
        let samples = measure(0, 0, &[s("true")]).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn zero_warmup_and_one_run_is_ok() {
        let samples = measure(0, 1, &[s("true")]).unwrap();
        assert_eq!(samples.len(), 1);
    }

    #[test]
    fn warmup_runs_are_not_in_samples() {
        let samples = measure(5, 2, &[s("true")]).unwrap();
        assert_eq!(samples.len(), 2);
    }

    #[test]
    fn command_runs_warmup_plus_measured_times() {
        let path = counter_file("total");
        let args = counting_args(&path);
        let samples = measure(2, 3, &args).unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(lines_in(&path), 5);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn zero_warmup_runs_command_only_measured_times() {
        let path = counter_file("nowarm");
        let args = counting_args(&path);
        let samples = measure(0, 4, &args).unwrap();
        assert_eq!(samples.len(), 4);
        assert_eq!(lines_in(&path), 4);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn zero_measured_runs_still_runs_warmup() {
        let path = counter_file("warm_only");
        let args = counting_args(&path);
        let samples = measure(3, 0, &args).unwrap();
        assert!(samples.is_empty());
        assert_eq!(lines_in(&path), 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn cycles_are_some_for_real_command() {
        let samples = measure(0, 2, &[s("sleep"), s("0.05")]).unwrap();
        for sample in &samples {
            assert!(sample.cycles.is_some());
        }
    }

    #[test]
    fn all_four_counters_are_present_for_real_command() {
        let samples = measure(0, 2, &[s("sleep"), s("0.05")]).unwrap();
        for sample in &samples {
            assert!(sample.cycles.is_some());
            assert!(sample.instructions.is_some());
            assert!(sample.cache_misses.is_some());
            assert!(sample.branch_misses.is_some());
        }
    }

    #[test]
    fn cycles_are_positive_for_real_command() {
        let samples = measure(0, 2, &[s("sleep"), s("0.05")]).unwrap();
        for sample in &samples {
            assert!(sample.cycles.unwrap() > 0);
        }
    }

    #[test]
    fn wall_time_is_positive_in_milliseconds() {
        let samples = measure(0, 2, &[s("sleep"), s("0.1")]).unwrap();
        for sample in &samples {
            assert!(sample.wall_ms > 0.0);
        }
    }

    #[test]
    fn wall_time_reflects_sleep_duration() {
        let samples = measure(0, 2, &[s("sleep"), s("0.1")]).unwrap();
        for sample in &samples {
            assert!(sample.wall_ms >= 100.0, "measured {} ms", sample.wall_ms);
            assert!(sample.wall_ms < 2000.0, "measured {} ms", sample.wall_ms);
        }
    }

    #[test]
    fn longer_command_has_longer_wall_time() {
        let short = measure(0, 2, &[s("sleep"), s("0.05")]).unwrap();
        let long = measure(0, 2, &[s("sleep"), s("0.2")]).unwrap();
        let short_avg: f64 = short.iter().map(|x| x.wall_ms).sum::<f64>() / 2.0;
        let long_avg: f64 = long.iter().map(|x| x.wall_ms).sum::<f64>() / 2.0;
        assert!(long_avg > short_avg);
    }

    #[test]
    fn samples_keep_run_order_and_count() {
        let samples = measure(1, 5, &[s("true")]).unwrap();
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn large_measured_count_is_accepted() {
        let samples = measure(0, 50, &[s("true")]).unwrap();
        assert_eq!(samples.len(), 50);
    }

    #[test]
    fn command_with_arguments_is_measured() {
        let samples = measure(0, 2, &[s("sh"), s("-c"), s("exit 0")]).unwrap();
        assert_eq!(samples.len(), 2);
    }

    // ---------- error behavior ----------

    #[test]
    fn failing_measured_command_returns_error() {
        assert!(measure(0, 2, &[s("false")]).is_err());
    }

    #[test]
    fn nonzero_exit_returns_error() {
        assert!(measure(0, 2, &[s("sh"), s("-c"), s("exit 3")]).is_err());
    }

    #[test]
    fn failing_warmup_returns_error() {
        assert!(measure(2, 3, &[s("false")]).is_err());
    }

    #[test]
    fn failing_warmup_stops_before_measuring() {
        let path = counter_file("fail_warm");
        let args = vec![
            s("sh"),
            s("-c"),
            format!("echo run >> {}; exit 1", path.display()),
        ];
        assert!(measure(3, 3, &args).is_err());
        assert_eq!(lines_in(&path), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_program_returns_error() {
        assert!(measure(0, 1, &[s("definitely_not_a_real_command_xyz")]).is_err());
    }

    #[test]
    fn empty_args_returns_error() {
        assert!(measure(0, 1, &[]).is_err());
    }

    #[test]
    fn empty_args_with_zero_runs_is_still_ok() {
        // Nothing is executed, so there is nothing to fail.
        let samples = measure(0, 0, &[]).unwrap();
        assert!(samples.is_empty());
    }

    #[test]
    fn error_on_later_run_discards_earlier_samples() {
        // The command succeeds on the first call, then fails. Since measure
        // returns Result<Vec<_>>, the error replaces the whole vector.
        let path = counter_file("later_fail");
        let args = vec![
            s("sh"),
            s("-c"),
            format!(
                "n=$(wc -l < {p} 2>/dev/null || echo 0); echo run >> {p}; [ \"$n\" -lt 1 ]",
                p = path.display()
            ),
        ];
        assert!(measure(0, 3, &args).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn warmup_zero_with_failing_command_errors_on_measured_run() {
        assert!(measure(0, 1, &[s("false")]).is_err());
    }
}
