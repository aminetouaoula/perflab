use crate::summarize_all::Summary;
use crate::summary::MetricSummary;
use anyhow::Result;
use serde::Serialize;
use serde_json::to_string_pretty;
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
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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

    // ---------- expected behavior ----------

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

    // ---------- missing values ----------

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

    // ---------- edge cases ----------

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
}
