use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

pub fn parse_perf_csv(text: &str) -> Result<BTreeMap<String, Option<u64>>> {
    let mut counters = BTreeMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 3 {
            continue;
        }

        let raw_value = fields[0].trim();
        let event = fields[2].trim();
        let name = event.split(':').next().unwrap_or(event);

        let value = if raw_value.starts_with('<') {
            None
        } else {
            let parsed = raw_value
                .parse::<u64>()
                .with_context(|| format!("invalid counter value {raw_value:?} for {event}"))?;
            Some(parsed)
        };

        if counters.insert(name.to_string(), value).is_some() {
            bail!("duplicate event {name} in perf output");
        }
    }

    Ok(counters)
}
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
305768,,cycles:u,101033599,100.00,,
281095,,instructions:u,101033599,100.00,,
7452,,cache-misses:u,101033599,100.00,,
4617,,branch-misses:u,101033599,100.00,,
";

    #[test]
    fn parses_four_counters() {
        let c = parse_perf_csv(SAMPLE).unwrap();
        assert_eq!(c.len(), 4);
        assert_eq!(c["cycles"], Some(305768));
        assert_eq!(c["instructions"], Some(281095));
        assert_eq!(c["cache-misses"], Some(7452));
        assert_eq!(c["branch-misses"], Some(4617));
    }

    #[test]
    fn strips_modifier_suffix() {
        let c = parse_perf_csv("10,,cycles:u,1,100,,\n").unwrap();
        assert!(c.contains_key("cycles"));
        assert!(!c.contains_key("cycles:u"));
    }

    #[test]
    fn event_without_modifier_is_kept() {
        let c = parse_perf_csv("10,,cycles,1,100,,\n").unwrap();
        assert_eq!(c["cycles"], Some(10));
    }

    #[test]
    fn not_counted_becomes_none() {
        let c = parse_perf_csv("<not counted>,,branch-misses:u,0,0.00,,\n").unwrap();
        assert_eq!(c["branch-misses"], None);
    }

    #[test]
    fn not_supported_becomes_none() {
        let c = parse_perf_csv("<not supported>,,stalled-cycles-frontend:u,0,0.00,,\n").unwrap();
        assert_eq!(c["stalled-cycles-frontend"], None);
    }

    #[test]
    fn zero_is_some_zero_not_none() {
        let c = parse_perf_csv("0,,cache-misses:u,1,100,,\n").unwrap();
        assert_eq!(c["cache-misses"], Some(0));
    }

    #[test]
    fn large_value_fits_u64() {
        let c = parse_perf_csv("18446744073709551615,,cycles:u,1,100,,\n").unwrap();
        assert_eq!(c["cycles"], Some(u64::MAX));
    }

    #[test]
    fn empty_input_gives_empty_map() {
        assert!(parse_perf_csv("").unwrap().is_empty());
    }

    #[test]
    fn whitespace_only_input_gives_empty_map() {
        assert!(parse_perf_csv("   \n\n  \t\n").unwrap().is_empty());
    }

    #[test]
    fn handles_crlf_line_endings() {
        let c = parse_perf_csv("10,,cycles:u,1,100,,\r\n20,,instructions:u,1,100,,\r\n").unwrap();
        assert_eq!(c["cycles"], Some(10));
        assert_eq!(c["instructions"], Some(20));
    }

    #[test]
    fn no_trailing_newline_still_parses() {
        let c = parse_perf_csv("10,,cycles:u,1,100,,").unwrap();
        assert_eq!(c["cycles"], Some(10));
    }

    #[test]
    fn ignores_comment_lines() {
        let c = parse_perf_csv("# started on Mon\n10,,cycles:u,1,100,,\n").unwrap();
        assert_eq!(c.len(), 1);
    }
}
