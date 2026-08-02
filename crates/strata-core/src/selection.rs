//! View-independent selections.

use crate::{
    AddressSpaceId, ByteRangeSet, SelectionId, SourceGeneration, SourceId, TransformNodeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Semantic role a selection plays in an investigation.
pub enum SelectionRole {
    /// Main range currently under inspection.
    Primary,
    /// Range compared with the primary selection.
    Comparison,
    /// Range explicitly excluded from an operation.
    Exclusion,
    /// Range pinned as evidence.
    Evidence,
    /// Ephemeral hover, preview, or in-progress range.
    Transient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Precision with which a selection resolves back to source data.
pub enum SelectionCoverage {
    /// One exact contiguous source range.
    ExactContiguous,
    /// Several exact source ranges.
    ExactDiscontiguous,
    /// Exact contributors retained from a disclosed sample.
    SampledContributors,
    /// Only aggregate coverage is available, without individual contributors.
    AggregateOnly,
    /// Coverage is estimated rather than exact.
    Approximate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// View-independent source selection with exact provenance context.
pub struct Selection {
    /// Stable identity of the selection.
    pub id: SelectionId,
    /// User-facing selection label.
    pub label: String,
    /// Investigation role of the selection.
    pub role: SelectionRole,
    /// Immutable source to which the ranges belong.
    pub source_id: SourceId,
    /// Source generation against which the selection was resolved.
    pub generation: SourceGeneration,
    /// Coordinate system used by the stored ranges.
    pub address_space: AddressSpaceId,
    /// Selected ranges in the declared address space.
    pub ranges: ByteRangeSet,
    /// Precision of the source mapping.
    pub coverage: SelectionCoverage,
    /// Ordered transforms traversed from the source to this selection.
    pub transform_path: Vec<TransformNodeId>,
    /// Stable identifier of the command or view that created the selection.
    pub origin: String,
}
