//! Serializable view descriptions.

use crate::{ArtifactId, SourceId, TransformNodeId, ViewId};

#[derive(Debug, Clone, PartialEq)]
/// Serializable two-dimensional camera state.
pub struct Camera2d {
    /// Horizontal coordinate at the viewport center.
    pub center_x: f64,
    /// Vertical coordinate at the viewport center.
    pub center_y: f64,
    /// Positive view scale applied around the center.
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Serializable three-dimensional look-at camera state.
pub struct Camera3d {
    /// Camera position in view coordinates.
    pub eye: [f32; 3],
    /// Point toward which the camera looks.
    pub target: [f32; 3],
    /// Camera up direction.
    pub up: [f32; 3],
    /// Vertical field of view in degrees.
    pub field_of_view_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
/// Camera representation used by a view.
pub enum CameraSpec {
    /// Explicit two-dimensional camera.
    TwoDimensional(Camera2d),
    /// Explicit three-dimensional camera.
    ThreeDimensional(Camera3d),
    /// View implementation chooses its deterministic domain default.
    DomainDefault,
}

#[derive(Debug, Clone, PartialEq)]
/// Serializable declaration of one source-bound view instance.
pub struct ViewSpec {
    /// Stable identity of the view instance.
    pub id: ViewId,
    /// View implementation kind.
    pub kind: String,
    /// Version of the view contract.
    pub version: String,
    /// Immutable source displayed by the view.
    pub source_id: SourceId,
    /// Optional transform node whose output feeds the view.
    pub transform_binding: Option<TransformNodeId>,
    /// Derived artifacts consumed by the view.
    pub artifacts: Vec<ArtifactId>,
    /// Serializable camera state.
    pub camera: CameraSpec,
    /// Canonical JSON parameters controlling the view.
    pub parameter_json: String,
    /// Optional group used to synchronize compatible view interactions.
    pub linked_group: Option<String>,
    /// Named quality and resource policy selected for the view.
    pub quality_policy: String,
}
