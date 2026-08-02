//! Deterministic, source-addressable 3D projections for the interactive POC.
//!
//! The facade keeps composition and sampling imports stable while separating
//! contracts, coordinate mappings, bounded feature extraction, and colour.
#![allow(clippy::redundant_pub_crate)] // Parent-only helpers live in binary modules.

mod color;
mod coordinates;
mod model;
mod sampling;

#[cfg(test)]
mod tests;

pub(crate) use model::{
    ProjectionChannels, ProjectionColorFeature, ProjectionCompareMode, ProjectionComposition,
    ProjectionDimensions, ProjectionDomain, ProjectionGeometry, ProjectionHeightFeature,
    ProjectionKind, ProjectionOpacityFeature, ProjectionOverlays, ProjectionParameters,
    ProjectionRegionPlacement, ProjectionSample, ProjectionSamplingConfig, ProjectionSizeFeature,
};
pub(crate) use sampling::{
    sample_projection_sample_at_source_offset, sample_projection_samples_at_offset,
    sample_projection_samples_in_source, sample_projection_samples_with_config,
};

#[cfg(test)]
use coordinates::morph_position;
#[cfg(test)]
use sampling::local_entropy;
#[cfg(test)]
pub(crate) use sampling::sample_projection_samples;
