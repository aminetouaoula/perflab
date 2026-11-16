use anyhow::Result;

use crate::perf::perf;
pub fn warm_up(n: u8, args: &[String], core: Option<u32>) -> Result<()> {
    for _ in 0..n {
        perf(args, core)?;
    }
    Ok(())
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

    // Each run of the command appends one line to a unique file, so the line
    // count shows how many times warm-up actually executed the command.
    fn counter_file(name: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "perflab_warmup_{}_{}_{}.txt",
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
    fn zero_runs_is_ok_and_calls_nothing() {
        assert!(warm_up(0, &[s("true")], None).is_ok());
    }

    #[test]
    fn zero_runs_with_empty_args_is_ok() {
        // No perf call happens, so there is nothing to fail.
        assert!(warm_up(0, &[], None).is_ok());
    }

    #[test]
    fn zero_runs_with_failing_command_is_ok() {
        assert!(warm_up(0, &[s("false")], None).is_ok());
    }

    #[test]
    fn zero_runs_with_impossible_core_is_ok() {
        // No perf or taskset call happens, so the core is never checked.
        assert!(warm_up(0, &[s("true")], Some(9999)).is_ok());
    }

    #[test]
    fn empty_args_is_error() {
        assert!(warm_up(1, &[], None).is_err());
    }

    #[test]
    fn missing_program_is_error() {
        assert!(warm_up(1, &[s("definitely_not_a_real_command_xyz")], None).is_err());
    }

    #[test]
    fn failing_command_is_error() {
        assert!(warm_up(1, &[s("false")], None).is_err());
    }

    #[test]
    fn nonzero_exit_of_measured_program_is_error() {
        assert!(warm_up(2, &[s("sh"), s("-c"), s("exit 3")], None).is_err());
    }

    #[test]
    fn failing_command_fails_on_first_iteration() {
        // The error comes from the first call, so later iterations never run.
        // The error still surfaces even with many iterations requested.
        assert!(warm_up(u8::MAX, &[s("false")], None).is_err());
    }

    #[test]
    fn impossible_core_is_error() {
        assert!(warm_up(1, &[s("true")], Some(9999)).is_err());
    }

    #[test]
    fn impossible_core_error_names_perf() {
        let err = warm_up(1, &[s("true")], Some(9999)).unwrap_err();
        assert!(!format!("{err:#}").is_empty());
    }

    #[test]
    fn maximum_warmup_count_with_bad_command_fails_fast() {
        // u8::MAX runs requested, but the first failure stops the loop.
        assert!(warm_up(u8::MAX, &[s("false")], None).is_err());
    }

    // ---------- expected behavior: counts (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn one_run_executes_command_once() {
        let path = counter_file("one");
        assert!(warm_up(1, &counting_args(&path), None).is_ok());
        assert_eq!(lines_in(&path), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn three_runs_execute_command_exactly_three_times() {
        let path = counter_file("three");
        assert!(warm_up(3, &counting_args(&path), None).is_ok());
        assert_eq!(lines_in(&path), 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn successful_command_returns_ok() {
        assert!(warm_up(2, &[s("true")], None).is_ok());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn warm_up_returns_unit_on_success() {
        let result: Result<()> = warm_up(1, &[s("true")], None);
        assert!(result.is_ok());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn failure_on_later_run_stops_the_loop() {
        // Fails on every run, so only the first run should execute.
        let path = counter_file("stop_early");
        assert!(warm_up(3, &counting_failing_args(&path), None).is_err());
        assert_eq!(lines_in(&path), 1);
        let _ = fs::remove_file(&path);
    }

    // ---------- pinned path (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn pinned_runs_execute_command_same_number_of_times() {
        let path = counter_file("pinned");
        assert!(warm_up(3, &counting_args(&path), Some(0)).is_ok());
        assert_eq!(lines_in(&path), 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn pinned_zero_runs_is_ok() {
        assert!(warm_up(0, &[s("true")], Some(0)).is_ok());
    }
}
