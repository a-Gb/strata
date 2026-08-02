//! Capability-scoped external plugin host contracts.
#![forbid(unsafe_code)]

use strata_core::{DomainError, PluginId};
use strata_plugin_api::{CapabilityRequest, PluginManifest, PluginOutput, PluginSourceHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Per-instance resource ceilings enforced by the component runtime.
pub struct PluginQuota {
    /// Maximum linear memory available to the plugin.
    pub memory_bytes: u64,
    /// Maximum instruction fuel available before replenishment or termination.
    pub fuel: u64,
    /// Maximum elapsed time allowed for one invocation.
    pub wall_time_ms: u64,
    /// Maximum combined bytes returned by one invocation.
    pub maximum_output_bytes: u64,
    /// Maximum plugin calls that may execute concurrently.
    pub maximum_concurrent_calls: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Host policy decision for one requested plugin capability.
pub struct CapabilityGrant {
    /// Original capability request from the signed manifest.
    pub request: CapabilityRequest,
    /// Whether host policy currently authorizes the request.
    pub granted: bool,
    /// Canonical JSON that further narrows the authorized operation.
    pub scope_json: String,
}

/// Resolves plugin capability policy and source access without exposing raw handles.
pub trait CapabilityBroker: Send + Sync {
    /// Returns the current policy decisions for an installed plugin.
    ///
    /// # Errors
    ///
    /// Returns an error when the plugin is unknown or policy state cannot be loaded.
    fn grants_for(&self, plugin: PluginId) -> Result<Vec<CapabilityGrant>, DomainError>;
    /// Opens a source handle constrained by plugin policy and the versioned request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is malformed, unauthorized, or cannot preserve an
    /// immutable source generation and exact approved ranges.
    fn open_source_handle(
        &self,
        plugin: PluginId,
        request_json: &str,
    ) -> Result<PluginSourceHandle, DomainError>;
}

/// Isolated live component instance with bounded invocation and lifecycle operations.
pub trait PluginInstance: Send {
    /// Returns the verified manifest associated with this instance.
    fn manifest(&self) -> &PluginManifest;
    /// Invokes one declared operation with versioned canonical JSON input.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation is unsupported, input is invalid, policy denies a
    /// capability, a quota is exceeded, or output validation fails.
    fn invoke(
        &mut self,
        operation: &str,
        input_json: &str,
    ) -> Result<Vec<PluginOutput>, DomainError>;
    /// Captures bounded plugin-local state for a resumable session checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when state serialization fails or exceeds the instance quota.
    fn checkpoint_state(&mut self) -> Result<Vec<u8>, DomainError>;
    /// Terminates the component and releases all instance-owned resources.
    fn terminate(&mut self);
}

/// Installs, instantiates, and revokes externally supplied plugin components.
pub trait PluginHost: Send + Sync {
    /// Verifies and installs a plugin bundle from a local path.
    ///
    /// # Errors
    ///
    /// Returns an error when the bundle is unreadable, invalid, incompatible, or fails integrity
    /// and policy validation.
    fn install(&self, bundle_path: &str) -> Result<PluginManifest, DomainError>;
    /// Creates an isolated instance under explicit resource ceilings.
    ///
    /// # Errors
    ///
    /// Returns an error when the plugin is absent, revoked, incompatible, or cannot be admitted
    /// under `quota`.
    fn instantiate(
        &self,
        plugin: PluginId,
        quota: PluginQuota,
    ) -> Result<Box<dyn PluginInstance>, DomainError>;
    /// Revokes an installed plugin and prevents future instantiation.
    ///
    /// # Errors
    ///
    /// Returns an error when the plugin is unknown or durable revocation fails.
    fn revoke(&self, plugin: PluginId) -> Result<(), DomainError>;
}
