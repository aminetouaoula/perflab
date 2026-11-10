use crate::summarize_all::Summary;
use crate::summary::MetricSummary;
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::to_string_pretty;
use std::fs;
use std::path::Path;
#[derive(Serialize)]
pub struct Report {
    pub command: String,
    pub warmup: u8,
    pub runs: u32,
    pub timestamp: u64,
    pub summary: Summary,
}
pub fn to_json(report: &Report) -> Result<String> {
    Ok(to_string_pretty(report)?)
}
fn fmt_value(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.2}"),
        None => "-".to_string(),
    }
}

fn row(name: &str, m: &MetricSummary) -> String {
    format!(
        "| {} | {} | {} | {} | {}/{} |\n",
        name,
        fmt_value(m.median),
        fmt_value(m.min),
        fmt_value(m.max),
        m.valid,
        m.total
    )
}

pub fn to_markdown(report: &Report) -> String {
    let s = &report.summary;
    let mut out = String::new();
    out.push_str("| Metric | Median | Min | Max | Valid |\n");
    out.push_str("|---|---|---|---|---|\n");
    out.push_str(&row("cycles", &s.cycles));
    out.push_str(&row("instructions", &s.instructions));
    out.push_str(&row("cache_misses", &s.cache_misses));
    out.push_str(&row("branch_misses", &s.branch_misses));
    out.push_str(&row("wall_ms", &s.wall_ms));
    out.push_str(&row("peak_rss_kb", &s.peak_rss_kb));
    out
}
pub fn write_json(report: &Report, path: &Path) -> Result<()> {
    let json = to_json(report)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::MetricSummary;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    fn metric(median: Option<f64>, valid: usize, total: usize) -> MetricSummary {
        MetricSummary {
            median,
            min: median,
            max: median,
            valid,
            total,
        }
    }

    fn empty_metric(total: usize) -> MetricSummary {
        MetricSummary {
            median: None,
            min: None,
            max: None,
            valid: 0,
            total,
        }
    }

    fn summary() -> Summary {
        Summary {
            cycles: metric(Some(1000.0), 3, 3),
            instructions: metric(Some(2000.0), 3, 3),
            cache_misses: metric(Some(100.0), 3, 3),
            branch_misses: metric(Some(10.0), 3, 3),
            wall_ms: metric(Some(12.5), 3, 3),
            peak_rss_kb: metric(Some(3660.0), 3, 3),
        }
    }

    fn report(command: &str, warmup: u8, runs: u32, timestamp: u64, s: Summary) -> Report {
        Report {
            command: command.to_string(),
            warmup,
            runs,
            timestamp,
            summary: s,
        }
    }

    fn parse(report: &Report) -> Value {
        serde_json::from_str(&to_json(report).unwrap()).unwrap()
    }

    // A fresh directory per call, so tests never share files.
    fn scratch_dir(name: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "perflab_report_{}_{}_{}",
            name,
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    // ---------- to_json: expected behavior ----------

    #[test]
    fn top_level_has_all_five_fields() {
        let v = parse(&report("sleep 0.1", 2, 5, 1_700_000_000, summary()));
        for key in ["command", "warmup", "runs", "timestamp", "summary"] {
            assert!(v.get(key).is_some(), "missing key {key}");
        }
    }

    #[test]
    fn scalar_fields_have_expected_values() {
        let v = parse(&report("sleep 0.1", 2, 5, 1_700_000_000, summary()));
        assert_eq!(v["command"], "sleep 0.1");
        assert_eq!(v["warmup"], 2);
        assert_eq!(v["runs"], 5);
        assert_eq!(v["timestamp"], 1_700_000_000u64);
    }

    #[test]
    fn summary_contains_one_entry_per_metric() {
        let v = parse(&report("true", 0, 1, 0, summary()));
        let s = &v["summary"];
        for key in [
            "cycles",
            "instructions",
            "cache_misses",
            "branch_misses",
            "wall_ms",
            "peak_rss_kb",
        ] {
            assert!(s.get(key).is_some(), "missing metric {key}");
        }
    }

    #[test]
    fn metric_entry_has_median_min_max_valid_total() {
        let v = parse(&report("true", 0, 1, 0, summary()));
        let m = &v["summary"]["cycles"];
        assert_eq!(m["median"], 1000.0);
        assert_eq!(m["min"], 1000.0);
        assert_eq!(m["max"], 1000.0);
        assert_eq!(m["valid"], 3);
        assert_eq!(m["total"], 3);
    }

    #[test]
    fn fractional_medians_keep_their_decimals() {
        let v = parse(&report("true", 0, 1, 0, summary()));
        assert_eq!(v["summary"]["wall_ms"]["median"], 12.5);
    }

    #[test]
    fn output_is_pretty_printed_with_newlines() {
        let json = to_json(&report("true", 0, 1, 0, summary())).unwrap();
        assert!(json.contains('\n'));
    }

    #[test]
    fn output_round_trips_to_same_values() {
        let original = report("sleep 0.1", 2, 5, 42, summary());
        let v = parse(&original);
        assert_eq!(v["timestamp"], 42);
        assert_eq!(v["summary"]["cycles"]["median"], 1000.0);
    }

    // ---------- to_json: missing values ----------

    #[test]
    fn missing_median_is_null_not_zero() {
        let mut s = summary();
        s.cycles = empty_metric(3);
        let v = parse(&report("true", 0, 3, 0, s));
        let m = &v["summary"]["cycles"];
        assert!(m["median"].is_null());
        assert!(m["min"].is_null());
        assert!(m["max"].is_null());
    }

    #[test]
    fn missing_metric_still_reports_valid_and_total() {
        let mut s = summary();
        s.cycles = empty_metric(3);
        let v = parse(&report("true", 0, 3, 0, s));
        assert_eq!(v["summary"]["cycles"]["valid"], 0);
        assert_eq!(v["summary"]["cycles"]["total"], 3);
    }

    #[test]
    fn missing_metric_does_not_affect_other_metrics() {
        let mut s = summary();
        s.cycles = empty_metric(3);
        let v = parse(&report("true", 0, 3, 0, s));
        assert_eq!(v["summary"]["instructions"]["median"], 2000.0);
    }

    // ---------- to_json: edge cases ----------

    #[test]
    fn zero_warmup_and_zero_runs_are_written() {
        let v = parse(&report("true", 0, 0, 0, summary()));
        assert_eq!(v["warmup"], 0);
        assert_eq!(v["runs"], 0);
    }

    #[test]
    fn max_warmup_value_is_written() {
        let v = parse(&report("true", u8::MAX, 1, 0, summary()));
        assert_eq!(v["warmup"], 255);
    }

    #[test]
    fn large_run_count_is_written() {
        let v = parse(&report("true", 0, u32::MAX, 0, summary()));
        assert_eq!(v["runs"], u32::MAX);
    }

    #[test]
    fn large_timestamp_is_written_exactly() {
        let v = parse(&report("true", 0, 1, u64::MAX, summary()));
        assert_eq!(v["timestamp"], u64::MAX);
    }

    #[test]
    fn empty_command_string_is_written_as_empty() {
        let v = parse(&report("", 0, 1, 0, summary()));
        assert_eq!(v["command"], "");
    }

    #[test]
    fn command_with_quotes_is_escaped_and_round_trips() {
        let cmd = r#"sh -c "echo hi""#;
        let v = parse(&report(cmd, 0, 1, 0, summary()));
        assert_eq!(v["command"], cmd);
    }

    #[test]
    fn command_with_unicode_round_trips() {
        let cmd = "echo héllo ✓";
        let v = parse(&report(cmd, 0, 1, 0, summary()));
        assert_eq!(v["command"], cmd);
    }

    #[test]
    fn command_with_newline_is_escaped() {
        let cmd = "echo a\necho b";
        let v = parse(&report(cmd, 0, 1, 0, summary()));
        assert_eq!(v["command"], cmd);
    }

    #[test]
    fn all_metrics_missing_still_produces_valid_json() {
        let s = Summary {
            cycles: empty_metric(2),
            instructions: empty_metric(2),
            cache_misses: empty_metric(2),
            branch_misses: empty_metric(2),
            wall_ms: empty_metric(2),
            peak_rss_kb: empty_metric(2),
        };
        let v = parse(&report("true", 0, 2, 0, s));
        assert!(v["summary"]["wall_ms"]["median"].is_null());
    }

    #[test]
    fn zero_median_is_written_as_zero_not_null() {
        let mut s = summary();
        s.cycles = metric(Some(0.0), 1, 1);
        let v = parse(&report("true", 0, 1, 0, s));
        assert_eq!(v["summary"]["cycles"]["median"], 0.0);
        assert!(!v["summary"]["cycles"]["median"].is_null());
    }

    #[test]
    fn negative_wall_time_is_written() {
        let mut s = summary();
        s.wall_ms = metric(Some(-1.5), 1, 1);
        let v = parse(&report("true", 0, 1, 0, s));
        assert_eq!(v["summary"]["wall_ms"]["median"], -1.5);
    }

    // ---------- write_json: expected behavior ----------

    #[test]
    fn write_json_creates_file_and_returns_ok() {
        let dir = scratch_dir("create");
        let path = dir.join("run.json");
        assert!(write_json(&report("true", 0, 1, 0, summary()), &path).is_ok());
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn written_file_matches_to_json_output_exactly() {
        let dir = scratch_dir("exact");
        let path = dir.join("run.json");
        let r = report("sleep 0.1", 2, 5, 1_700_000_000, summary());
        write_json(&r, &path).unwrap();
        let on_disk = fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, to_json(&r).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn written_file_parses_back_as_json() {
        let dir = scratch_dir("parse");
        let path = dir.join("run.json");
        write_json(&report("sleep 0.1", 2, 5, 42, summary()), &path).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["timestamp"], 42);
        assert_eq!(v["summary"]["cycles"]["median"], 1000.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_single_missing_parent_directory() {
        let dir = scratch_dir("one_parent");
        let path = dir.join("results").join("run.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(dir.join("results").is_dir());
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_deeply_nested_missing_parents() {
        let dir = scratch_dir("nested");
        let path = dir.join("a").join("b").join("c").join("d").join("run.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_parent_directory_is_reused() {
        let dir = scratch_dir("reuse");
        fs::create_dir_all(dir.join("results")).unwrap();
        let path = dir.join("results").join("run.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_sibling_files_are_left_untouched() {
        let dir = scratch_dir("siblings");
        let other = dir.join("other.txt");
        fs::write(&other, "keep me").unwrap();
        write_json(&report("true", 0, 1, 0, summary()), &dir.join("run.json")).unwrap();
        assert_eq!(fs::read_to_string(&other).unwrap(), "keep me");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn overwrites_existing_file_completely() {
        let dir = scratch_dir("overwrite");
        let path = dir.join("run.json");
        fs::write(
            &path,
            "old content that is much longer than the new report would be",
        )
        .unwrap();
        let r = report("true", 0, 1, 0, summary());
        write_json(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), to_json(&r).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shorter_report_leaves_no_trailing_bytes_from_longer_one() {
        let dir = scratch_dir("truncate");
        let path = dir.join("run.json");
        write_json(&report(&"x".repeat(5000), 0, 1, 0, summary()), &path).unwrap();
        let small = report("t", 0, 1, 0, summary());
        write_json(&small, &path).unwrap();
        let on_disk = fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, to_json(&small).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn garbage_file_is_replaced_by_valid_json() {
        let dir = scratch_dir("garbage");
        let path = dir.join("run.json");
        fs::write(&path, "not json at all {{{").unwrap();
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["command"], "true");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_same_report_twice_gives_identical_file() {
        let dir = scratch_dir("twice");
        let path = dir.join("run.json");
        let r = report("true", 1, 2, 3, summary());
        write_json(&r, &path).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        write_json(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_with_spaces_works() {
        let dir = scratch_dir("spaces");
        let path = dir.join("my results").join("final run.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_with_unicode_works() {
        let dir = scratch_dir("unicode");
        let path = dir.join("résultats_✓").join("run.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filename_with_several_dots_works() {
        let dir = scratch_dir("dots");
        let path = dir.join("run.v1.final.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filename_without_extension_works() {
        let dir = scratch_dir("noext");
        let path = dir.join("run");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parent_components_inside_path_resolve() {
        let dir = scratch_dir("dotdot");
        let path = dir.join("x").join("..").join("run.json");
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        assert!(dir.join("run.json").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_report_is_written_completely() {
        let dir = scratch_dir("large");
        let path = dir.join("run.json");
        let cmd = "a".repeat(1_000_000);
        let r = report(&cmd, 0, 1, 0, summary());
        write_json(&r, &path).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["command"].as_str().unwrap().len(), 1_000_000);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_writes_to_distinct_files_all_succeed() {
        let dir = scratch_dir("concurrent");
        thread::scope(|scope| {
            for i in 0..8u32 {
                let path = dir.join(format!("run_{i}.json"));
                scope.spawn(move || {
                    let r = report("true", 0, i, i as u64, summary());
                    write_json(&r, &path).unwrap();
                    let v: Value =
                        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
                    assert_eq!(v["runs"], i);
                });
            }
        });
        let _ = fs::remove_dir_all(&dir);
    }

    // ---------- write_json: error behavior ----------

    #[test]
    fn path_that_is_an_existing_directory_is_error() {
        let dir = scratch_dir("is_dir");
        let target = dir.join("already_a_dir");
        fs::create_dir_all(&target).unwrap();
        assert!(write_json(&report("true", 0, 1, 0, summary()), &target).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parent_that_is_a_regular_file_is_error() {
        let dir = scratch_dir("file_parent");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "I am a file").unwrap();
        let path = blocker.join("run.json");
        assert!(write_json(&report("true", 0, 1, 0, summary()), &path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parent_file_blocker_is_not_modified_on_error() {
        let dir = scratch_dir("blocker_intact");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "I am a file").unwrap();
        let _ = write_json(
            &report("true", 0, 1, 0, summary()),
            &blocker.join("run.json"),
        );
        assert_eq!(fs::read_to_string(&blocker).unwrap(), "I am a file");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_path_is_error() {
        assert!(write_json(&report("true", 0, 1, 0, summary()), Path::new("")).is_err());
    }

    #[test]
    fn path_ending_in_separator_is_error() {
        let dir = scratch_dir("trailing_sep");
        let path = dir.join("missing_dir").join("");
        assert!(write_json(&report("true", 0, 1, 0, summary()), &path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_containing_nul_byte_is_error() {
        let dir = scratch_dir("nul");
        let path = dir.join("bad\0name.json");
        assert!(write_json(&report("true", 0, 1, 0, summary()), &path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filename_longer_than_filesystem_limit_is_error() {
        let dir = scratch_dir("too_long");
        let name = format!("{}.json", "a".repeat(300));
        let path = dir.join(name);
        assert!(write_json(&report("true", 0, 1, 0, summary()), &path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn error_message_names_the_failing_path() {
        let dir = scratch_dir("message");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("run.json");
        let err = write_json(&report("true", 0, 1, 0, summary()), &path).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("blocker"), "message was: {msg}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_write_leaves_no_partial_file_at_target() {
        let dir = scratch_dir("no_partial");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("run.json");
        let _ = write_json(&report("true", 0, 1, 0, summary()), &path);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
#[cfg(test)]
mod markdown_tests {
    use super::*;
    use crate::summary::MetricSummary;

    fn m(
        median: Option<f64>,
        min: Option<f64>,
        max: Option<f64>,
        valid: usize,
        total: usize,
    ) -> MetricSummary {
        MetricSummary {
            median,
            min,
            max,
            valid,
            total,
        }
    }

    fn empty(total: usize) -> MetricSummary {
        m(None, None, None, 0, total)
    }

    fn full_summary() -> Summary {
        Summary {
            cycles: m(Some(1000.0), Some(900.0), Some(1100.0), 3, 3),
            instructions: m(Some(2000.0), Some(1800.0), Some(2200.0), 3, 3),
            cache_misses: m(Some(100.0), Some(90.0), Some(110.0), 3, 3),
            branch_misses: m(Some(10.0), Some(9.0), Some(11.0), 3, 3),
            wall_ms: m(Some(12.5), Some(12.0), Some(13.25), 3, 3),
            peak_rss_kb: m(Some(3660.0), Some(3600.0), Some(3700.0), 3, 3),
        }
    }

    fn empty_summary(total: usize) -> Summary {
        Summary {
            cycles: empty(total),
            instructions: empty(total),
            cache_misses: empty(total),
            branch_misses: empty(total),
            wall_ms: empty(total),
            peak_rss_kb: empty(total),
        }
    }

    fn report_with(summary: Summary) -> Report {
        Report {
            command: "true".to_string(),
            warmup: 0,
            runs: 3,
            timestamp: 0,
            summary,
        }
    }

    // ---------- fmt_value ----------

    #[test]
    fn fmt_value_none_is_dash() {
        assert_eq!(fmt_value(None), "-");
    }

    #[test]
    fn fmt_value_some_zero_is_zero_not_dash() {
        assert_eq!(fmt_value(Some(0.0)), "0.00");
    }

    #[test]
    fn fmt_value_pads_to_two_decimals() {
        assert_eq!(fmt_value(Some(2.5)), "2.50");
        assert_eq!(fmt_value(Some(7.0)), "7.00");
    }

    #[test]
    fn fmt_value_keeps_sign_for_negative() {
        assert_eq!(fmt_value(Some(-2.5)), "-2.50");
    }

    #[test]
    fn fmt_value_rounds_down_below_half() {
        assert_eq!(fmt_value(Some(1.001)), "1.00");
    }

    #[test]
    fn fmt_value_rounds_up_above_half() {
        assert_eq!(fmt_value(Some(1.006)), "1.01");
    }

    #[test]
    fn fmt_value_binary_tie_case_is_stored_below_half() {
        // 1.005 is stored in binary as about 1.00499999..., so it rounds down.
        assert_eq!(fmt_value(Some(1.005)), "1.00");
    }

    #[test]
    fn fmt_value_large_value_has_no_scientific_notation() {
        assert_eq!(fmt_value(Some(1e15)), "1000000000000000.00");
    }

    #[test]
    fn fmt_value_tiny_value_rounds_to_zero() {
        assert_eq!(fmt_value(Some(1e-9)), "0.00");
    }

    #[test]
    fn fmt_value_negative_zero_keeps_sign() {
        // Documents the current behavior: -0.0 prints with its sign.
        assert_eq!(fmt_value(Some(-0.0)), "-0.00");
    }

    #[test]
    fn fmt_value_nan_prints_as_nan() {
        assert_eq!(fmt_value(Some(f64::NAN)), "NaN");
    }

    #[test]
    fn fmt_value_infinity_prints_as_inf() {
        assert_eq!(fmt_value(Some(f64::INFINITY)), "inf");
        assert_eq!(fmt_value(Some(f64::NEG_INFINITY)), "-inf");
    }

    // ---------- row ----------

    #[test]
    fn row_full_metric_is_exact() {
        let r = row("cycles", &m(Some(1.0), Some(0.5), Some(2.0), 3, 3));
        assert_eq!(r, "| cycles | 1.00 | 0.50 | 2.00 | 3/3 |\n");
    }

    #[test]
    fn row_empty_metric_shows_dashes_and_zero_valid() {
        let r = row("cycles", &empty(3));
        assert_eq!(r, "| cycles | - | - | - | 0/3 |\n");
    }

    #[test]
    fn row_partial_metric_shows_valid_over_total() {
        let r = row("cycles", &m(Some(1.0), Some(1.0), Some(1.0), 2, 5));
        assert!(r.ends_with("| 2/5 |\n"), "row was: {r}");
    }

    #[test]
    fn row_zero_total_shows_zero_over_zero() {
        let r = row("cycles", &empty(0));
        assert_eq!(r, "| cycles | - | - | - | 0/0 |\n");
    }

    #[test]
    fn row_name_is_written_verbatim_with_underscore() {
        let r = row("cache_misses", &empty(1));
        assert!(r.starts_with("| cache_misses |"), "row was: {r}");
    }

    #[test]
    fn row_has_exactly_five_cells() {
        let r = row("cycles", &m(Some(1.0), Some(1.0), Some(1.0), 1, 1));
        // Splitting "| a | b |" on '|' gives 4 pieces, so five cells gives 7.
        assert_eq!(r.trim_end().split('|').count(), 7);
    }

    #[test]
    fn row_ends_with_newline() {
        let r = row("cycles", &empty(1));
        assert!(r.ends_with('\n'));
    }

    #[test]
    fn row_empty_metric_never_shows_fake_zero() {
        let r = row("cycles", &empty(4));
        assert!(!r.contains("0.00"), "row was: {r}");
    }

    #[test]
    fn row_handles_maximum_counts() {
        let r = row(
            "cycles",
            &m(Some(1.0), Some(1.0), Some(1.0), usize::MAX, usize::MAX),
        );
        let expected = format!("{}/{}", usize::MAX, usize::MAX);
        assert!(r.contains(&expected), "row was: {r}");
    }

    // ---------- to_markdown: expected output ----------

    #[test]
    fn to_markdown_full_output_matches_expected_text() {
        let out = to_markdown(&report_with(full_summary()));
        let expected = "\
| Metric | Median | Min | Max | Valid |
|---|---|---|---|---|
| cycles | 1000.00 | 900.00 | 1100.00 | 3/3 |
| instructions | 2000.00 | 1800.00 | 2200.00 | 3/3 |
| cache_misses | 100.00 | 90.00 | 110.00 | 3/3 |
| branch_misses | 10.00 | 9.00 | 11.00 | 3/3 |
| wall_ms | 12.50 | 12.00 | 13.25 | 3/3 |
| peak_rss_kb | 3660.00 | 3600.00 | 3700.00 | 3/3 |
";
        assert_eq!(out, expected);
    }

    #[test]
    fn to_markdown_header_is_first_line() {
        let out = to_markdown(&report_with(full_summary()));
        assert_eq!(
            out.lines().next().unwrap(),
            "| Metric | Median | Min | Max | Valid |"
        );
    }

    #[test]
    fn to_markdown_separator_is_second_line() {
        let out = to_markdown(&report_with(full_summary()));
        assert_eq!(out.lines().nth(1).unwrap(), "|---|---|---|---|---|");
    }

    #[test]
    fn to_markdown_has_header_separator_and_six_rows() {
        let out = to_markdown(&report_with(full_summary()));
        assert_eq!(out.lines().count(), 8);
    }

    #[test]
    fn to_markdown_rows_follow_fixed_metric_order() {
        let out = to_markdown(&report_with(full_summary()));
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[2].starts_with("| cycles |"));
        assert!(lines[3].starts_with("| instructions |"));
        assert!(lines[4].starts_with("| cache_misses |"));
        assert!(lines[5].starts_with("| branch_misses |"));
        assert!(lines[6].starts_with("| wall_ms |"));
        assert!(lines[7].starts_with("| peak_rss_kb |"));
    }

    #[test]
    fn to_markdown_every_line_has_same_cell_count() {
        let out = to_markdown(&report_with(full_summary()));
        for line in out.lines() {
            assert_eq!(line.trim_end().split('|').count(), 7, "line was: {line}");
        }
    }

    #[test]
    fn to_markdown_ends_with_newline() {
        let out = to_markdown(&report_with(full_summary()));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn to_markdown_is_deterministic() {
        let r = report_with(full_summary());
        assert_eq!(to_markdown(&r), to_markdown(&r));
    }

    // ---------- to_markdown: missing values ----------

    #[test]
    fn to_markdown_all_empty_gives_six_dash_rows() {
        let out = to_markdown(&report_with(empty_summary(4)));
        assert_eq!(out.matches("| - | - | - | 0/4 |").count(), 6);
    }

    #[test]
    fn to_markdown_all_empty_never_shows_fake_zero() {
        let out = to_markdown(&report_with(empty_summary(4)));
        assert!(!out.contains("0.00"), "output was:\n{out}");
    }

    #[test]
    fn to_markdown_one_empty_metric_does_not_affect_others() {
        let mut s = full_summary();
        s.cycles = empty(3);
        let out = to_markdown(&report_with(s));
        assert!(out.contains("| cycles | - | - | - | 0/3 |"));
        assert!(out.contains("| instructions | 2000.00 | 1800.00 | 2200.00 | 3/3 |"));
    }

    #[test]
    fn to_markdown_partial_metric_shows_valid_over_total() {
        let mut s = full_summary();
        s.wall_ms = m(Some(12.5), Some(12.0), Some(13.0), 2, 5);
        let out = to_markdown(&report_with(s));
        assert!(out.contains("| wall_ms | 12.50 | 12.00 | 13.00 | 2/5 |"));
    }

    #[test]
    fn to_markdown_zero_runs_shows_zero_over_zero() {
        let out = to_markdown(&report_with(empty_summary(0)));
        assert_eq!(out.matches("0/0").count(), 6);
    }

    // ---------- to_markdown: value edge cases ----------

    #[test]
    fn to_markdown_huge_value_is_written_in_full() {
        let mut s = full_summary();
        s.cycles = m(Some(1e300), Some(1e300), Some(1e300), 1, 1);
        let out = to_markdown(&report_with(s));
        let cycles_line = out.lines().find(|l| l.starts_with("| cycles |")).unwrap();
        assert!(
            cycles_line.len() > 300,
            "line was {} chars",
            cycles_line.len()
        );
    }

    #[test]
    fn to_markdown_negative_wall_time_is_shown_with_sign() {
        let mut s = full_summary();
        s.wall_ms = m(Some(-1.5), Some(-2.0), Some(0.5), 3, 3);
        let out = to_markdown(&report_with(s));
        assert!(out.contains("| wall_ms | -1.50 | -2.00 | 0.50 | 3/3 |"));
    }
}
