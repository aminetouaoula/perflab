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
            peak_rss_kb: Some(mem_usage_reader(args)?),
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

    fn s(v: &str) -> String {
        v.to_string()
    }

    // Each call appends one line to this file, so the number of lines
    // equals the number of times perf actually ran the command.
    fn counter_file(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "perflab_warmup_{}_{}.txt",
            name,
            std::process::id()
        ));
        let _ = fs::remove_file(&p);
        p
    }

    fn counting_args(path: &Path) -> Vec<String> {
        vec![s("sh"), s("-c"), format!("echo run >> {}", path.display())]
    }

    fn lines_in(path: &Path) -> usize {
        fs::read_to_string(path)
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }

    // ---------- expected behavior ----------

    #[test]
    fn zero_runs_returns_ok_and_runs_nothing() {
        let path = counter_file("zero");
        let args = counting_args(&path);
        assert!(warm_up(0, &args).is_ok());
        assert_eq!(lines_in(&path), 0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn one_run_executes_command_once() {
        let path = counter_file("one");
        let args = counting_args(&path);
        assert!(warm_up(1, &args).is_ok());
        assert_eq!(lines_in(&path), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn three_runs_executes_command_exactly_three_times() {
        let path = counter_file("three");
        let args = counting_args(&path);
        assert!(warm_up(3, &args).is_ok());
        assert_eq!(lines_in(&path), 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn successful_command_returns_ok() {
        assert!(warm_up(2, &[s("true")]).is_ok());
    }

    #[test]
    fn warm_up_returns_unit_on_success() {
        let result: Result<()> = warm_up(1, &[s("true")]);
        assert!(result.is_ok());
    }

    // ---------- error behavior ----------

    #[test]
    fn failing_command_returns_error() {
        assert!(warm_up(1, &[s("false")]).is_err());
    }

    #[test]
    fn nonzero_exit_of_measured_program_is_error() {
        assert!(warm_up(2, &[s("sh"), s("-c"), s("exit 3")]).is_err());
    }

    #[test]
    fn missing_program_is_error() {
        assert!(warm_up(1, &[s("definitely_not_a_real_command_xyz")]).is_err());
    }

    #[test]
    fn empty_args_is_error() {
        assert!(warm_up(1, &[]).is_err());
    }

    #[test]
    fn failure_stops_the_loop_early() {
        // The command fails on its first run, so perf is called only once,
        // not three times.
        let path = counter_file("stop_early");
        let args = vec![
            s("sh"),
            s("-c"),
            format!("echo run >> {}; exit 1", path.display()),
        ];
        assert!(warm_up(3, &args).is_err());
        assert_eq!(lines_in(&path), 1);
        let _ = fs::remove_file(&path);
    }

    // ---------- edge cases ----------

    #[test]
    fn zero_runs_with_empty_args_is_still_ok() {
        // No perf call happens, so there is nothing to fail.
        assert!(warm_up(0, &[]).is_ok());
    }

    #[test]
    fn zero_runs_with_failing_command_is_ok() {
        assert!(warm_up(0, &[s("false")]).is_ok());
    }
}
