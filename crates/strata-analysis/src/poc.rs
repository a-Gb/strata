//! Deterministic, dependency-free CPU analyzers used by the proof of concept.
//!
//! This facade keeps the original `strata_analysis::poc` API stable while the
//! implementation is organized by analytical responsibility. Every analyzer
//! operates on immutable bytes and retains exact source-relative provenance.

mod digram;
mod discovery;
mod resonance;
mod statistics;

#[cfg(test)]
mod tests;

pub use digram::{DigramCounts, digram_counts};
pub use discovery::{
    DiscoveryConfig, DiscoveryEvidence, DiscoveryFinding, DiscoveryFindingId, DiscoveryKind,
    MAX_DISCOVERY_BYTES, MAX_DISCOVERY_FINDINGS, MAX_DISCOVERY_WINDOWS,
    MIN_XOR_CORRELATED_DISTINCT_BYTES, discover_findings,
};
pub use resonance::{ResonanceMatch, ResonanceMetric, ResonanceScan, selection_resonance};
pub use statistics::{
    ByteClass, ByteHistogram, EntropyBlock, block_shannon_entropy, byte_histogram, classify_byte,
};
