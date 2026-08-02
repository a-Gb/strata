//! Host-side mirror of the versioned WIT plugin contract.
#![forbid(unsafe_code)]

use strata_analysis::{ArtifactPayload, RegionFinding};
use strata_core::{ByteRangeSet, PluginId, SourceGeneration, SourceId};
use strata_render::SceneFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Versioned identity, capability request, and integrity metadata for one plugin.
pub struct PluginManifest {
    /// Stable host-facing plugin identity.
    pub id: PluginId,
    /// Globally unique textual identifier used by package and policy tooling.
    pub canonical_id: String,
    /// Human-readable display name.
    pub name: String,
    /// Plugin package version.
    pub version: String,
    /// Oldest host API version with which the plugin is compatible.
    pub minimum_host_api: String,
    /// Capabilities that require an explicit host policy decision.
    pub requested_capabilities: Vec<CapabilityRequest>,
    /// Component roles implemented by the plugin.
    pub plugin_types: Vec<String>,
    /// Canonical serialized resource limits requested by the plugin.
    pub resource_policy_json: String,
    /// Digest of the exact component bytes described by this manifest.
    pub component_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A narrowly scoped operation that a plugin may ask the host to authorize.
pub enum CapabilityRequest {
    /// Read non-byte source descriptors such as length and content identity.
    ReadSourceMetadata,
    /// Read approved source ranges through opaque handles.
    ReadSourceRanges {
        /// Maximum number of source bytes returned by one host call.
        maximum_bytes_per_call: u64,
    },
    /// Ask the host to execute an allow-listed set of analyzers.
    RequestHostAnalysis {
        /// Analyzer identities that the plugin may request.
        analyzer_ids: Vec<String>,
    },
    /// Emit range-backed findings for host validation and presentation.
    EmitFindings,
    /// Emit a declarative scene fragment without direct GPU access.
    EmitDeclarativeScene,
    /// Persist bounded plugin-owned state.
    PluginLocalStorage {
        /// Maximum retained storage in bytes.
        maximum_bytes: u64,
    },
    /// Make network requests only to an explicit domain allow-list.
    Network {
        /// Domain names approved for this plugin.
        allowed_domains: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Opaque, range-limited reference to one immutable source generation.
pub struct PluginSourceHandle {
    /// Host-issued value used for subsequent plugin calls.
    pub opaque_handle: u64,
    /// Stable identity of the source exposed through the handle.
    pub source_id: SourceId,
    /// Exact immutable source generation exposed through the handle.
    pub generation: SourceGeneration,
    /// Source ranges that host policy permits the plugin to read.
    pub approved_ranges: ByteRangeSet,
}

#[derive(Debug, Clone, PartialEq)]
/// Declarative output returned by a plugin for host validation and routing.
pub enum PluginOutput {
    /// Exact range-backed observations to add to the findings surface.
    Findings(Vec<RegionFinding>),
    /// A typed analysis artifact produced under the plugin contract.
    Artifact(ArtifactPayload),
    /// A renderer-neutral scene fragment for host-side compilation.
    Scene(SceneFragment),
    /// A plugin-produced file retained behind a temporary host handle.
    ExportedFile {
        /// MIME media type of the exported bytes.
        media_type: String,
        /// Opaque host handle used to retrieve or save the file.
        temporary_handle: u64,
    },
    /// A command response encoded by the command's versioned JSON schema.
    CommandResult {
        /// Canonical JSON response payload.
        json: String,
    },
}
