pub mod measure;
pub mod memory;
pub mod perf;
pub mod reader;
pub mod report;
pub mod runner;
pub mod stats;
pub mod summarize_all;
pub mod summary;
pub mod warmup;
use crate::measure::measure;
use crate::report::{Report, write_json, write_markdown};
use crate::summarize_all::summarize_all;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub command: Vec<String>,
    pub warmup: u8,
    pub runs: u16,
    pub out_dir: PathBuf,
    pub core: Option<u32>,
}
#[derive(Debug)]
pub struct Outputs {
    pub json: PathBuf,
    pub markdown: PathBuf,
}

pub fn run(config: &RunConfig) -> Result<Outputs> {
    if config.command.is_empty() {
        bail!("no command given to measure");
    }
    if config.runs == 0 {
        bail!("the number of measured runs must be at least 1");
    }

    let samples = measure(config.warmup, config.runs, &config.command)?;
    let summary = summarize_all(&samples);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is set before 1970")?
        .as_secs();

    let report = Report {
        command: config.command.join(" "),
        warmup: config.warmup,
        runs: config.runs,
        timestamp,
        summary,
    };

    let outputs = Outputs {
        json: config.out_dir.join("run.json"),
        markdown: config.out_dir.join("run.md"),
    };

    write_json(&report, &outputs.json)?;
    write_markdown(&report, &outputs.markdown)?;

    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    // A fresh base directory per test. The output folder is created by run,
    // so it does not exist yet when the test starts.
    fn scratch_base(name: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "perflab_run_{}_{}_{}",
            name,
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch base");
        dir
    }

    fn config(command: &[&str], warmup: u8, runs: u16, out_dir: PathBuf) -> RunConfig {
        RunConfig {
            command: command.iter().map(|s| s.to_string()).collect(),
            warmup,
            runs,
            out_dir,
        }
    }

    // ---------- guards: fail before any measurement or file output ----------

    #[test]
    fn empty_command_is_error() {
        let base = scratch_base("empty_cmd");
        let out = base.join("results");
        assert!(run(&config(&[], 0, 1, out)).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_command_error_says_no_command() {
        let base = scratch_base("empty_msg");
        let err = run(&config(&[], 0, 1, base.join("results"))).unwrap_err();
        assert!(format!("{err:#}").contains("no command given"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_command_creates_no_output_folder() {
        let base = scratch_base("empty_nofolder");
        let out = base.join("results");
        let _ = run(&config(&[], 0, 1, out.clone()));
        assert!(!out.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn zero_runs_is_error() {
        let base = scratch_base("zero_runs");
        assert!(run(&config(&["true"], 0, 0, base.join("results"))).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn zero_runs_error_says_at_least_one() {
        let base = scratch_base("zero_msg");
        let err = run(&config(&["true"], 0, 0, base.join("results"))).unwrap_err();
        assert!(format!("{err:#}").contains("at least 1"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn zero_runs_creates_no_output_folder() {
        let base = scratch_base("zero_nofolder");
        let out = base.join("results");
        let _ = run(&config(&["true"], 0, 0, out.clone()));
        assert!(!out.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_command_with_zero_runs_reports_command_first() {
        let base = scratch_base("both_bad");
        let err = run(&config(&[], 0, 0, base.join("results"))).unwrap_err();
        assert!(format!("{err:#}").contains("no command given"));
        let _ = fs::remove_dir_all(&base);
    }

    // ---------- failures during measurement: no files written ----------

    #[test]
    fn missing_program_is_error_and_writes_nothing() {
        let base = scratch_base("missing_prog");
        let out = base.join("results");
        let r = run(&config(
            &["definitely_not_a_real_command_xyz"],
            0,
            1,
            out.clone(),
        ));
        assert!(r.is_err());
        assert!(!out.join("run.json").exists());
        assert!(!out.join("run.md").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn failing_measured_command_is_error_and_writes_nothing() {
        let base = scratch_base("failing_cmd");
        let out = base.join("results");
        assert!(run(&config(&["false"], 0, 1, out.clone())).is_err());
        assert!(!out.join("run.json").exists());
        assert!(!out.join("run.md").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn failing_warmup_writes_nothing() {
        let base = scratch_base("failing_warm");
        let out = base.join("results");
        assert!(run(&config(&["false"], 2, 3, out.clone())).is_err());
        assert!(!out.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn maximum_run_count_is_accepted_by_type_and_fails_fast_on_bad_command() {
        // A failing command stops measure at its first run, so u16::MAX
        // runs is not executed in full. This checks the boundary compiles
        // and is handled without writing output.
        let base = scratch_base("max_runs");
        let out = base.join("results");
        assert!(run(&config(&["false"], 0, u16::MAX, out.clone())).is_err());
        assert!(!out.exists());
        let _ = fs::remove_dir_all(&base);
    }

    // ---------- expected behavior (needs real perf counters) ----------

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn run_creates_both_files_and_returns_their_paths() {
        let base = scratch_base("both_files");
        let out = base.join("results");
        let outputs = run(&config(&["sleep", "0.01"], 0, 2, out.clone())).unwrap();
        assert_eq!(outputs.json, out.join("run.json"));
        assert_eq!(outputs.markdown, out.join("run.md"));
        assert!(outputs.json.is_file());
        assert!(outputs.markdown.is_file());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn json_file_echoes_config_values() {
        let base = scratch_base("json_echo");
        let out = base.join("results");
        let outputs = run(&config(&["sleep", "0.01"], 1, 3, out)).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&outputs.json).unwrap()).unwrap();
        assert_eq!(v["warmup"], 1);
        assert_eq!(v["runs"], 3);
        assert_eq!(v["command"], "sleep 0.01");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn json_timestamp_is_recent() {
        let base = scratch_base("timestamp");
        let outputs = run(&config(&["sleep", "0.01"], 0, 1, base.join("r"))).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&outputs.json).unwrap()).unwrap();
        let ts = v["timestamp"].as_u64().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(ts <= now && now - ts < 3600, "timestamp {ts}, now {now}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn markdown_has_header_separator_and_six_rows() {
        let base = scratch_base("md_rows");
        let outputs = run(&config(&["sleep", "0.01"], 0, 2, base.join("r"))).unwrap();
        let text = fs::read_to_string(&outputs.markdown).unwrap();
        assert_eq!(text.lines().count(), 8);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn markdown_shows_full_validity_for_clean_run() {
        let base = scratch_base("md_valid");
        let outputs = run(&config(&["sleep", "0.01"], 0, 2, base.join("r"))).unwrap();
        let text = fs::read_to_string(&outputs.markdown).unwrap();
        assert!(text.contains("| cycles |"));
        assert!(text.contains("2/2"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn nested_output_folder_is_created() {
        let base = scratch_base("nested_out");
        let out = base.join("a").join("b").join("c");
        let outputs = run(&config(&["sleep", "0.01"], 0, 1, out)).unwrap();
        assert!(outputs.json.is_file());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn rerun_overwrites_previous_results() {
        let base = scratch_base("rerun");
        let out = base.join("r");
        let first = run(&config(&["sleep", "0.01"], 0, 1, out.clone())).unwrap();
        let second = run(&config(&["sleep", "0.01"], 0, 4, out)).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&second.json).unwrap()).unwrap();
        assert_eq!(v["runs"], 4);
        assert_eq!(first.json, second.json);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn command_is_displayed_joined_by_spaces() {
        let base = scratch_base("display");
        let outputs = run(&config(&["sh", "-c", "exit 0"], 0, 1, base.join("r"))).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&outputs.json).unwrap()).unwrap();
        assert_eq!(v["command"], "sh -c exit 0");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "needs hardware perf counters"]
    fn single_run_with_zero_warmup_works() {
        let base = scratch_base("single");
        let outputs = run(&config(&["true"], 0, 1, base.join("r"))).unwrap();
        assert!(outputs.json.is_file());
        let _ = fs::remove_dir_all(&base);
    }
}
