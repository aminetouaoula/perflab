use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};
use wait4::Wait4;
pub fn mem_usage_reader(args: &[String]) -> Result<u64> {
    let (program, rest) = args.split_first().context("no command given to measure")?;
    let child = Command::new(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(rest)
        .spawn()
        .with_context(|| format!("failed to start {program}"))?;
    let result = child.wait4()?;
    if !result.status.success() {
        bail!("{program} exited with {}", result.status);
    }
    Ok(result.rusage.maxrss / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn true_returns_ok() {
        assert!(mem_usage_reader(&[s("true")]).is_ok());
    }

    #[test]
    fn true_reports_positive_peak_memory() {
        let kb = mem_usage_reader(&[s("true")]).unwrap();
        assert!(kb > 0, "expected a positive value, got {kb}");
    }

    #[test]
    fn peak_memory_is_plausible_for_small_program() {
        // A trivial program should use well under 1 GB (about 1,000,000 KB).
        let kb = mem_usage_reader(&[s("true")]).unwrap();
        assert!(kb < 1_000_000, "unexpectedly large: {kb} KB");
    }

    #[test]
    fn program_with_arguments_runs_successfully() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("exit 0")]).is_ok());
    }

    #[test]
    fn extra_arguments_are_passed_to_program() {
        assert!(mem_usage_reader(&[s("true"), s("one"), s("two")]).is_ok());
    }

    #[test]
    fn argument_with_spaces_stays_one_argument() {
        let r = mem_usage_reader(&[s("sh"), s("-c"), s("test \"a b\" = \"a b\"")]);
        assert!(r.is_ok());
    }

    #[test]
    fn program_stdout_is_silenced_but_run_succeeds() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("echo noise; exit 0")]).is_ok());
    }

    #[test]
    fn program_stderr_is_silenced_but_run_succeeds() {
        assert!(mem_usage_reader(&[s("sh"), s("-c"), s("echo oops >&2; exit 0")]).is_ok());
    }

    // ---------- error behavior ----------

    #[test]
    fn false_returns_error() {
        assert!(mem_usage_reader(&[s("false")]).is_err());
    }

    #[test]
    fn nonzero_exit_returns_error_with_status() {
        let err = mem_usage_reader(&[s("sh"), s("-c"), s("exit 3")]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("exited with"), "message was: {msg}");
    }

    #[test]
    fn failure_message_names_the_program() {
        let err = mem_usage_reader(&[s("false")]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("false"), "message was: {msg}");
    }

    #[test]
    fn missing_program_returns_error() {
        assert!(mem_usage_reader(&[s("definitely_not_a_real_command_xyz")]).is_err());
    }

    #[test]
    fn missing_program_error_names_it_and_says_start_failed() {
        let err = mem_usage_reader(&[s("definitely_not_a_real_command_xyz")]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("failed to start"), "message was: {msg}");
        assert!(
            msg.contains("definitely_not_a_real_command_xyz"),
            "message was: {msg}"
        );
    }

    #[test]
    fn missing_path_returns_error() {
        assert!(mem_usage_reader(&[s("./no/such/binary")]).is_err());
    }

    #[test]
    fn empty_program_name_returns_error() {
        assert!(mem_usage_reader(&[s("")]).is_err());
    }

    #[test]
    fn empty_args_returns_error() {
        assert!(mem_usage_reader(&[]).is_err());
    }

    #[test]
    fn empty_args_error_says_no_command_given() {
        let err = mem_usage_reader(&[]).unwrap_err();
        assert!(format!("{err:#}").contains("no command given"));
    }

    // ---------- carry-over and consistency ----------

    #[test]
    fn repeated_small_runs_are_consistent() {
        let a = mem_usage_reader(&[s("true")]).unwrap();
        let b = mem_usage_reader(&[s("true")]).unwrap();
        assert!(a > 0 && b > 0);
        // Same program, same footprint: values should be close, not wildly different.
        let diff = a.abs_diff(b);
        assert!(diff < a / 2 + 1024, "a={a} KB, b={b} KB");
    }

    #[test]
    fn small_run_after_large_run_reports_less_memory() {
        // Allocates a large buffer first, then a trivial command.
        // If the value carries over between runs, the second value will be
        // as large as the first. This is the test that catches that problem.
        let big = mem_usage_reader(&[
            s("sh"),
            s("-c"),
            s("x=$(head -c 200000000 /dev/zero | tr '\\0' a); exit 0"),
        ])
        .unwrap();
        let small = mem_usage_reader(&[s("true")]).unwrap();
        assert!(
            small < big,
            "carry-over suspected: small={small} KB, big={big} KB"
        );
    }

    #[test]
    fn large_allocation_reports_more_than_trivial_program() {
        let trivial = mem_usage_reader(&[s("true")]).unwrap();
        let large = mem_usage_reader(&[
            s("sh"),
            s("-c"),
            s("x=$(head -c 200000000 /dev/zero | tr '\\0' a); exit 0"),
        ])
        .unwrap();
        assert!(large > trivial, "large={large} KB, trivial={trivial} KB");
    }
}
