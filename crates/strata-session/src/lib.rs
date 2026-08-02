//! Serializable session state, reducer, journal, and recovery contracts.
#![forbid(unsafe_code)]

mod bundle;

pub use bundle::{
    BundleManifest, Journal, JournalEntry, JournalEvent, Reattachment, SessionBundle,
    SessionBundleError, SourceFingerprint, WorkspaceSnapshot,
};

use strata_core::{CommandId, DomainError, EvidenceId, Selection, SessionId, ViewSpec};
use strata_provenance::EvidenceRecordHeader;

#[derive(Debug, Clone, PartialEq)]
/// Serializable source-free state of one investigation session.
pub struct SessionState {
    /// Stable session identity.
    pub id: SessionId,
    /// Version of the session-state contract.
    pub schema_version: String,
    /// Canonical JSON references to sources without embedded bytes.
    pub source_references_json: String,
    /// Views retained by the session.
    pub views: Vec<ViewSpec>,
    /// View-independent selections retained by the session.
    pub selections: Vec<Selection>,
    /// Evidence records referenced by the session.
    pub evidence_ids: Vec<EvidenceId>,
    /// Canonical JSON for application-specific workspace state.
    pub workspace_json: String,
    /// Canonical JSON for source-free plugin state.
    pub plugin_state_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// User or system intent submitted to the session reducer.
pub struct SessionCommand {
    /// Stable command identity.
    pub id: CommandId,
    /// Command kind understood by the reducer.
    pub kind: String,
    /// Canonical JSON command parameters.
    pub parameter_json: String,
    /// Prior command that caused this command, when applicable.
    pub caused_by: Option<CommandId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Append-only state transition emitted by the session reducer.
pub struct SessionEvent {
    /// Monotonic zero-based event sequence.
    pub sequence: u64,
    /// Command responsible for the event.
    pub command_id: CommandId,
    /// Event kind understood by replay.
    pub kind: String,
    /// Canonical JSON event payload.
    pub payload_json: String,
    /// Whether the reducer supplies a corresponding inverse operation.
    pub undoable: bool,
}

/// Deterministic boundary for applying commands and producing journal events.
pub trait SessionReducer: Send + Sync {
    /// Applies one command to a session state.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the command cannot be applied.
    fn reduce(
        &self,
        state: &SessionState,
        command: SessionCommand,
    ) -> Result<(SessionState, Vec<SessionEvent>), DomainError>;
}

/// Persistence boundary for session snapshots, events, and sealed evidence.
pub trait SessionStore: Send + Sync {
    /// Loads a session state.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the state cannot be loaded.
    fn load(&self, session_id: SessionId) -> Result<SessionState, DomainError>;
    /// Appends session events.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the events cannot be persisted.
    fn append(&self, events: &[SessionEvent]) -> Result<(), DomainError>;
    /// Persists a session snapshot.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the snapshot cannot be persisted.
    fn snapshot(&self, state: &SessionState) -> Result<(), DomainError>;
    /// Permanently seals an evidence record.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the record cannot be sealed.
    fn seal_evidence(&self, record: EvidenceRecordHeader) -> Result<(), DomainError>;
}
