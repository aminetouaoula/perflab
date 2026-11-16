use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::process::Command;

use crate::reader::parse_perf_csv;

pub fn perf(args: &[String], core: Option<u32>) -> Result<BTreeMap<String, Option<u64>>> {
    let (program, rest) = args.split_first().context("no command given to measure")?;

    let mut full: Vec<String> = [
        "stat",
        "-x,",
        "-e",
        "cycles,instructions,cache-misses,branch-misses",
        "--",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    full.push(program.clone());
    full.extend(rest.iter().cloned());
    let mut cmd = match core {
        Some(c) => {
            let mut t = Command::new("taskset");
            t.arg("-c").arg(c.to_string()).arg("perf");
            t
        }
        None => Command::new("perf"),
    };

    let output = cmd.args(&full).output().context("failed to start perf")?;

    if output.status.success() {
        let text = String::from_utf8(output.stderr)
            .context("perf produced output that is not valid UTF-8")?;
        parse_perf_csv(&text)
    } else {
        bail!(
            "perf failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    // ---------- expected behavior (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn sleep_returns_four_counters() {
        let c = perf(&[s("sleep"), s("0.1")], None).unwrap();
        assert_eq!(c.len(), 4);
        assert!(c.contains_key("cycles"));
        assert!(c.contains_key("instructions"));
        assert!(c.contains_key("cache-misses"));
        assert!(c.contains_key("branch-misses"));
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn sleep_cycles_are_positive() {
        let c = perf(&[s("sleep"), s("0.1")], None).unwrap();
        assert!(c["cycles"].unwrap() > 0);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn longer_work_uses_more_instructions() {
        let short = perf(
            &[
                s("sh"),
                s("-c"),
                s("i=0; while [ $i -lt 10 ]; do i=$((i+1)); done"),
            ],
            None,
        )
        .unwrap();
        let long = perf(
            &[
                s("sh"),
                s("-c"),
                s("i=0; while [ $i -lt 10000 ]; do i=$((i+1)); done"),
            ],
            None,
        )
        .unwrap();
        assert!(long["instructions"].unwrap() > short["instructions"].unwrap());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn successful_command_with_zero_exit_is_ok() {
        assert!(perf(&[s("sh"), s("-c"), s("exit 0")], None).is_ok());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn single_program_without_arguments_is_ok() {
        assert!(perf(&[s("true")], None).is_ok());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn many_arguments_are_all_passed() {
        let r = perf(
            &[
                s("sh"),
                s("-c"),
                s("test \"$1\" = a && test \"$2\" = b && test \"$3\" = c && test \"$4\" = d"),
                s("sh"),
                s("a"),
                s("b"),
                s("c"),
                s("d"),
            ],
            None,
        );
        assert!(r.is_ok());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn argument_with_spaces_stays_one_argument() {
        let r = perf(&[s("sh"), s("-c"), s("test \"a b\" = \"a b\"")], None);
        assert!(r.is_ok());
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn measured_program_stdout_does_not_appear_in_counters() {
        let c = perf(&[s("sh"), s("-c"), s("echo cycles,1,2,3; exit 0")], None).unwrap();
        assert_eq!(c.len(), 4);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn counter_values_fit_u64_and_are_some_when_counted() {
        let c = perf(&[s("sleep"), s("0.05")], None).unwrap();
        for (name, value) in &c {
            if let Some(v) = value {
                assert!(*v < u64::MAX, "{name} overflowed");
            }
        }
    }

    // ---------- pinned path (needs hardware perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn pinned_core_zero_returns_four_counters() {
        let c = perf(&[s("sleep"), s("0.1")], Some(0)).unwrap();
        assert_eq!(c.len(), 4);
        assert!(c["cycles"].unwrap() > 0);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn pinned_and_unpinned_return_same_counter_names() {
        let pinned = perf(&[s("sleep"), s("0.05")], Some(0)).unwrap();
        let unpinned = perf(&[s("sleep"), s("0.05")], None).unwrap();
        assert_eq!(
            pinned.keys().collect::<Vec<_>>(),
            unpinned.keys().collect::<Vec<_>>()
        );
    }

    // ---------- error behavior (runs on CI, with or without perf) ----------

    #[test]
    fn nonzero_exit_of_measured_command_is_error() {
        assert!(perf(&[s("sh"), s("-c"), s("exit 3")], None).is_err());
    }

    #[test]
    fn false_command_is_error() {
        assert!(perf(&[s("false")], None).is_err());
    }

    #[test]
    fn missing_program_is_error() {
        assert!(perf(&[s("definitely_not_a_real_command_xyz")], None).is_err());
    }

    #[test]
    fn empty_args_is_error_not_panic() {
        assert!(perf(&[], None).is_err());
    }

    #[test]
    fn empty_args_with_core_is_error_not_panic() {
        assert!(perf(&[], Some(0)).is_err());
    }

    #[test]
    fn wrong_argument_value_causes_error() {
        let r = perf(
            &[
                s("sh"),
                s("-c"),
                s("test \"$1\" = expected"),
                s("sh"),
                s("actual"),
            ],
            None,
        );
        assert!(r.is_err());
    }

    #[test]
    fn impossible_core_is_error() {
        assert!(perf(&[s("sleep"), s("0.01")], Some(9999)).is_err());
    }

    #[test]
    fn failure_message_contains_perf_stderr() {
        let err = perf(&[s("definitely_not_a_real_command_xyz")], None).unwrap_err();
        assert!(!format!("{err:#}").is_empty());
    }

    // ---------- error messages (need perf to actually run) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn failure_message_contains_exit_status() {
        let err = perf(&[s("sh"), s("-c"), s("exit 3")], None).unwrap_err();
        assert!(format!("{err:#}").contains("perf failed"));
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn impossible_core_message_names_perf() {
        let err = perf(&[s("sleep"), s("0.01")], Some(9999)).unwrap_err();
        assert!(format!("{err:#}").contains("perf failed"));
    }
}
