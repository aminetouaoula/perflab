use anyhow::{Context, Result, bail};
use std::process::Command;
use std::time::Instant;

pub fn runner_once(cmd: &str, args: &[String]) -> Result<f64> {
    let start = Instant::now();
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("failed to start {cmd}"))?;
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    if !status.success() {
        bail!("{cmd} exited with {status}");
    }
    Ok(elapsed)
}

#[cfg(test)]
mod test {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn succeeds_on_zero_exit() {
        assert!(runner_once("true", &[]).is_ok());
    }

    #[test]
    fn fails_on_nonzero_exit() {
        assert!(runner_once("false", &[]).is_err());
    }

    #[test]
    fn fails_when_command_does_not_exist() {
        assert!(runner_once("definitely_not_a_real_command_xyz", &[]).is_err());
    }

    #[test]
    fn missing_command_error_mentions_start_failure() {
        let err = runner_once("definitely_not_a_real_command_xyz", &[]).unwrap_err();
        assert!(format!("{err:#}").contains("failed to start"));
    }

    #[test]
    fn nonzero_exit_error_mentions_exit_status() {
        let err = runner_once("false", &[]).unwrap_err();
        assert!(format!("{err:#}").contains("exited with"));
    }

    #[test]
    fn returns_non_negative_time() {
        let t = runner_once("true", &[]).unwrap();
        assert!(t >= 0.0);
    }

    #[test]
    fn measures_roughly_sleep_duration() {
        let t = runner_once("sleep", &[s("0.1")]).unwrap();
        assert!(t >= 100.0, "measured {t} ms");
        assert!(t < 1000.0, "measured {t} ms");
    }

    #[test]
    fn longer_sleep_takes_longer() {
        let short = runner_once("sleep", &[s("0.05")]).unwrap();
        let long = runner_once("sleep", &[s("0.2")]).unwrap();
        assert!(long > short);
    }

    #[test]
    fn args_are_passed_to_command() {
        assert!(runner_once("sh", &[s("-c"), s("exit 0")]).is_ok());
        assert!(runner_once("sh", &[s("-c"), s("exit 3")]).is_err());
    }

    #[test]
    fn empty_args_slice_is_accepted() {
        assert!(runner_once("true", &[]).is_ok());
    }

    #[test]
    fn args_with_spaces_stay_single_argument() {
        assert!(runner_once("sh", &[s("-c"), s("test \"a b\" = \"a b\"")]).is_ok());
    }

    #[test]
    fn extra_args_to_true_are_ignored() {
        assert!(runner_once("true", &[s("one"), s("two")]).is_ok());
    }

    #[test]
    fn stdout_noise_does_not_break_timing() {
        assert!(runner_once("sh", &[s("-c"), s("echo hello; exit 0")]).is_ok());
    }

    #[test]
    fn stderr_output_does_not_break_timing() {
        assert!(runner_once("sh", &[s("-c"), s("echo oops >&2; exit 0")]).is_ok());
    }

    #[test]
    fn empty_command_name_fails() {
        assert!(runner_once("", &[]).is_err());
    }

    #[test]
    fn command_with_path_that_does_not_exist_fails() {
        assert!(runner_once("./no/such/binary", &[]).is_err());
    }
}
