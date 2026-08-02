//! View registry and analysis-demand contracts.
#![forbid(unsafe_code)]

/// Dependency-free image mappings used by the interactive POC.
pub mod poc;

/// Pure investigation state for findings, evidence, hypotheses, and linked navigation.
pub mod investigation;

/// Source-free selected-range metrics, cross-links, and next-action contracts.
pub mod dossier;

/// Pure contracts for regions, hypothesis branches, source comparison, and 3D cohorts.
pub mod workbench;

use strata_analysis::{AnalysisEnvelope, AnalysisRequest};
use strata_core::{DomainError, Selection, ViewId, ViewSpec};
use strata_render::SceneFragment;

#[derive(Debug, Clone, PartialEq)]
/// Immutable inputs shared by one view compilation or interaction update.
pub struct ViewContext {
    /// Versioned view configuration selected by the user or session.
    pub spec: ViewSpec,
    /// Exact source selections linked into the view.
    pub selections: Vec<Selection>,
    /// Canonical JSON describing the currently visible source domain.
    pub visible_domain_json: String,
    /// Canonical JSON for transient, view-specific interaction state.
    pub interaction_state_json: String,
}

/// Stateful controller that translates analysis artifacts and actions into a scene.
pub trait ViewController: Send {
    /// Returns the stable identity of this view instance.
    fn id(&self) -> ViewId;
    /// Declares the analysis artifacts required by the current view context.
    ///
    /// # Errors
    ///
    /// Returns an error when the view specification or visible domain is invalid.
    fn demands(&self, context: &ViewContext) -> Result<Vec<AnalysisRequest>, DomainError>;
    /// Incorporates a completed artifact into the view's retained state.
    ///
    /// # Errors
    ///
    /// Returns an error when the artifact is incompatible, stale, or malformed.
    fn update_artifact(&mut self, artifact: AnalysisEnvelope) -> Result<(), DomainError>;
    /// Compiles retained artifacts and exact selections into a declarative scene.
    ///
    /// # Errors
    ///
    /// Returns an error when required artifacts are absent or scene validation fails.
    fn compile_scene(&self, context: &ViewContext) -> Result<SceneFragment, DomainError>;
    /// Applies one versioned JSON interaction and returns emitted host commands.
    ///
    /// # Errors
    ///
    /// Returns an error when the action is invalid for this view or cannot preserve provenance.
    fn handle_action(&mut self, action_json: &str) -> Result<Vec<String>, DomainError>;
}

/// Constructs one supported kind and version of view controller.
pub trait ViewFactory: Send + Sync {
    /// Returns the view kind produced by this factory.
    fn kind(&self) -> &str;
    /// Returns the implementation version of the produced view contract.
    fn version(&self) -> &str;
    /// Creates a controller from a validated view specification.
    ///
    /// # Errors
    ///
    /// Returns an error when the specification kind, version, or parameters are unsupported.
    fn create(&self, spec: ViewSpec) -> Result<Box<dyn ViewController>, DomainError>;
}

/// Thread-safe catalog of installed view factories.
pub trait ViewRegistry: Send + Sync {
    /// Registers one factory under its kind and version.
    ///
    /// # Errors
    ///
    /// Returns an error when the registration conflicts with an existing factory.
    fn register(&self, factory: Box<dyn ViewFactory>) -> Result<(), DomainError>;
    /// Creates a controller using the factory selected by `spec`.
    ///
    /// # Errors
    ///
    /// Returns an error when no compatible factory exists or creation fails.
    fn create(&self, spec: ViewSpec) -> Result<Box<dyn ViewController>, DomainError>;
}
