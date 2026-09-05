//! Opt-in GPUI window-profiler summaries and budget checks.

use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

/// A compact summary of one GPUI profiler histogram.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceMetricSummary {
    /// Number of samples recorded by the window profiler.
    pub sample_count: u64,
    /// 95th percentile in nanoseconds, or `None` when no samples were recorded.
    pub p95_nanos: Option<u64>,
    /// Maximum sample in nanoseconds, or `None` when no samples were recorded.
    pub max_nanos: Option<u64>,
}

impl PerformanceMetricSummary {
    /// Returns the p95 value as a [`Duration`], when available.
    pub fn p95(&self) -> Option<Duration> {
        self.p95_nanos.map(Duration::from_nanos)
    }

    /// Returns the maximum value as a [`Duration`], when available.
    pub fn max(&self) -> Option<Duration> {
        self.max_nanos.map(Duration::from_nanos)
    }
}

/// GPUI frame metrics collected for one isolated story window.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceReport {
    /// Time spent in `Window::draw`.
    pub draw_duration: PerformanceMetricSummary,
    /// Time from the first invalidation to platform presentation.
    pub dirty_to_present: PerformanceMetricSummary,
}

impl PerformanceReport {
    /// Checks this report against a budget and returns all failed assertions.
    pub fn check(&self, budget: &PerformanceBudget) -> Result<(), PerformanceBudgetFailure> {
        let mut violations = Vec::new();
        check_metric(
            &mut violations,
            PerformanceMetric::DrawDuration,
            self.draw_duration,
            budget.min_draw_samples,
            budget.max_draw_p95_nanos,
            budget.max_draw_nanos,
        );
        check_metric(
            &mut violations,
            PerformanceMetric::DirtyToPresent,
            self.dirty_to_present,
            budget.min_dirty_to_present_samples,
            budget.max_dirty_to_present_p95_nanos,
            budget.max_dirty_to_present_nanos,
        );

        if violations.is_empty() {
            Ok(())
        } else {
            Err(PerformanceBudgetFailure {
                report: *self,
                violations,
            })
        }
    }

    /// Creates a summary from raw histogram values.
    ///
    /// This constructor is public so consumers that collect profiler snapshots
    /// through their own GPUI host can still use the same budget checker.
    pub const fn from_summaries(
        draw_duration: PerformanceMetricSummary,
        dirty_to_present: PerformanceMetricSummary,
    ) -> Self {
        Self {
            draw_duration,
            dirty_to_present,
        }
    }

    #[cfg(feature = "performance")]
    pub(crate) fn from_window(window: &gpui_kit::Window) -> Self {
        let snapshot = window.frame_duration_snapshot();
        let draw_duration = PerformanceMetricSummary {
            sample_count: snapshot.draw_duration_histogram.len(),
            p95_nanos: (!snapshot.draw_duration_histogram.is_empty())
                .then(|| snapshot.draw_duration_histogram.value_at_quantile(0.95)),
            max_nanos: (!snapshot.draw_duration_histogram.is_empty())
                .then(|| snapshot.draw_duration_histogram.max()),
        };
        let dirty_to_present = PerformanceMetricSummary {
            sample_count: snapshot.dirty_to_present_histogram.len(),
            p95_nanos: (!snapshot.dirty_to_present_histogram.is_empty())
                .then(|| snapshot.dirty_to_present_histogram.value_at_quantile(0.95)),
            max_nanos: (!snapshot.dirty_to_present_histogram.is_empty())
                .then(|| snapshot.dirty_to_present_histogram.max()),
        };
        Self::from_summaries(draw_duration, dirty_to_present)
    }
}

/// Limits applied to GPUI frame profiler metrics.
///
/// All duration limits are expressed in nanoseconds in the serialized form.
/// Use the duration builder methods when constructing a budget in Rust.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceBudget {
    /// Minimum number of draw samples required.
    pub min_draw_samples: Option<u64>,
    /// Maximum allowed draw p95, in nanoseconds.
    pub max_draw_p95_nanos: Option<u64>,
    /// Maximum allowed draw sample, in nanoseconds.
    pub max_draw_nanos: Option<u64>,
    /// Minimum number of dirty-to-present samples required.
    pub min_dirty_to_present_samples: Option<u64>,
    /// Maximum allowed dirty-to-present p95, in nanoseconds.
    pub max_dirty_to_present_p95_nanos: Option<u64>,
    /// Maximum allowed dirty-to-present sample, in nanoseconds.
    pub max_dirty_to_present_nanos: Option<u64>,
}

impl PerformanceBudget {
    /// Sets the minimum draw sample count.
    pub const fn with_min_draw_samples(mut self, samples: u64) -> Self {
        self.min_draw_samples = Some(samples);
        self
    }

    /// Sets the maximum draw p95 duration.
    pub const fn with_max_draw_p95(mut self, duration: Duration) -> Self {
        self.max_draw_p95_nanos = Some(duration.as_nanos() as u64);
        self
    }

    /// Sets the maximum draw duration.
    pub const fn with_max_draw(mut self, duration: Duration) -> Self {
        self.max_draw_nanos = Some(duration.as_nanos() as u64);
        self
    }

    /// Sets the minimum dirty-to-present sample count.
    pub const fn with_min_dirty_to_present_samples(mut self, samples: u64) -> Self {
        self.min_dirty_to_present_samples = Some(samples);
        self
    }

    /// Sets the maximum dirty-to-present p95 duration.
    pub const fn with_max_dirty_to_present_p95(mut self, duration: Duration) -> Self {
        self.max_dirty_to_present_p95_nanos = Some(duration.as_nanos() as u64);
        self
    }

    /// Sets the maximum dirty-to-present duration.
    pub const fn with_max_dirty_to_present(mut self, duration: Duration) -> Self {
        self.max_dirty_to_present_nanos = Some(duration.as_nanos() as u64);
        self
    }
}

/// Metric named by a failed performance-budget assertion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceMetric {
    /// Time spent in `Window::draw`.
    DrawDuration,
    /// Time from invalidation to presentation.
    DirtyToPresent,
}

/// Statistic named by a failed performance-budget assertion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceStatistic {
    /// 95th percentile duration.
    P95,
    /// Maximum duration.
    Max,
    /// Number of samples.
    SampleCount,
}

/// One typed performance-budget violation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceViolation {
    /// Metric that violated the budget.
    pub metric: PerformanceMetric,
    /// Statistic that violated the budget.
    pub statistic: PerformanceStatistic,
    /// Observed value in nanoseconds, or sample count for `SampleCount`.
    pub observed: u64,
    /// Allowed value in nanoseconds, or minimum count for `SampleCount`.
    pub limit: u64,
}

/// Typed failure containing both the complete report and every violation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PerformanceBudgetFailure {
    /// Full metric report used for the assertion.
    pub report: PerformanceReport,
    /// Failed budget assertions.
    pub violations: Vec<PerformanceViolation>,
}

impl PerformanceBudgetFailure {
    /// Returns the number of failed assertions.
    pub const fn violations_len(&self) -> usize {
        self.violations.len()
    }
}

impl fmt::Display for PerformanceBudgetFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "performance budget failed with {} violation(s)",
            self.violations.len()
        )
    }
}

impl std::error::Error for PerformanceBudgetFailure {}

impl From<PerformanceBudgetFailure> for PerformanceReport {
    fn from(value: PerformanceBudgetFailure) -> Self {
        value.report
    }
}

impl fmt::Display for PerformanceViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} {:?}: observed {}, limit {}",
            self.metric, self.statistic, self.observed, self.limit
        )
    }
}

fn check_metric(
    violations: &mut Vec<PerformanceViolation>,
    metric: PerformanceMetric,
    summary: PerformanceMetricSummary,
    minimum_samples: Option<u64>,
    p95_limit: Option<u64>,
    max_limit: Option<u64>,
) {
    if let Some(limit) = minimum_samples
        && summary.sample_count < limit
    {
        violations.push(PerformanceViolation {
            metric,
            statistic: PerformanceStatistic::SampleCount,
            observed: summary.sample_count,
            limit,
        });
    }
    if let Some(limit) = p95_limit
        && summary.p95_nanos.is_none_or(|value| value > limit)
    {
        violations.push(PerformanceViolation {
            metric,
            statistic: PerformanceStatistic::P95,
            observed: summary.p95_nanos.unwrap_or_default(),
            limit,
        });
    }
    if let Some(limit) = max_limit
        && summary.max_nanos.is_none_or(|value| value > limit)
    {
        violations.push(PerformanceViolation {
            metric,
            statistic: PerformanceStatistic::Max,
            observed: summary.max_nanos.unwrap_or_default(),
            limit,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(count: u64, p95: u64, max: u64) -> PerformanceMetricSummary {
        PerformanceMetricSummary {
            sample_count: count,
            p95_nanos: Some(p95),
            max_nanos: Some(max),
        }
    }

    #[test]
    fn budget_reports_p95_max_and_sample_count_violations() {
        let report = PerformanceReport::from_summaries(summary(2, 20, 30), summary(1, 40, 50));
        let budget = PerformanceBudget::default()
            .with_min_draw_samples(3)
            .with_max_draw_p95(Duration::from_nanos(10))
            .with_max_draw(Duration::from_nanos(25))
            .with_min_dirty_to_present_samples(2)
            .with_max_dirty_to_present_p95(Duration::from_nanos(35))
            .with_max_dirty_to_present(Duration::from_nanos(45));

        let failure = report.check(&budget).expect_err("budget should fail");
        assert_eq!(failure.violations.len(), 6);
        assert_eq!(
            failure.violations[0].statistic,
            PerformanceStatistic::SampleCount
        );
        assert_eq!(failure.violations[1].statistic, PerformanceStatistic::P95);
        assert_eq!(failure.violations[2].statistic, PerformanceStatistic::Max);
        assert_eq!(
            failure.violations[3].statistic,
            PerformanceStatistic::SampleCount
        );
        assert_eq!(failure.violations[4].statistic, PerformanceStatistic::P95);
        assert_eq!(failure.violations[5].statistic, PerformanceStatistic::Max);
    }

    #[test]
    fn empty_metrics_fail_duration_assertions() {
        let report = PerformanceReport::default();
        let budget = PerformanceBudget::default()
            .with_max_draw_p95(Duration::from_nanos(1))
            .with_max_dirty_to_present(Duration::from_nanos(1));
        let failure = report
            .check(&budget)
            .expect_err("empty metrics should fail");
        assert_eq!(failure.violations.len(), 2);
        assert_eq!(failure.violations[0].observed, 0);
    }
}
