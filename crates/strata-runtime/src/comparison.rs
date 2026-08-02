//! Matched bounded tile comparison for sources larger than contiguous budgets.

use sha2::{Digest, Sha256};
use strata_analysis::tiles::{TilePlanConfig, TilePrecision, plan_source_tiles};
use strata_core::{ByteRange, DomainError, Priority, SourceGeneration, SourceId};

use crate::AttachedSource;

/// Semantic identity of the matched tiled diff artifact.
pub const TILED_DIFF_SEMANTICS: &str = "strata.tiled-diff/v1";

/// Bounded matched-diff parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TiledDiffConfig {
    /// Shared coverage, tile-count, and resident-byte policy.
    pub tile_plan: TilePlanConfig,
}

/// One pair of exact resident reads over the same logical coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiledDiffTile {
    /// Logical aligned-address coverage represented by the tile.
    pub coverage: ByteRange,
    /// Exact range read from both sources.
    pub read_range: ByteRange,
    /// Whether the read is complete coverage or a disclosed overview sample.
    pub precision: TilePrecision,
    /// Exact source-A bytes from `read_range`.
    pub left_bytes: Vec<u8>,
    /// Exact source-B bytes from `read_range`.
    pub right_bytes: Vec<u8>,
    /// One byte per compared position: zero when equal, one when different.
    pub change_mask: Vec<u8>,
}

impl TiledDiffTile {
    /// Returns the exact number of changed resident sample bytes.
    #[must_use]
    pub fn changed_bytes(&self) -> usize {
        self.change_mask
            .iter()
            .map(|changed| usize::from(*changed != 0))
            .sum()
    }
}

/// Immutable bounded comparison with explicit sampled-versus-exact semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiledDiffArtifact {
    /// Source-A identity and generation.
    pub left: (SourceId, SourceGeneration),
    /// Source-B identity and generation.
    pub right: (SourceId, SourceGeneration),
    /// Complete logical length of source A.
    pub left_length: u64,
    /// Complete logical length of source B.
    pub right_length: u64,
    /// Address length for which paired comparison is possible.
    pub aligned_length: u64,
    /// Coarse overview level selected by the shared planner.
    pub overview_level: u8,
    /// Exact paired resident reads in deterministic address order.
    pub tiles: Vec<TiledDiffTile>,
    /// Exact compared sample byte count.
    pub compared_sample_bytes: u64,
    /// Exact changed count within resident sample bytes.
    pub changed_sample_bytes: u64,
    /// Canonical digest of artifact semantics, parameters, provenance, and bytes.
    pub artifact_digest: String,
}

impl TiledDiffArtifact {
    /// Returns whether any tile samples a larger logical range.
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        self.tiles
            .iter()
            .any(|tile| tile.precision == TilePrecision::OverviewSample)
    }

    /// Returns total retained left, right, and mask payload bytes.
    #[must_use]
    pub fn resident_bytes(&self) -> usize {
        self.tiles.iter().fold(0_usize, |total, tile| {
            total
                .saturating_add(tile.left_bytes.len())
                .saturating_add(tile.right_bytes.len())
                .saturating_add(tile.change_mask.len())
        })
    }
}

/// Reads the same bounded tile ranges from two immutable sources and compares them.
///
/// Unaligned tails are disclosed by `left_length` and `right_length`; they are
/// not silently counted as changed paired bytes.
///
/// # Errors
///
/// Returns [`DomainError`] for unknown lengths, invalid limits, stale sources,
/// bounded read failures, or checked-arithmetic overflow.
pub fn build_tiled_diff(
    left: &AttachedSource,
    right: &AttachedSource,
    config: TiledDiffConfig,
    parameter_bytes: &[u8],
) -> Result<TiledDiffArtifact, DomainError> {
    let left_descriptor = left.descriptor();
    let right_descriptor = right.descriptor();
    let left_length = left_descriptor.length.ok_or_else(|| {
        DomainError::UnsupportedCapability("source A length is unknown".to_owned())
    })?;
    let right_length = right_descriptor.length.ok_or_else(|| {
        DomainError::UnsupportedCapability("source B length is unknown".to_owned())
    })?;
    let aligned_length = left_length.min(right_length);
    let plan = plan_source_tiles(
        left_descriptor.id,
        left_descriptor.generation,
        aligned_length,
        config.tile_plan,
        parameter_bytes,
    )?;
    let mut tiles = Vec::with_capacity(plan.tiles.len());
    let mut compared_sample_bytes = 0_u64;
    let mut changed_sample_bytes = 0_u64;
    for planned in plan.tiles {
        let left_bytes = left.read_exact(
            planned.read_range,
            Priority::Visible,
            config.tile_plan.tile_bytes,
        )?;
        let right_bytes = right.read_exact(
            planned.read_range,
            Priority::Visible,
            config.tile_plan.tile_bytes,
        )?;
        if left_bytes.len() != right_bytes.len() {
            return Err(DomainError::SourceMismatch);
        }
        let change_mask = left_bytes
            .iter()
            .zip(&right_bytes)
            .map(|(left, right)| u8::from(left != right))
            .collect::<Vec<_>>();
        let changed = change_mask
            .iter()
            .map(|value| u64::from(*value))
            .sum::<u64>();
        let compared = u64::try_from(change_mask.len()).map_err(|_| DomainError::RangeOverflow)?;
        changed_sample_bytes = changed_sample_bytes
            .checked_add(changed)
            .ok_or(DomainError::RangeOverflow)?;
        compared_sample_bytes = compared_sample_bytes
            .checked_add(compared)
            .ok_or(DomainError::RangeOverflow)?;
        tiles.push(TiledDiffTile {
            coverage: planned.coverage,
            read_range: planned.read_range,
            precision: planned.key.precision,
            left_bytes,
            right_bytes,
            change_mask,
        });
    }
    let artifact_digest = artifact_digest(
        &left_descriptor,
        &right_descriptor,
        left_length,
        right_length,
        plan.overview_level,
        &tiles,
        parameter_bytes,
    );
    Ok(TiledDiffArtifact {
        left: (left_descriptor.id, left_descriptor.generation),
        right: (right_descriptor.id, right_descriptor.generation),
        left_length,
        right_length,
        aligned_length,
        overview_level: plan.overview_level,
        tiles,
        compared_sample_bytes,
        changed_sample_bytes,
        artifact_digest,
    })
}

fn artifact_digest(
    left: &strata_source::SourceDescriptor,
    right: &strata_source::SourceDescriptor,
    left_length: u64,
    right_length: u64,
    overview_level: u8,
    tiles: &[TiledDiffTile],
    parameter_bytes: &[u8],
) -> String {
    let mut digest = Sha256::new();
    digest.update(TILED_DIFF_SEMANTICS.as_bytes());
    digest.update([0]);
    digest.update(left.id.0.to_le_bytes());
    digest.update(left.generation.0.to_le_bytes());
    digest.update(right.id.0.to_le_bytes());
    digest.update(right.generation.0.to_le_bytes());
    digest.update(left_length.to_le_bytes());
    digest.update(right_length.to_le_bytes());
    digest.update([overview_level]);
    digest.update(parameter_bytes);
    for tile in tiles {
        digest.update(tile.coverage.start.to_le_bytes());
        digest.update(tile.coverage.end.to_le_bytes());
        digest.update(tile.read_range.start.to_le_bytes());
        digest.update(tile.read_range.end.to_le_bytes());
        digest.update([match tile.precision {
            TilePrecision::OverviewSample => 0,
            TilePrecision::Exact => 1,
        }]);
        digest.update(&tile.left_bytes);
        digest.update(&tile.right_bytes);
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn matched_tiles_retain_exact_offsets_and_changed_counts() -> Result<(), DomainError> {
        let left_bytes = vec![0x11; 2_048];
        let mut right_bytes = left_bytes.clone();
        right_bytes[17] = 0x22;
        right_bytes[1_777] = 0x33;
        let left = AttachedSource::retained(
            SourceId(1),
            SourceGeneration(4),
            "left",
            Arc::<[u8]>::from(left_bytes),
        )?;
        let right = AttachedSource::retained(
            SourceId(2),
            SourceGeneration(4),
            "right",
            Arc::<[u8]>::from(right_bytes),
        )?;
        let artifact = build_tiled_diff(
            &left,
            &right,
            TiledDiffConfig {
                tile_plan: TilePlanConfig {
                    tile_bytes: 256,
                    maximum_tiles: 8,
                    maximum_resident_bytes: 2_048,
                    focus: None,
                    focus_radius_tiles: 1,
                },
            },
            b"matched-test",
        )?;
        assert_eq!(artifact.compared_sample_bytes, 2_048);
        assert_eq!(artifact.changed_sample_bytes, 2);
        assert!(!artifact.is_sampled());
        assert_eq!(artifact.resident_bytes(), 6_144);
        assert_eq!(artifact.artifact_digest.len(), 64);
        Ok(())
    }

    #[test]
    fn sampled_tiles_never_claim_unsampled_changes() -> Result<(), DomainError> {
        let left = AttachedSource::retained(
            SourceId(1),
            SourceGeneration(0),
            "left",
            Arc::<[u8]>::from(vec![0; 4_096]),
        )?;
        let mut right_bytes = vec![0; 4_096];
        right_bytes[0] = 1;
        let right = AttachedSource::retained(
            SourceId(2),
            SourceGeneration(0),
            "right",
            Arc::<[u8]>::from(right_bytes),
        )?;
        let artifact = build_tiled_diff(
            &left,
            &right,
            TiledDiffConfig {
                tile_plan: TilePlanConfig {
                    tile_bytes: 128,
                    maximum_tiles: 4,
                    maximum_resident_bytes: 512,
                    focus: None,
                    focus_radius_tiles: 1,
                },
            },
            b"sampled-test",
        )?;
        assert!(artifact.is_sampled());
        assert_eq!(artifact.compared_sample_bytes, 512);
        assert!(artifact.compared_sample_bytes < artifact.aligned_length);
        Ok(())
    }
}
