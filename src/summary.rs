use crate::measure::RunSample;
use anyhow::Result;
pub struct MetricSummary {
    pub median: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub valid: u32,
    pub total: u32,
}
pub fn metric_sammarizer(samples_holder: &Vec<RunSample>) -> Result<MetricSummary> {
    Ok(MetricSummary)
}
