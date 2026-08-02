//! Export inventory, redaction, and atomic writer contracts.
#![forbid(unsafe_code)]

use strata_core::{DomainError, EvidenceId, ViewId};
use strata_session::SessionState;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Kind of deliverable requested from an exporter.
pub enum ExportKind {
    /// Data-faithful image with analytical channels and legends.
    AnalyticalImage,
    /// Presentation-oriented image that may use illustrative styling.
    IllustrativeImage,
    /// Machine-readable derived data.
    Data,
    /// Exact selected source ranges, subject to source-byte policy.
    SelectedRanges,
    /// Versioned view or analysis preset.
    ReproduciblePreset,
    /// Source-free investigation session bundle.
    SessionBundle,
    /// Evidence report with provenance references.
    EvidenceReport,
    /// Interchangeable three-dimensional scene description.
    ThreeDimensionalScene,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Whether one class of potentially identifying metadata is retained.
pub enum RedactionDisposition {
    /// Retain the field in the export.
    Retain,
    /// Remove the field from the export.
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Policy governing whether immutable source bytes may enter an export.
pub enum SourceBytePolicy {
    /// Do not include source bytes.
    Omit,
    /// Include at most the declared number of source bytes.
    Include {
        /// Hard byte limit enforced before export materialization.
        maximum_bytes: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Explicit redaction choices applied before an export is written.
pub struct RedactionPolicy {
    /// Disposition of local and remote source paths.
    pub paths: RedactionDisposition,
    /// Disposition of user-facing source names.
    pub source_names: RedactionDisposition,
    /// Disposition of analyst identity fields.
    pub analyst_identity: RedactionDisposition,
    /// Disposition of wall-clock timestamps.
    pub timestamps: RedactionDisposition,
    /// Whether source bytes may be included and under what bound.
    pub source_bytes: SourceBytePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Complete request to plan or execute an export.
pub struct ExportRequest {
    /// Deliverable kind.
    pub kind: ExportKind,
    /// Destination interpreted by the active export host.
    pub destination: String,
    /// Views included in the export.
    pub view_ids: Vec<ViewId>,
    /// Evidence records included in the export.
    pub evidence_ids: Vec<EvidenceId>,
    /// Whether sampled, approximate, or heuristic data must fail closed.
    pub require_exact: bool,
    /// Redaction policy applied to every generated file.
    pub redaction: RedactionPolicy,
    /// Canonical JSON parameters specific to the export kind.
    pub parameter_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Planned or completed file inventory for an export.
pub struct ExportInventory {
    /// Files produced or expected relative to the destination root.
    pub files: Vec<InventoryFile>,
    /// Canonical JSON disclosure of included source-byte ranges.
    pub source_byte_ranges_json: String,
    /// Accuracy, redaction, or compatibility warnings.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Digest-pinned description of one export file.
pub struct InventoryFile {
    /// Normalized path relative to the export root.
    pub relative_path: String,
    /// Media type of the file contents.
    pub media_type: String,
    /// Final file length in bytes.
    pub byte_len: u64,
    /// Lowercase digest of the final file contents.
    pub digest: String,
}

/// Boundary for validating, planning, and atomically materializing exports.
pub trait Exporter: Send + Sync {
    /// Validates a request and returns its expected inventory without writing.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when source state, exactness, redaction, resource,
    /// or destination constraints prevent a safe export.
    fn plan(
        &self,
        session: &SessionState,
        request: &ExportRequest,
    ) -> Result<ExportInventory, DomainError>;
    /// Materializes a validated export and returns its final inventory.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when validation fails or any file cannot be
    /// written and finalized atomically.
    fn execute(
        &self,
        session: &SessionState,
        request: ExportRequest,
    ) -> Result<ExportInventory, DomainError>;
}
