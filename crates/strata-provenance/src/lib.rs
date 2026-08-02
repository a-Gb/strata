//! Provenance DAG and evidence dependency contracts.
#![forbid(unsafe_code)]

use strata_core::{
    ArtifactId, ByteRangeSet, EvidenceId, ProvenanceToken, SourceGeneration, SourceId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Semantic role of a node in a provenance derivation graph.
pub enum ProvenanceNodeKind {
    /// Immutable source identity or snapshot.
    Source,
    /// Exact or sampled set of source ranges.
    RangeSet,
    /// Reproducible data transformation.
    Transform,
    /// Versioned analysis implementation.
    Analyzer,
    /// Sampling decision applied before derivation.
    Sampling,
    /// Materialized analysis result.
    Artifact,
    /// View configuration that presented an artifact.
    View,
    /// Analyst-confirmed claim linked to supporting derivations.
    Evidence,
    /// Export operation and its fixed parameters.
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One immutable node in the provenance directed acyclic graph.
pub struct ProvenanceNode {
    /// Stable identity of this derivation node.
    pub token: ProvenanceToken,
    /// Semantic role of the node.
    pub kind: ProvenanceNodeKind,
    /// Versioned identifier defining the node's semantics.
    pub semantics_id: String,
    /// Canonical JSON parameters required to reproduce the node.
    pub parameter_json: String,
    /// Ordered dependency tokens consumed by this node.
    pub input_tokens: Vec<ProvenanceToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Exact source coverage and provenance root of one analysis artifact.
pub struct ArtifactProvenance {
    /// Artifact whose derivation is described.
    pub artifact_id: ArtifactId,
    /// Immutable source from which the artifact was derived.
    pub source_id: SourceId,
    /// Source generation inspected by the derivation.
    pub generation: SourceGeneration,
    /// Source ranges represented by the artifact.
    pub ranges: ByteRangeSet,
    /// Root token of the artifact's derivation graph.
    pub root: ProvenanceToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Source-free header of an analyst evidence record.
pub struct EvidenceRecordHeader {
    /// Stable evidence identity.
    pub id: EvidenceId,
    /// Human-readable claim made by the record.
    pub claim: String,
    /// Workflow status such as candidate, supported, or contradicted.
    pub status: String,
    /// Explanation of how confidence was established.
    pub confidence_basis: String,
    /// Derivation roots supporting or contradicting the claim.
    pub provenance_roots: Vec<ProvenanceToken>,
    /// Prior evidence record replaced by this revision, if any.
    pub supersedes: Option<EvidenceId>,
}

/// Persistence boundary for immutable provenance nodes and evidence headers.
pub trait ProvenanceStore: Send + Sync {
    /// Records a node after validating dependencies and acyclicity.
    ///
    /// # Errors
    ///
    /// Returns [`ProvenanceError`] when a dependency is absent, a token is
    /// duplicated, a cycle would be introduced, or storage fails.
    fn record_node(&self, node: ProvenanceNode) -> Result<(), ProvenanceError>;
    /// Retrieves a provenance node by token.
    ///
    /// # Errors
    ///
    /// Returns [`ProvenanceError::Storage`] when the backing store cannot be read.
    fn get_node(&self, token: ProvenanceToken) -> Result<Option<ProvenanceNode>, ProvenanceError>;
    /// Seals an evidence header against already-recorded provenance roots.
    ///
    /// # Errors
    ///
    /// Returns [`ProvenanceError`] when a root is absent, an identity is
    /// duplicated, a cycle would be introduced, or storage fails.
    fn seal_evidence(&self, record: EvidenceRecordHeader) -> Result<(), ProvenanceError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Failure produced while validating or persisting provenance.
pub enum ProvenanceError {
    /// A referenced dependency token has not been recorded.
    MissingDependency(ProvenanceToken),
    /// Adding a node or evidence revision would create a cycle.
    CycleDetected,
    /// A token already identifies another immutable node.
    DuplicateToken(ProvenanceToken),
    /// Backing storage failed with the supplied safe diagnostic.
    Storage(String),
}
