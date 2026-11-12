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
    pub runs: u16,
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
    out.push_str(&format!("Command: `{}`\n\n", report.command));
    out.push_str(&format!("Warm-up runs: {}\n", report.warmup));
    out.push_str(&format!("Measured runs: {}\n", report.runs));
    out.push_str(&format!(
        "Timestamp: {} (Unix seconds)\n\n",
        report.timestamp
    ));
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
    write_file(&to_json(report)?, path)
}

pub fn write_markdown(report: &Report, path: &Path) -> Result<()> {
    write_file(&to_markdown(report), path)
}
fn write_file(content: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::MetricSummary;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    // ---------- helpers ----------

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

    // Median, min and max all equal, for JSON tests.
    fn metric(median: Option<f64>, valid: usize, total: usize) -> MetricSummary {
        m(median, median, median, valid, total)
    }

    fn empty(total: usize) -> MetricSummary {
        m(None, None, None, 0, total)
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

    fn report(command: &str, warmup: u8, runs: u16, timestamp: u64, s: Summary) -> Report {
        Report {
            command: command.to_string(),
            warmup,
            runs,
            timestamp,
            summary: s,
        }
    }

    fn report_with(s: Summary) -> Report {
        report("true", 0, 3, 0, s)
    }

    fn parse(r: &Report) -> Value {
        serde_json::from_str(&to_json(r).unwrap()).unwrap()
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
    fn json_top_level_has_all_five_fields() {
        let v = parse(&report("sleep 0.1", 2, 5, 1_700_000_000, summary()));
        for key in ["command", "warmup", "runs", "timestamp", "summary"] {
            assert!(v.get(key).is_some(), "missing key {key}");
        }
    }

    #[test]
    fn json_scalar_fields_have_expected_values() {
        let v = parse(&report("sleep 0.1", 2, 5, 1_700_000_000, summary()));
        assert_eq!(v["command"], "sleep 0.1");
        assert_eq!(v["warmup"], 2);
        assert_eq!(v["runs"], 5);
        assert_eq!(v["timestamp"], 1_700_000_000u64);
    }

    #[test]
    fn json_summary_has_one_entry_per_metric() {
        let v = parse(&report_with(summary()));
        for key in [
            "cycles",
            "instructions",
            "cache_misses",
            "branch_misses",
            "wall_ms",
            "peak_rss_kb",
        ] {
            assert!(v["summary"].get(key).is_some(), "missing metric {key}");
        }
    }

    #[test]
    fn json_metric_has_median_min_max_valid_total() {
        let v = parse(&report_with(summary()));
        let c = &v["summary"]["cycles"];
        assert_eq!(c["median"], 1000.0);
        assert_eq!(c["min"], 1000.0);
        assert_eq!(c["max"], 1000.0);
        assert_eq!(c["valid"], 3);
        assert_eq!(c["total"], 3);
    }

    #[test]
    fn json_fractional_medians_keep_decimals() {
        let v = parse(&report_with(summary()));
        assert_eq!(v["summary"]["wall_ms"]["median"], 12.5);
    }

    #[test]
    fn json_is_pretty_printed() {
        let json = to_json(&report_with(summary())).unwrap();
        assert!(json.contains('\n'));
    }

    #[test]
    fn json_round_trips_values() {
        let v = parse(&report("sleep 0.1", 2, 5, 42, summary()));
        assert_eq!(v["timestamp"], 42);
        assert_eq!(v["summary"]["cycles"]["median"], 1000.0);
    }

    // ---------- to_json: missing values ----------

    #[test]
    fn json_missing_median_is_null_not_zero() {
        let mut s = summary();
        s.cycles = empty(3);
        let v = parse(&report("true", 0, 3, 0, s));
        let c = &v["summary"]["cycles"];
        assert!(c["median"].is_null());
        assert!(c["min"].is_null());
        assert!(c["max"].is_null());
    }

    #[test]
    fn json_missing_metric_reports_valid_and_total() {
        let mut s = summary();
        s.cycles = empty(3);
        let v = parse(&report("true", 0, 3, 0, s));
        assert_eq!(v["summary"]["cycles"]["valid"], 0);
        assert_eq!(v["summary"]["cycles"]["total"], 3);
    }

    #[test]
    fn json_missing_metric_does_not_affect_others() {
        let mut s = summary();
        s.cycles = empty(3);
        let v = parse(&report("true", 0, 3, 0, s));
        assert_eq!(v["summary"]["instructions"]["median"], 2000.0);
    }

    #[test]
    fn json_all_metrics_missing_is_still_valid_json() {
        let v = parse(&report("true", 0, 2, 0, empty_summary(2)));
        assert!(v["summary"]["wall_ms"]["median"].is_null());
    }

    // ---------- to_json: edge cases ----------

    #[test]
    fn json_zero_warmup_and_runs_are_written() {
        let v = parse(&report("true", 0, 0, 0, summary()));
        assert_eq!(v["warmup"], 0);
        assert_eq!(v["runs"], 0);
    }

    #[test]
    fn json_max_warmup_is_written() {
        let v = parse(&report("true", u8::MAX, 1, 0, summary()));
        assert_eq!(v["warmup"], 255);
    }

    #[test]
    fn json_max_runs_is_written() {
        let v = parse(&report("true", 0, u16::MAX, 0, summary()));
        assert_eq!(v["runs"], u16::MAX);
    }

    #[test]
    fn json_max_timestamp_is_written_exactly() {
        let v = parse(&report("true", 0, 1, u64::MAX, summary()));
        assert_eq!(v["timestamp"], u64::MAX);
    }

    #[test]
    fn json_empty_command_is_written_as_empty_string() {
        let v = parse(&report("", 0, 1, 0, summary()));
        assert_eq!(v["command"], "");
    }

    #[test]
    fn json_quotes_are_escaped_and_round_trip() {
        let cmd = r#"sh -c "echo hi""#;
        let v = parse(&report(cmd, 0, 1, 0, summary()));
        assert_eq!(v["command"], cmd);
    }

    #[test]
    fn json_unicode_round_trips() {
        let cmd = "echo héllo ✓";
        let v = parse(&report(cmd, 0, 1, 0, summary()));
        assert_eq!(v["command"], cmd);
    }

    #[test]
    fn json_newline_is_escaped_and_round_trips() {
        let cmd = "echo a\necho b";
        let v = parse(&report(cmd, 0, 1, 0, summary()));
        assert_eq!(v["command"], cmd);
    }

    #[test]
    fn json_zero_median_is_zero_not_null() {
        let mut s = summary();
        s.cycles = metric(Some(0.0), 1, 1);
        let v = parse(&report("true", 0, 1, 0, s));
        assert_eq!(v["summary"]["cycles"]["median"], 0.0);
        assert!(!v["summary"]["cycles"]["median"].is_null());
    }

    #[test]
    fn json_negative_value_is_written() {
        let mut s = summary();
        s.wall_ms = metric(Some(-1.5), 1, 1);
        let v = parse(&report("true", 0, 1, 0, s));
        assert_eq!(v["summary"]["wall_ms"]["median"], -1.5);
    }

    // ---------- fmt_value ----------

    #[test]
    fn fmt_value_none_is_dash() {
        assert_eq!(fmt_value(None), "-");
    }

    #[test]
    fn fmt_value_zero_is_zero_not_dash() {
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
    fn fmt_value_1_005_rounds_down_because_binary_is_below_half() {
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
        assert_eq!(row("cycles", &empty(3)), "| cycles | - | - | - | 0/3 |\n");
    }

    #[test]
    fn row_partial_metric_shows_valid_over_total() {
        let r = row("cycles", &m(Some(1.0), Some(1.0), Some(1.0), 2, 5));
        assert!(r.ends_with("| 2/5 |\n"), "row was: {r}");
    }

    #[test]
    fn row_zero_total_shows_zero_over_zero() {
        assert_eq!(row("cycles", &empty(0)), "| cycles | - | - | - | 0/0 |\n");
    }

    #[test]
    fn row_name_is_written_verbatim() {
        let r = row("cache_misses", &empty(1));
        assert!(r.starts_with("| cache_misses |"), "row was: {r}");
    }

    #[test]
    fn row_has_exactly_five_cells() {
        let r = row("cycles", &m(Some(1.0), Some(1.0), Some(1.0), 1, 1));
        // "| a | b |" splits into 4 pieces, so five cells give 7.
        assert_eq!(r.trim_end().split('|').count(), 7);
    }

    #[test]
    fn row_ends_with_newline() {
        assert!(row("cycles", &empty(1)).ends_with('\n'));
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
        assert!(
            r.contains(&format!("{}/{}", usize::MAX, usize::MAX)),
            "row was: {r}"
        );
    }

    // ---------- to_markdown: expected output ----------

    #[test]
    fn markdown_full_output_matches_expected_text() {
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
    fn markdown_first_line_is_header() {
        let out = to_markdown(&report_with(full_summary()));
        assert_eq!(
            out.lines().next().unwrap(),
            "| Metric | Median | Min | Max | Valid |"
        );
    }

    #[test]
    fn markdown_second_line_is_separator() {
        let out = to_markdown(&report_with(full_summary()));
        assert_eq!(out.lines().nth(1).unwrap(), "|---|---|---|---|---|");
    }

    #[test]
    fn markdown_has_header_separator_and_six_rows() {
        let out = to_markdown(&report_with(full_summary()));
        assert_eq!(out.lines().count(), 8);
    }

    #[test]
    fn markdown_rows_follow_fixed_order() {
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
    fn markdown_every_line_has_same_cell_count() {
        let out = to_markdown(&report_with(full_summary()));
        for line in out.lines() {
            assert_eq!(line.trim_end().split('|').count(), 7, "line was: {line}");
        }
    }

    #[test]
    fn markdown_ends_with_newline() {
        assert!(to_markdown(&report_with(full_summary())).ends_with('\n'));
    }

    #[test]
    fn markdown_is_deterministic() {
        let r = report_with(full_summary());
        assert_eq!(to_markdown(&r), to_markdown(&r));
    }

    // ---------- to_markdown: missing values ----------

    #[test]
    fn markdown_all_empty_gives_six_dash_rows() {
        let out = to_markdown(&report_with(empty_summary(4)));
        assert_eq!(out.matches("| - | - | - | 0/4 |").count(), 6);
    }

    #[test]
    fn markdown_all_empty_never_shows_fake_zero() {
        let out = to_markdown(&report_with(empty_summary(4)));
        assert!(!out.contains("0.00"), "output was:\n{out}");
    }

    #[test]
    fn markdown_one_empty_metric_does_not_affect_others() {
        let mut s = full_summary();
        s.cycles = empty(3);
        let out = to_markdown(&report_with(s));
        assert!(out.contains("| cycles | - | - | - | 0/3 |"));
        assert!(out.contains("| instructions | 2000.00 | 1800.00 | 2200.00 | 3/3 |"));
    }

    #[test]
    fn markdown_partial_metric_shows_valid_over_total() {
        let mut s = full_summary();
        s.wall_ms = m(Some(12.5), Some(12.0), Some(13.0), 2, 5);
        let out = to_markdown(&report_with(s));
        assert!(out.contains("| wall_ms | 12.50 | 12.00 | 13.00 | 2/5 |"));
    }

    #[test]
    fn markdown_zero_runs_shows_zero_over_zero() {
        let out = to_markdown(&report_with(empty_summary(0)));
        assert_eq!(out.matches("0/0").count(), 6);
    }

    // ---------- to_markdown: value edge cases ----------

    #[test]
    fn markdown_huge_value_is_written_in_full() {
        let mut s = full_summary();
        s.cycles = m(Some(1e300), Some(1e300), Some(1e300), 1, 1);
        let out = to_markdown(&report_with(s));
        let line = out.lines().find(|l| l.starts_with("| cycles |")).unwrap();
        assert!(line.len() > 300, "line was {} chars", line.len());
    }

    #[test]
    fn markdown_negative_wall_time_keeps_sign() {
        let mut s = full_summary();
        s.wall_ms = m(Some(-1.5), Some(-2.0), Some(0.5), 3, 3);
        let out = to_markdown(&report_with(s));
        assert!(out.contains("| wall_ms | -1.50 | -2.00 | 0.50 | 3/3 |"));
    }

    // ---------- write_file: expected behavior ----------

    #[test]
    fn write_file_creates_file_with_exact_content() {
        let dir = scratch_dir("wf_exact");
        let path = dir.join("out.txt");
        write_file("hello\nworld\n", &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\nworld\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_creates_nested_parents() {
        let dir = scratch_dir("wf_nested");
        let path = dir.join("a").join("b").join("c.txt");
        write_file("x", &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_replaces_longer_existing_content() {
        let dir = scratch_dir("wf_truncate");
        let path = dir.join("out.txt");
        fs::write(&path, "a very long previous content that must disappear").unwrap();
        write_file("new", &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_empty_content_creates_empty_file() {
        let dir = scratch_dir("wf_empty");
        let path = dir.join("out.txt");
        write_file("", &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_leaves_sibling_files_alone() {
        let dir = scratch_dir("wf_siblings");
        fs::write(dir.join("keep.txt"), "keep").unwrap();
        write_file("x", &dir.join("new.txt")).unwrap();
        assert_eq!(fs::read_to_string(dir.join("keep.txt")).unwrap(), "keep");
        let _ = fs::remove_dir_all(&dir);
    }

    // ---------- write_file: error behavior ----------

    #[test]
    fn write_file_directory_as_target_is_error() {
        let dir = scratch_dir("wf_is_dir");
        let target = dir.join("already_dir");
        fs::create_dir_all(&target).unwrap();
        assert!(write_file("x", &target).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_regular_file_as_parent_is_error() {
        let dir = scratch_dir("wf_file_parent");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        assert!(write_file("x", &blocker.join("out.txt")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_failure_leaves_blocker_unmodified() {
        let dir = scratch_dir("wf_blocker_intact");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let _ = write_file("x", &blocker.join("out.txt"));
        assert_eq!(fs::read_to_string(&blocker).unwrap(), "file");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_empty_path_is_error() {
        assert!(write_file("x", Path::new("")).is_err());
    }

    #[test]
    fn write_file_nul_byte_in_path_is_error() {
        let dir = scratch_dir("wf_nul");
        assert!(write_file("x", &dir.join("bad\0name.txt")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_overlong_name_is_error() {
        let dir = scratch_dir("wf_long");
        let name = "a".repeat(300);
        assert!(write_file("x", &dir.join(name)).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_file_error_names_the_path() {
        let dir = scratch_dir("wf_message");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let err = write_file("x", &blocker.join("out.txt")).unwrap_err();
        assert!(format!("{err:#}").contains("blocker"));
        let _ = fs::remove_dir_all(&dir);
    }

    // ---------- write_json ----------

    #[test]
    fn write_json_creates_file_and_returns_ok() {
        let dir = scratch_dir("wj_create");
        let path = dir.join("run.json");
        assert!(write_json(&report_with(summary()), &path).is_ok());
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_file_matches_to_json_exactly() {
        let dir = scratch_dir("wj_exact");
        let path = dir.join("run.json");
        let r = report("sleep 0.1", 2, 5, 1_700_000_000, summary());
        write_json(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), to_json(&r).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_file_parses_back() {
        let dir = scratch_dir("wj_parse");
        let path = dir.join("run.json");
        write_json(&report("sleep 0.1", 2, 5, 42, summary()), &path).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["timestamp"], 42);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_creates_missing_parent() {
        let dir = scratch_dir("wj_parent");
        let path = dir.join("results").join("run.json");
        write_json(&report_with(summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_overwrites_existing_file_completely() {
        let dir = scratch_dir("wj_overwrite");
        let path = dir.join("run.json");
        fs::write(&path, "old content that is much longer than the new report").unwrap();
        let r = report_with(summary());
        write_json(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), to_json(&r).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_replaces_garbage_with_valid_json() {
        let dir = scratch_dir("wj_garbage");
        let path = dir.join("run.json");
        fs::write(&path, "not json {{{").unwrap();
        write_json(&report("true", 0, 1, 0, summary()), &path).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["command"], "true");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_is_idempotent() {
        let dir = scratch_dir("wj_twice");
        let path = dir.join("run.json");
        let r = report("true", 1, 2, 3, summary());
        write_json(&r, &path).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        write_json(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_does_not_write_markdown() {
        let dir = scratch_dir("wj_not_md");
        let path = dir.join("run.json");
        write_json(&report_with(summary()), &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("| Metric |"));
        assert!(text.trim_start().starts_with('{'));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_path_with_spaces_and_unicode_works() {
        let dir = scratch_dir("wj_names");
        let path = dir.join("my résultats").join("final run.json");
        write_json(&report_with(summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_large_command_is_written_completely() {
        let dir = scratch_dir("wj_large");
        let path = dir.join("run.json");
        let cmd = "a".repeat(1_000_000);
        write_json(&report(&cmd, 0, 1, 0, summary()), &path).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["command"].as_str().unwrap().len(), 1_000_000);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_directory_target_is_error() {
        let dir = scratch_dir("wj_is_dir");
        let target = dir.join("already_dir");
        fs::create_dir_all(&target).unwrap();
        assert!(write_json(&report_with(summary()), &target).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_empty_path_is_error() {
        assert!(write_json(&report_with(summary()), Path::new("")).is_err());
    }

    #[test]
    fn write_json_trailing_separator_is_error() {
        let dir = scratch_dir("wj_trailing");
        let path = dir.join("missing_dir").join("");
        assert!(write_json(&report_with(summary()), &path).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_blocked_parent_is_error_and_leaves_no_file() {
        let dir = scratch_dir("wj_blocked");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("run.json");
        assert!(write_json(&report_with(summary()), &path).is_err());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_json_concurrent_distinct_files_all_succeed() {
        let dir = scratch_dir("wj_concurrent");
        thread::scope(|scope| {
            for i in 0..8u16 {
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

    // ---------- write_markdown ----------

    #[test]
    fn write_markdown_creates_file_and_returns_ok() {
        let dir = scratch_dir("wm_create");
        let path = dir.join("run.md");
        assert!(write_markdown(&report_with(full_summary()), &path).is_ok());
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_file_matches_to_markdown_exactly() {
        let dir = scratch_dir("wm_exact");
        let path = dir.join("run.md");
        let r = report_with(full_summary());
        write_markdown(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), to_markdown(&r));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_file_is_not_json() {
        let dir = scratch_dir("wm_not_json");
        let path = dir.join("run.md");
        write_markdown(&report_with(full_summary()), &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.trim_start().starts_with('{'));
        assert!(serde_json::from_str::<Value>(&text).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_file_has_table_header() {
        let dir = scratch_dir("wm_header");
        let path = dir.join("run.md");
        write_markdown(&report_with(full_summary()), &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("| Metric | Median | Min | Max | Valid |"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_creates_missing_parent() {
        let dir = scratch_dir("wm_parent");
        let path = dir.join("results").join("run.md");
        write_markdown(&report_with(full_summary()), &path).unwrap();
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_overwrites_existing_file_completely() {
        let dir = scratch_dir("wm_overwrite");
        let path = dir.join("run.md");
        fs::write(
            &path,
            "old text that is much longer than the new table would be",
        )
        .unwrap();
        let r = report_with(empty_summary(1));
        write_markdown(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), to_markdown(&r));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_is_idempotent() {
        let dir = scratch_dir("wm_twice");
        let path = dir.join("run.md");
        let r = report_with(full_summary());
        write_markdown(&r, &path).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        write_markdown(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_directory_target_is_error() {
        let dir = scratch_dir("wm_is_dir");
        let target = dir.join("already_dir");
        fs::create_dir_all(&target).unwrap();
        assert!(write_markdown(&report_with(full_summary()), &target).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_blocked_parent_is_error_and_leaves_no_file() {
        let dir = scratch_dir("wm_blocked");
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let path = blocker.join("run.md");
        assert!(write_markdown(&report_with(full_summary()), &path).is_err());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_markdown_empty_path_is_error() {
        assert!(write_markdown(&report_with(full_summary()), Path::new("")).is_err());
    }

    #[test]
    fn write_markdown_concurrent_distinct_files_all_succeed() {
        let dir = scratch_dir("wm_concurrent");
        thread::scope(|scope| {
            for i in 0..8u32 {
                let path = dir.join(format!("run_{i}.md"));
                scope.spawn(move || {
                    let r = report_with(full_summary());
                    write_markdown(&r, &path).unwrap();
                    assert_eq!(fs::read_to_string(&path).unwrap(), to_markdown(&r));
                });
            }
        });
        let _ = fs::remove_dir_all(&dir);
    }

    // ---------- writers together ----------

    #[test]
    fn json_and_markdown_written_to_different_files_both_correct() {
        let dir = scratch_dir("both");
        let r = report("sleep 0.1", 2, 5, 7, full_summary());
        write_json(&r, &dir.join("run.json")).unwrap();
        write_markdown(&r, &dir.join("run.md")).unwrap();
        let json: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(json["timestamp"], 7);
        assert_eq!(
            fs::read_to_string(dir.join("run.md")).unwrap(),
            to_markdown(&r)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn markdown_write_after_json_write_replaces_same_path_contents() {
        let dir = scratch_dir("same_path");
        let path = dir.join("out");
        let r = report_with(full_summary());
        write_json(&r, &path).unwrap();
        write_markdown(&r, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), to_markdown(&r));
        let _ = fs::remove_dir_all(&dir);
    }
}
