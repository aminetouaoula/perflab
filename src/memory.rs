use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};
use wait4::Wait4;
pub fn mem_usage_reader(args: &[String], core: Option<u32>) -> Result<u64> {
    let (program, rest) = args.split_first().context("no command given to measure")?;

    let label = match core {
        Some(_) => "taskset",
        None => program.as_str(),
    };

    let mut cmd = match core {
        Some(c) => {
            let mut t = Command::new("taskset");
            t.arg("-c").arg(c.to_string()).arg(program);
            t
        }
        None => Command::new(program),
    };

    let child = cmd
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(if core.is_some() {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .spawn()
        .with_context(|| format!("failed to start {program}"))?;
    let result = child.wait4()?;
    if !result.status.success() {
        bail!("{label} exited with {}", result.status);
    }
    Ok(result.rusage.maxrss / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    // ---------- guards and failures: run everywhere ----------

    #[test]
    fn empty_args_is_error() {
        assert!(mem_usage_reader(&[], None).is_err());
    }

    #[test]
    fn empty_args_error_says_no_command() {
        let err = mem_usage_reader(&[], None).unwrap_err();
        assert!(format!("{err:#}").contains("no command given"));
    }

    #[test]
    fn empty_args_with_core_is_error() {
        assert!(mem_usage_reader(&[], Some(0)).is_err());
    }

    #[test]
    fn empty_program_name_is_error() {
        assert!(mem_usage_reader(&[s("")], None).is_err());
    }

    #[test]
    fn missing_program_is_error() {
        assert!(mem_usage_reader(&[s("definitely_not_a_real_command_xyz")], None).is_err());
    }

    #[test]
    fn missing_program_error_names_it_and_says_start_failed() {
        let err = mem_usage_reader(&[s("definitely_not_a_real_command_xyz")], None).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("failed to start"), "message was: {msg}");
        assert!(
            msg.contains("definitely_not_a_real_command_xyz"),
            "message was: {msg}"
        );
    }

    #[test]
    fn missing_path_is_error() {
        assert!(mem_usage_reader(&[s("./no/such/binary")], None).is_err());
    }

    #[test]
    fn false_is_error() {
        assert!(mem_usage_reader(&[s("false")], None).is_err());
    }

    #[test]
    fn failure_message_names_the_program_when_unpinned() {
        let err = mem_usage_reader(&[s("false")], None).unwrap_err();
        assert!(format!("{err:#}").contains("false exited with"));
    }

    #[test]
    fn nonzero_exit_is_error_with_status() {
        let err = mem_usage_reader(&[s("sh"), s("-c"), s("exit 3")], None).unwrap_err();
        assert!(format!("{err:#}").contains("exited with"));
    }

    #[test]
    fn impossible_core_is_error() {
        assert!(mem_usage_reader(&[s("true")], Some(9999)).is_err());
    }

    #[test]
    fn impossible_core_error_names_taskset() {
        let err = mem_usage_reader(&[s("true")], Some(9999)).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("taskset"), "message was: {msg}");
    }

    #[test]
    fn pinned_failing_command_reports_exit_status() {
        let err = mem_usage_reader(&[s("sh"), s("-c"), s("exit 3")], Some(0)).unwrap_err();
        assert!(format!("{err:#}").contains("exited with"));
    }

    // ---------- expected behavior: unpinned ----------

    #[test]
    fn true_returns_ok() {
        assert!(mem_usage_reader(&[s("true")], None).is_ok());
    }

    #[test]
    fn true_reports_positive_peak_memory() {
        let kb = mem_usage_reader(&[s("true")], None).unwrap();
        assert!(kb > 0, "expected a positive value, got {kb}");
    }

    #[test]
    fn value_is_in_kilobytes_not_bytes() {
        // A byte count for true would be in the millions. In kilobytes it is
        // a few thousand, even with the inherited floor from the test process.
        let kb = mem_usage_reader(&[s("true")], None).unwrap();
        assert!(
            kb >= 500,
            "suspiciously small, possibly wrong unit: {kb} KB"
        );
        assert!(kb < 1_000_000, "unexpectedly large: {kb} KB");
    }

    #[test]
    fn repeated_small_runs_are_consistent() {
        let a = mem_usage_reader(&[s("true")], None).unwrap();
        let b = mem_usage_reader(&[s("true")], None).unwrap();
        assert!(a.abs_diff(b) < a / 2 + 1024, "a={a} KB, b={b} KB");
    }

    #[test]
    fn program_with_arguments_runs_successfully() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("exit 0")], None).is_ok());
    }

    #[test]
    fn extra_arguments_are_passed_to_program() {
        assert!(mem_usage_reader(&[s("true"), s("one"), s("two")], None).is_ok());
    }

    #[test]
    fn argument_with_spaces_stays_one_argument() {
        let r = mem_usage_reader(&[s("sh"), s("-c"), s("test \"a b\" = \"a b\"")], None);
        assert!(r.is_ok());
    }

    #[test]
    fn program_stdout_is_silenced_but_run_succeeds() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("echo noise; exit 0")], None).is_ok());
    }

    #[test]
    fn program_stderr_is_silenced_but_run_succeeds() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("echo oops >&2; exit 0")], None).is_ok());
    }

    // ---------- expected behavior: pinned ----------

    #[test]
    fn pinned_true_returns_positive_value() {
        let kb = mem_usage_reader(&[s("true")], Some(0)).unwrap();
        assert!(kb > 0, "expected a positive value, got {kb}");
    }

    #[test]
    fn pinned_command_with_arguments_succeeds() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("exit 0")], Some(0)).is_ok());
    }

    #[test]
    fn pinned_and_unpinned_are_same_order_of_magnitude() {
        // Pinning should not change how much memory the program uses, so both
        // values should be close, sharing the same inherited floor.
        let unpinned = mem_usage_reader(&[s("true")], None).unwrap();
        let pinned = mem_usage_reader(&[s("true")], Some(0)).unwrap();
        assert!(
            pinned < unpinned * 2 && unpinned < pinned * 2,
            "unpinned={unpinned} KB, pinned={pinned} KB"
        );
    }

    // ---------- heavy allocations: run locally, not on CI ----------

    #[test]
    #[ignore = "allocates about 200 MB, run locally"]
    fn large_allocation_reports_more_than_trivial_program() {
        let trivial = mem_usage_reader(&[s("true")], None).unwrap();
        let large = mem_usage_reader(
            &[
                s("sh"),
                s("-c"),
                s("x=$(head -c 200000000 /dev/zero | tr '\\0' a); exit 0"),
            ],
            None,
        )
        .unwrap();
        assert!(large > trivial, "large={large} KB, trivial={trivial} KB");
    }

    #[test]
    #[ignore = "allocates about 200 MB, run locally"]
    fn small_run_after_large_run_reports_less_memory() {
        // If the value carried over between runs, the second value would be
        // as large as the first. This catches that.
        let big = mem_usage_reader(
            &[
                s("sh"),
                s("-c"),
                s("x=$(head -c 200000000 /dev/zero | tr '\\0' a); exit 0"),
            ],
            None,
        )
        .unwrap();
        let small = mem_usage_reader(&[s("true")], None).unwrap();
        assert!(
            small < big,
            "carry-over suspected: small={small} KB, big={big} KB"
        );
    }

    #[test]
    #[ignore = "allocates about 200 MB, run locally"]
    fn pinned_large_allocation_reports_child_not_wrapper() {
        // If taskset forked instead of exec-ing, wait4 would report its small
        // footprint. A large value here shows the child itself is measured.
        let pinned_large = mem_usage_reader(
            &[
                s("sh"),
                s("-c"),
                s("x=$(head -c 200000000 /dev/zero | tr '\\0' a); exit 0"),
            ],
            Some(0),
        )
        .unwrap();
        let trivial = mem_usage_reader(&[s("true")], None).unwrap();
        assert!(
            pinned_large > trivial * 10,
            "wrapper may be hiding the child: pinned_large={pinned_large} KB, trivial={trivial} KB"
        );
    }
}
