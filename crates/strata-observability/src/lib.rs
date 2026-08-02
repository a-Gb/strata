//! Source-safe tracing, metrics, and diagnostic snapshot contracts.
#![forbid(unsafe_code)]

use strata_core::{AnalysisRequestId, CommandId, SourceId, ViewId};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Source-safe correlation identifiers propagated across one operation.
pub struct TraceContext {
    /// Identity shared by all spans in one trace.
    pub trace_id: u128,
    /// Identity of the current span.
    pub span_id: u64,
    /// Command that initiated the operation, when applicable.
    pub command_id: Option<CommandId>,
    /// Analysis request represented by the span, when applicable.
    pub request_id: Option<AnalysisRequestId>,
    /// Opaque source identity without path or source bytes.
    pub source_id: Option<SourceId>,
    /// View associated with the operation, when applicable.
    pub view_id: Option<ViewId>,
}

#[derive(Debug, Clone, PartialEq)]
/// Value accepted by the metrics boundary.
pub enum MetricValue {
    /// Monotonic event count.
    Counter(u64),
    /// Point-in-time scalar measurement.
    Gauge(f64),
    /// One observation added to a histogram distribution.
    HistogramSample(f64),
}

#[derive(Debug, Clone, PartialEq)]
/// One source-safe metric observation.
pub struct MetricRecord {
    /// Stable metric name.
    pub name: String,
    /// Recorded metric value.
    pub value: MetricValue,
    /// Low-cardinality labels reviewed to exclude paths and source contents.
    pub safe_labels: Vec<(String, String)>,
}

/// Sink boundary for source-safe metric observations.
pub trait MetricSink: Send + Sync {
    /// Records one metric without exposing source content.
    fn record(&self, record: MetricRecord);
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Redacted diagnostic state suitable for support bundles.
pub struct DiagnosticSnapshot {
    /// Canonical JSON summary of active jobs.
    pub active_jobs_json: String,
    /// Canonical JSON summary of cache occupancy and limits.
    pub cache_json: String,
    /// Canonical JSON summary of active GPU adapters and faults.
    pub gpu_json: String,
    /// Canonical JSON summary of frame timing and quality policy.
    pub frame_json: String,
    /// Canonical JSON summary of plugin lifecycle state.
    pub plugin_json: String,
    /// Number of completed results rejected because their generation was stale.
    pub stale_result_count: u64,
}
