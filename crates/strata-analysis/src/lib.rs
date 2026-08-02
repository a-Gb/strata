//! Analyzer, planner, scheduler, and typed artifact contracts.
#![forbid(unsafe_code)]

use std::{future::Future, pin::Pin};

use strata_core::{
    AnalysisIdentity, AnalysisRequestId, ArtifactDescriptor, ByteRangeSet, DataDomain, DomainError,
    Priority, SamplingPolicy, SourceGeneration, SourceId, TransformGraphSpec,
};
use strata_source::ByteSource;

pub mod poc;
pub mod production;
pub mod projection_p1;
pub mod signatures;
pub mod tiles;
pub mod workbench;

/// Heap-pinned, sendable future returned by object-safe analysis traits.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Complete source-bound request submitted to an analyzer.
pub struct AnalysisRequest {
    /// Stable request identity used for cancellation and publication.
    pub id: AnalysisRequestId,
    /// Immutable source to analyze.
    pub source_id: SourceId,
    /// Source generation required by the request.
    pub generation: SourceGeneration,
    /// Source ranges included in the analysis.
    pub ranges: ByteRangeSet,
    /// Reproducible transforms applied before analysis.
    pub transform: TransformGraphSpec,
    /// Versioned analyzer required by the request.
    pub analyzer: AnalysisIdentity,
    /// Canonical JSON analyzer parameters.
    pub parameter_json: String,
    /// Semantic output domain requested by the consumer.
    pub requested_domain: DataDomain,
    /// Policy controlling which source values are inspected.
    pub sampling: SamplingPolicy,
    /// Named numeric precision policy.
    pub precision: String,
    /// Named output resolution or level-of-detail policy.
    pub resolution: String,
    /// Scheduling importance of the request.
    pub priority: Priority,
}

#[derive(Debug, Clone, PartialEq)]
/// Typed payload carried by an analysis artifact envelope.
pub enum ArtifactPayload {
    /// Uninterpreted derived bytes.
    Bytes(Vec<u8>),
    /// Unsigned scalar values.
    UnsignedScalars(Vec<u64>),
    /// Floating-point scalar values.
    FloatScalars(Vec<f64>),
    /// Dense row-major unsigned matrix.
    MatrixU64 {
        /// Matrix width in values.
        width: u32,
        /// Matrix height in values.
        height: u32,
        /// Row-major matrix values.
        values: Vec<u64>,
    },
    /// Sparse weighted points on an unsigned three-dimensional lattice.
    SparsePoints3 {
        /// Point coordinates.
        xyz: Vec<[u32; 3]>,
        /// Per-point weights in matching order.
        weights: Vec<u64>,
    },
    /// Source-backed region findings.
    Regions(Vec<RegionFinding>),
    /// Versioned payload not represented by a built-in variant.
    Opaque {
        /// Media type defining the payload semantics.
        media_type: String,
        /// Opaque payload bytes.
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One source-backed region lead produced by an analyzer.
pub struct RegionFinding {
    /// Exact or sampled source ranges supporting the finding.
    pub ranges: ByteRangeSet,
    /// Stable finding kind.
    pub kind: String,
    /// User-facing finding label.
    pub label: String,
    /// Explanation of the evidence supporting confidence.
    pub confidence_basis: String,
    /// Canonical JSON attributes specific to the finding kind.
    pub attributes_json: String,
}

#[derive(Debug, Clone, PartialEq)]
/// Descriptor, payload, warnings, and metrics for one published artifact.
pub struct AnalysisEnvelope {
    /// Source coverage and provenance metadata.
    pub descriptor: ArtifactDescriptor,
    /// Typed artifact payload.
    pub payload: ArtifactPayload,
    /// Accuracy, sampling, or fallback warnings.
    pub warnings: Vec<String>,
    /// Canonical JSON execution metrics.
    pub metrics_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Static capabilities declared by an analyzer implementation.
pub struct AnalyzerCapabilities {
    /// Semantic output domains supported by the analyzer.
    pub supported_domains: Vec<DataDomain>,
    /// Whether a semantics-equivalent GPU implementation is available.
    pub gpu_eligible: bool,
    /// Whether incomplete artifacts may be published and refined later.
    pub supports_partial: bool,
    /// Whether individual source occurrences remain indexed in outputs.
    pub supports_occurrence_index: bool,
    /// User-facing deterministic or tolerance guarantee.
    pub deterministic_claim: String,
}

/// Versioned analysis implementation over bounded byte sources.
pub trait Analyzer: Send + Sync {
    /// Returns the implementation and semantics identity.
    fn identity(&self) -> &AnalysisIdentity;
    /// Returns static analyzer capabilities.
    fn capabilities(&self) -> &AnalyzerCapabilities;

    /// Analyzes a source according to a validated request.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the source is stale or mismatched, request
    /// semantics are unsupported, work is cancelled, or bounds are exceeded.
    fn analyze<'a>(
        &'a self,
        source: &'a dyn ByteSource,
        request: AnalysisRequest,
    ) -> BoxFuture<'a, Result<Vec<AnalysisEnvelope>, DomainError>>;
}

/// Scheduling boundary for cancellable analysis requests.
pub trait AnalysisScheduler: Send + Sync {
    /// Queues a request for bounded execution.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when validation fails or the scheduler cannot
    /// accept the request within its resource limits.
    fn submit(&self, request: AnalysisRequest) -> BoxFuture<'_, Result<(), DomainError>>;
    /// Requests cancellation and returns whether the request was active.
    fn cancel(&self, request_id: AnalysisRequestId) -> bool;
    /// Changes scheduling priority and returns whether the request was active.
    fn reprioritize(&self, request_id: AnalysisRequestId, priority: Priority) -> bool;
}
