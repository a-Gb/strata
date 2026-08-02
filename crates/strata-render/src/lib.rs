//! Declarative render scene and picking contracts.
#![forbid(unsafe_code)]

use strata_core::{ArtifactId, ByteRangeSet, DomainError, ProvenanceToken, ViewId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Stable identifier used to resolve a rendered primitive back to source coverage.
pub struct PickId(pub u64);

#[derive(Debug, Clone, PartialEq)]
/// Backend-independent primitive published into a render scene.
pub enum ScenePrimitive {
    /// Continuous scalar values rendered over a rectangular domain.
    ScalarTile(ScalarTile),
    /// Discrete categories rendered over a rectangular domain.
    CategoricalTile(CategoricalTile),
    /// Weighted points in two- or three-dimensional coordinates.
    Points(PointCloud),
    /// Indexed line segments.
    Lines(LineSet),
    /// Axis-aligned rectangular regions.
    Rectangles(RectangleSet),
    /// Indexed triangle geometry.
    Mesh(Mesh),
    /// Text labels anchored in scene coordinates.
    Text(TextLabels),
    /// Addressable slices of a volume artifact.
    VolumeSlices(VolumeSlices),
}

#[derive(Debug, Clone, PartialEq)]
/// Placement and normalization of one continuous scalar artifact.
pub struct ScalarTile {
    /// Artifact containing scalar values.
    pub artifact: ArtifactId,
    /// Lower-left tile origin in scene coordinates.
    pub origin: [f32; 2],
    /// Tile width and height in scene coordinates.
    pub extent: [f32; 2],
    /// Canonical JSON describing value normalization.
    pub normalization_json: String,
}

#[derive(Debug, Clone, PartialEq)]
/// Placement and palette of one categorical artifact.
pub struct CategoricalTile {
    /// Artifact containing categorical values.
    pub artifact: ArtifactId,
    /// Lower-left tile origin in scene coordinates.
    pub origin: [f32; 2],
    /// Tile width and height in scene coordinates.
    pub extent: [f32; 2],
    /// Stable palette identifier.
    pub palette_id: String,
}

#[derive(Debug, Clone, PartialEq)]
/// Point positions, visual weights, and source-picking identities.
pub struct PointCloud {
    /// Point coordinates.
    pub positions: Vec<[f32; 3]>,
    /// Per-point visual weights in matching order.
    pub weights: Vec<f32>,
    /// Per-point picking identities in matching order.
    pub pick_ids: Vec<PickId>,
}

#[derive(Debug, Clone, PartialEq)]
/// Indexed line geometry with source-picking identities.
pub struct LineSet {
    /// Vertex coordinates.
    pub vertices: Vec<[f32; 3]>,
    /// Pairs of vertex indices defining segments.
    pub segments: Vec<[u32; 2]>,
    /// Per-segment picking identities in matching order.
    pub pick_ids: Vec<PickId>,
}

#[derive(Debug, Clone, PartialEq)]
/// Axis-aligned rectangles with source-picking identities.
pub struct RectangleSet {
    /// Rectangles encoded as `[minimum_x, minimum_y, maximum_x, maximum_y]`.
    pub rectangles: Vec<[f32; 4]>,
    /// Per-rectangle picking identities in matching order.
    pub pick_ids: Vec<PickId>,
}

#[derive(Debug, Clone, PartialEq)]
/// Indexed triangle mesh with optional aggregate picking identity.
pub struct Mesh {
    /// Vertex coordinates.
    pub positions: Vec<[f32; 3]>,
    /// Triangle vertex indices.
    pub indices: Vec<u32>,
    /// Picking identity shared by the mesh, when source coverage is available.
    pub pick_id: Option<PickId>,
}

#[derive(Debug, Clone, PartialEq)]
/// Text strings and their matching scene anchors.
pub struct TextLabels {
    /// User-facing label strings.
    pub labels: Vec<String>,
    /// Per-label scene anchors in matching order.
    pub anchors: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Addressable slice interval within a volume artifact.
pub struct VolumeSlices {
    /// Artifact containing the volume.
    pub artifact: ArtifactId,
    /// Axis normal to the requested slices.
    pub slice_axis: u8,
    /// Half-open slice index range.
    pub slice_range: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Precision of the source coverage resolved from a pick.
pub enum PickCoverage {
    /// Complete exact source contributors.
    Exact(ByteRangeSet),
    /// Exact contributors retained by a disclosed sampling policy.
    Sampled(ByteRangeSet),
    /// Aggregate meaning without per-value source contributors.
    Aggregate {
        /// User-facing explanation of aggregate coverage.
        description: String,
    },
    /// Estimated source coverage.
    Approximate(ByteRangeSet),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Mapping from one pick identity to source coverage and provenance.
pub struct PickMapping {
    /// Picking identity emitted by a scene primitive.
    pub id: PickId,
    /// Source coverage represented by the pick.
    pub coverage: PickCoverage,
    /// Root derivation token for the rendered value.
    pub provenance: ProvenanceToken,
    /// Canonical JSON attributes displayed by inspectors.
    pub attributes_json: String,
}

#[derive(Debug, Clone, PartialEq)]
/// Complete declarative scene contribution published by one view.
pub struct SceneFragment {
    /// View that owns this fragment.
    pub view_id: ViewId,
    /// Backend-independent render primitives.
    pub primitives: Vec<ScenePrimitive>,
    /// Picking mappings referenced by the primitives.
    pub picking: Vec<PickMapping>,
    /// Canonical JSON legend specification.
    pub legend_json: String,
    /// User-facing accuracy, sampling, or fallback warnings.
    pub warnings: Vec<String>,
}

/// Coordinator boundary between declarative views and the active renderer.
pub trait RenderCoordinator: Send + Sync {
    /// Publishes the complete latest scene fragment for one view.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when fragment validation or renderer publication fails.
    fn publish(&self, fragment: SceneFragment) -> Result<(), DomainError>;
    /// Removes all published scene state owned by a view.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when renderer state cannot be updated safely.
    fn remove_view(&self, view_id: ViewId) -> Result<(), DomainError>;
    /// Requests presentation of the latest published scene state.
    fn request_frame(&self);
}
