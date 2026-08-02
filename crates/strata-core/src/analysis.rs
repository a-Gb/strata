//! Shared analysis semantics.

use crate::{ArtifactId, ByteRangeSet, ProvenanceToken, SourceGeneration, SourceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Scheduling importance assigned to an analysis request.
pub enum Priority {
    /// Opportunistic work that must not delay visible results.
    Background,
    /// Anticipatory work for data likely to become visible next.
    Prefetch,
    /// Work needed to populate the current view.
    Visible,
    /// Latency-sensitive work caused by direct user interaction.
    Interactive,
    /// Work required to complete an explicit export accurately.
    ExportCritical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Reproducible policy describing which source values an analysis inspected.
pub enum SamplingPolicy {
    /// Inspect every value in the covered range.
    Exact,
    /// Retain values at a fixed periodic step and phase.
    Systematic {
        /// Distance between retained values.
        step: u64,
        /// Initial offset within the sampling period.
        phase: u64,
    },
    /// Retain a fixed number of seeded samples from each partition.
    Stratified {
        /// Number of partitions across the covered domain.
        strata: u32,
        /// Number of retained values in each partition.
        per_stratum: u32,
        /// Seed that makes sample choice reproducible.
        seed: u64,
    },
    /// Maintain a fixed-size seeded reservoir across a stream.
    Reservoir {
        /// Maximum number of retained values.
        samples: u64,
        /// Seed that makes replacement decisions reproducible.
        seed: u64,
    },
    /// Use one declared level of a deterministic multiresolution pyramid.
    PyramidLevel {
        /// Zero-based pyramid level, where zero is the finest level.
        level: u8,
    },
    /// Use a named deterministic adaptive sampler.
    Adaptive {
        /// Stable identifier for the adaptive sampling algorithm and semantics.
        policy_id: String,
        /// Seed used by the adaptive policy.
        seed: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Strength of the claim an artifact can make about its covered source.
pub enum Exactness {
    /// The result is exact for every declared covered value.
    Exact,
    /// The result is approximate within a declared finite bound.
    BoundedApproximation,
    /// The result describes only values retained by its sampling policy.
    Sampled,
    /// The result is a heuristic lead and is not proof by itself.
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Progress state of an artifact over its declared coverage.
pub enum Completeness {
    /// Only part of the requested coverage has been analyzed.
    Partial,
    /// A prior result has been improved but may still be incomplete.
    Refined,
    /// All work promised by the artifact contract has finished.
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Versioned identity of an analysis implementation and its meaning.
pub struct AnalysisIdentity {
    /// Stable analyzer identifier.
    pub id: String,
    /// Implementation version.
    pub version: String,
    /// Version of the result semantics independent of implementation changes.
    pub semantics_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Source-bound metadata required to interpret and reproduce an analysis artifact.
pub struct ArtifactDescriptor {
    /// Stable artifact identity.
    pub id: ArtifactId,
    /// Immutable source to which the artifact belongs.
    pub source_id: SourceId,
    /// Source generation inspected by the analyzer.
    pub generation: SourceGeneration,
    /// Source ranges represented by the artifact.
    pub covered_ranges: ByteRangeSet,
    /// Accuracy class of the artifact's claims.
    pub exactness: Exactness,
    /// Progress state of the requested analysis.
    pub completeness: Completeness,
    /// Policy used to choose inspected source values.
    pub sampling: SamplingPolicy,
    /// Token resolving to the artifact's derivation record.
    pub provenance: ProvenanceToken,
    /// Media type identifying the artifact payload format.
    pub media_type: String,
}
