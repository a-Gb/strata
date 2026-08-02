//! Shared production-promotion runtime for Strata frontends.
//!
//! This crate owns bounded orchestration, not presentation. GUI and CLI clients
//! submit the same source-generation requests and receive the same immutable
//! artifacts. Source handles use immutable shared ownership; no ambient path,
//! source-byte, or UI state enters an artifact identity.
#![forbid(unsafe_code)]

mod comparison;
mod digest;
mod source;

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use strata_analysis::production::{
    ArtifactCacheStats, ProductionAnalysisRuntime, ProductionRuntimeConfig, ProductionRuntimeEvent,
    StructureEntropyArtifact, StructureEntropyPreset, StructureEntropyRequest,
};
use strata_core::{AnalysisRequestId, ByteRangeSet, DomainError, Priority};

pub use comparison::{
    TILED_DIFF_SEMANTICS, TiledDiffArtifact, TiledDiffConfig, TiledDiffTile, build_tiled_diff,
};
pub use digest::{DigestRuntimeEvent, RuntimeDigestRequest, SourceDigestArtifact};
pub use source::{
    AttachedSource, DEFAULT_PREVIEW_BYTES, ResidentSourceTile, SourceOverview,
    SourceOverviewConfig, build_source_overview,
};

/// One structure-analysis request resolved through an immutable attached source.
pub struct RuntimeStructureRequest {
    /// Caller-owned identity used for cancellation and event correlation.
    pub request_id: AnalysisRequestId,
    /// Immutable source generation to analyze.
    pub source: AttachedSource,
    /// Exact, normalized source ranges included in the artifact.
    pub ranges: ByteRangeSet,
    /// Deterministic parameters shared by every frontend.
    pub preset: StructureEntropyPreset,
    /// Scheduler priority for every bounded source read.
    pub priority: Priority,
}

/// Completed blocking analysis with cache provenance.
#[derive(Debug, Clone)]
pub struct StructureAnalysisOutcome {
    /// Immutable artifact produced by the shared analyzer semantics.
    pub artifact: Arc<StructureEntropyArtifact>,
    /// Whether source reads and analyzer work were avoided by a cache hit.
    pub cache_hit: bool,
}

/// Shared bounded analysis runtime used by interactive and headless clients.
pub struct InvestigationRuntime {
    analysis: ProductionAnalysisRuntime,
    digests: digest::DigestRuntime,
}

impl InvestigationRuntime {
    /// Starts the bounded analysis worker.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when resource limits are invalid or the worker
    /// thread cannot be started.
    pub fn new(config: ProductionRuntimeConfig) -> Result<Self, DomainError> {
        let digests = digest::DigestRuntime::new(digest::DigestRuntimeConfig {
            queue_capacity: config.queue_capacity,
            step_bytes: config.read_chunk_bytes,
            progress_interval_bytes: 64 * 1024 * 1024,
        })?;
        ProductionAnalysisRuntime::new(config).map(|analysis| Self { analysis, digests })
    }

    /// Queues source-backed structure analysis through the common frontend seam.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the request is invalid, stale, duplicated,
    /// over budget, or cannot enter the bounded worker queue.
    pub fn submit_structure(&self, request: RuntimeStructureRequest) -> Result<(), DomainError> {
        self.analysis.submit(StructureEntropyRequest {
            request_id: request.request_id,
            source: request.source.byte_source(),
            ranges: request.ranges,
            preset: request.preset,
            priority: request.priority,
        })
    }

    /// Returns the next analysis event without blocking.
    #[must_use]
    pub fn poll_event(&self) -> Option<ProductionRuntimeEvent> {
        self.analysis.poll_event()
    }

    /// Marks an active or queued request cancelled.
    #[must_use]
    pub fn cancel(&self, request_id: AnalysisRequestId) -> bool {
        self.analysis.cancel(request_id)
    }

    /// Returns current bounded artifact-cache occupancy.
    #[must_use]
    pub fn cache_stats(&self) -> ArtifactCacheStats {
        self.analysis.cache_stats()
    }

    /// Queues progressive whole-source hashing without blocking the caller.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the request is duplicated, stale, lacks a
    /// known source length, or cannot enter the bounded digest queue.
    pub fn submit_digest(&self, request: RuntimeDigestRequest) -> Result<(), DomainError> {
        self.digests.submit(request)
    }

    /// Returns the next progressive digest event without blocking.
    #[must_use]
    pub fn poll_digest_event(&self) -> Option<DigestRuntimeEvent> {
        self.digests.poll_event()
    }

    /// Marks a queued or active digest request cancelled.
    #[must_use]
    pub fn cancel_digest(&self, request_id: AnalysisRequestId) -> bool {
        self.digests.cancel(request_id)
    }

    /// Runs one request to completion for a dedicated headless runtime.
    ///
    /// Interactive clients should use [`Self::submit_structure`] and
    /// [`Self::poll_event`] instead. This method consumes the runtime event
    /// stream until `request.request_id` reaches a terminal state.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when submission or analysis fails, the request
    /// is cancelled or stale, or the deadline expires.
    pub fn analyze_structure_blocking(
        &self,
        request: RuntimeStructureRequest,
        timeout: Duration,
    ) -> Result<StructureAnalysisOutcome, DomainError> {
        let request_id = request.request_id;
        self.submit_structure(request)?;
        let started = Instant::now();
        loop {
            if started.elapsed() >= timeout {
                let _ = self.cancel(request_id);
                return Err(DomainError::ResourceLimit(format!(
                    "analysis request {} exceeded its deadline",
                    request_id.0
                )));
            }
            match self.poll_event() {
                Some(ProductionRuntimeEvent::Completed {
                    request_id: completed,
                    artifact,
                    cache_hit,
                }) if completed == request_id => {
                    return Ok(StructureAnalysisOutcome {
                        artifact,
                        cache_hit,
                    });
                }
                Some(ProductionRuntimeEvent::Failed {
                    request_id: failed,
                    error,
                }) if failed == request_id => return Err(error),
                Some(ProductionRuntimeEvent::Cancelled {
                    request_id: cancelled,
                }) if cancelled == request_id => return Err(DomainError::Cancelled),
                Some(ProductionRuntimeEvent::Stale { request_id: stale })
                    if stale == request_id =>
                {
                    return Err(DomainError::StaleGeneration);
                }
                Some(ProductionRuntimeEvent::Started { .. }) | None => {
                    thread::park_timeout(Duration::from_millis(2));
                }
                Some(_) => {
                    return Err(DomainError::Internal(
                        "dedicated blocking runtime received an unrelated analysis event"
                            .to_owned(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_analysis::production::STRUCTURE_ENTROPY_SEMANTICS;
    use strata_core::{ByteRange, SourceGeneration, SourceId};

    #[test]
    fn blocking_and_event_driven_clients_share_artifact_semantics() -> Result<(), DomainError> {
        let bytes = Arc::<[u8]>::from(b"STRATA runtime parity fixture".repeat(32));
        let source =
            AttachedSource::retained(SourceId(8), SourceGeneration(1), "parity fixture", bytes)?;
        let descriptor = source.descriptor();
        let range = ByteRange::new(0, descriptor.length.unwrap_or(0))?;
        let runtime = InvestigationRuntime::new(ProductionRuntimeConfig::default())?;
        let outcome = runtime.analyze_structure_blocking(
            RuntimeStructureRequest {
                request_id: AnalysisRequestId(1),
                source,
                ranges: ByteRangeSet {
                    ranges: vec![range],
                },
                preset: StructureEntropyPreset::default(),
                priority: Priority::ExportCritical,
            },
            Duration::from_secs(5),
        )?;
        assert_eq!(STRUCTURE_ENTROPY_SEMANTICS, "strata.structure-entropy/v1");
        assert_eq!(outcome.artifact.covered_ranges.ranges, vec![range]);
        assert!(!outcome.artifact.artifact_digest.is_empty());
        Ok(())
    }
}
