//! UI-facing commands and workbench layout model.
#![forbid(unsafe_code)]

use strata_core::{CommandId, DomainError, ViewId};
use strata_session::{SessionCommand, SessionState};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Complete set of typed commands that may cross the UI/application boundary.
pub enum AppCommand {
    /// Apply a deterministic mutation to retained session state.
    Session(SessionCommand),
    /// Open an immutable source through a versioned connector description.
    OpenSource {
        /// Stable identity used for idempotence and event correlation.
        command_id: CommandId,
        /// Canonical JSON connector configuration.
        connector_json: String,
    },
    /// Close a source identified by its canonical source descriptor.
    CloseSource {
        /// Stable identity used for idempotence and event correlation.
        command_id: CommandId,
        /// Canonical JSON identity for the source to close.
        source_json: String,
    },
    /// Request an exact, unsampled representation of a view's visible domain.
    RequestExactVisible {
        /// Stable identity used for idempotence and event correlation.
        command_id: CommandId,
        /// View whose current visible domain should be materialized exactly.
        view_id: ViewId,
    },
    /// Run a versioned export program through the host.
    Export {
        /// Stable identity used for idempotence and event correlation.
        command_id: CommandId,
        /// Canonical JSON export request.
        request_json: String,
    },
    /// Invoke an installed plugin through its capability-scoped host contract.
    InvokePlugin {
        /// Stable identity used for idempotence and event correlation.
        command_id: CommandId,
        /// Canonical JSON plugin invocation.
        invocation_json: String,
    },
    /// Dispatch a versioned integration action to an approved host bridge.
    Bridge {
        /// Stable identity used for idempotence and event correlation.
        command_id: CommandId,
        /// Canonical JSON bridge action.
        action_json: String,
    },
}

/// Application command boundary implemented by the native host runtime.
pub trait CommandBus: Send + Sync {
    /// Validates and dispatches one typed UI command.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, authorization, idempotence, or downstream execution
    /// fails.
    fn dispatch(&self, command: AppCommand) -> Result<(), DomainError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Serializable definition of one workbench panel instance.
pub struct PanelSpec {
    /// Stable panel identity within the session layout.
    pub id: String,
    /// Registered panel implementation kind.
    pub kind: String,
    /// Analyst-facing panel title.
    pub title: String,
    /// Canonical JSON panel state.
    pub state_json: String,
}

#[derive(Debug, Clone, PartialEq)]
/// Immutable UI snapshot assembled from session, layout, and runtime status state.
pub struct WorkbenchSnapshot {
    /// Current deterministic session state.
    pub session: SessionState,
    /// Ordered panel layout and retained panel state.
    pub panels: Vec<PanelSpec>,
    /// Canonical JSON describing non-error runtime status.
    pub status_json: String,
    /// Canonical JSON diagnostics when a user-actionable problem is active.
    pub diagnostics_json: Option<String>,
}
