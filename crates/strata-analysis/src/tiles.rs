//! Deterministic bounded tile and level-of-detail planning for large sources.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strata_core::{ByteRange, DomainError, SourceGeneration, SourceId};

/// Semantic identity for the first tiled source planner.
pub const TILE_PLANNER_SEMANTICS: &str = "strata.tile-plan/v1";

/// Precision attached to one planned source tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TilePrecision {
    /// A bounded systematic sample represents a larger coverage range.
    OverviewSample,
    /// The complete level-zero tile is requested.
    Exact,
}

/// Bounded planner inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePlanConfig {
    /// Exact level-zero tile size.
    pub tile_bytes: u64,
    /// Maximum number of resident tile payloads.
    pub maximum_tiles: usize,
    /// Maximum total bytes read for the plan.
    pub maximum_resident_bytes: u64,
    /// Optional exact focus range.
    pub focus: Option<ByteRange>,
    /// Level-zero tiles retained on each side of the focus.
    pub focus_radius_tiles: u32,
}

impl Default for TilePlanConfig {
    fn default() -> Self {
        Self {
            tile_bytes: 256 * 1024,
            maximum_tiles: 64,
            maximum_resident_bytes: 16 * 1024 * 1024,
            focus: None,
            focus_radius_tiles: 1,
        }
    }
}

impl TilePlanConfig {
    fn validate(self) -> Result<(), DomainError> {
        if self.tile_bytes == 0
            || self.maximum_tiles == 0
            || self.maximum_resident_bytes < self.tile_bytes
        {
            return Err(DomainError::ResourceLimit(
                "tile size, count, and resident budget must retain at least one tile".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Cache identity for a planned tile payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TileKey {
    /// Immutable source identity.
    pub source_id: SourceId,
    /// Source generation used for the read.
    pub generation: SourceGeneration,
    /// Planner semantics version.
    pub semantics: &'static str,
    /// Pyramid level of the represented coverage.
    pub level: u8,
    /// Tile coordinate at that level.
    pub tile_index: u64,
    /// Exact or sampled payload semantics.
    pub precision: TilePrecision,
    /// Digest of analysis parameters layered above the source planner.
    pub parameter_digest: String,
}

/// One bounded payload read and the source coverage it represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedTile {
    /// Content-addressable tile identity.
    pub key: TileKey,
    /// Full logical coverage represented by this tile.
    pub coverage: ByteRange,
    /// Exact source range read into resident memory.
    pub read_range: ByteRange,
}

/// Complete bounded plan for one source generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilePlan {
    /// Captured source length.
    pub source_length: u64,
    /// Coarse overview level selected to fit the tile budget.
    pub overview_level: u8,
    /// Deterministically ordered reads, with exact focus tiles last.
    pub tiles: Vec<PlannedTile>,
    /// Sum of requested resident payload bytes.
    pub resident_bytes: u64,
}

impl TilePlan {
    /// Whether at least one tile represents sampled rather than exact coverage.
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        self.tiles
            .iter()
            .any(|tile| tile.key.precision == TilePrecision::OverviewSample)
    }
}

/// Plans systematic overview tiles plus exact focus tiles within hard limits.
///
/// # Errors
///
/// Returns an error for invalid limits or focus ranges, checked-arithmetic overflow, or a plan
/// that cannot fit the declared tile and resident-byte ceilings.
#[allow(clippy::too_many_lines)] // Keeping overview and focus accounting in one audited gate.
pub fn plan_source_tiles(
    source_id: SourceId,
    generation: SourceGeneration,
    source_length: u64,
    config: TilePlanConfig,
    parameter_bytes: &[u8],
) -> Result<TilePlan, DomainError> {
    config.validate()?;
    if source_length == 0 {
        return Ok(TilePlan {
            source_length,
            overview_level: 0,
            tiles: Vec::new(),
            resident_bytes: 0,
        });
    }
    if config
        .focus
        .is_some_and(|focus| focus.start > focus.end || focus.end > source_length)
    {
        return Err(DomainError::InvalidRange {
            start: config.focus.map_or(0, |focus| focus.start),
            end: config.focus.map_or(source_length, |focus| focus.end),
        });
    }

    let maximum_by_bytes = config.maximum_resident_bytes / config.tile_bytes;
    let maximum_tiles = config
        .maximum_tiles
        .min(usize::try_from(maximum_by_bytes).unwrap_or(usize::MAX))
        .max(1);
    let focus_indexes = exact_focus_indexes(source_length, config)?;
    let overview_budget = maximum_tiles.saturating_sub(focus_indexes.len()).max(1);
    let (overview_level, coverage_bytes) = overview_level(
        source_length,
        config.tile_bytes,
        u64::try_from(overview_budget).unwrap_or(u64::MAX),
    );
    let parameter_digest = format!("{:x}", Sha256::digest(parameter_bytes));
    let overview_count = source_length.div_ceil(coverage_bytes);
    let mut tiles = Vec::with_capacity(
        usize::try_from(overview_count)
            .unwrap_or(overview_budget)
            .saturating_add(focus_indexes.len()),
    );

    for tile_index in 0..overview_count {
        let coverage_start = tile_index
            .checked_mul(coverage_bytes)
            .ok_or(DomainError::RangeOverflow)?;
        let coverage_end = coverage_start
            .saturating_add(coverage_bytes)
            .min(source_length);
        let coverage = ByteRange::new(coverage_start, coverage_end)?;
        let payload_length = coverage.len().min(config.tile_bytes);
        let centered =
            coverage_start.saturating_add(coverage.len().saturating_sub(payload_length) / 2);
        let read_range = ByteRange::new(centered, centered.saturating_add(payload_length))?;
        let precision = if coverage == read_range {
            TilePrecision::Exact
        } else {
            TilePrecision::OverviewSample
        };
        tiles.push(PlannedTile {
            key: TileKey {
                source_id,
                generation,
                semantics: TILE_PLANNER_SEMANTICS,
                level: overview_level,
                tile_index,
                precision,
                parameter_digest: parameter_digest.clone(),
            },
            coverage,
            read_range,
        });
    }

    for tile_index in focus_indexes {
        let start = tile_index
            .checked_mul(config.tile_bytes)
            .ok_or(DomainError::RangeOverflow)?;
        let range = ByteRange::new(
            start,
            start.saturating_add(config.tile_bytes).min(source_length),
        )?;
        if tiles.iter().any(|tile| tile.read_range == range) {
            continue;
        }
        tiles.push(PlannedTile {
            key: TileKey {
                source_id,
                generation,
                semantics: TILE_PLANNER_SEMANTICS,
                level: 0,
                tile_index,
                precision: TilePrecision::Exact,
                parameter_digest: parameter_digest.clone(),
            },
            coverage: range,
            read_range: range,
        });
    }

    let resident_bytes = tiles.iter().try_fold(0_u64, |total, tile| {
        total
            .checked_add(tile.read_range.len())
            .ok_or(DomainError::RangeOverflow)
    })?;
    if tiles.len() > config.maximum_tiles || resident_bytes > config.maximum_resident_bytes {
        return Err(DomainError::ResourceLimit(
            "tile plan exceeded its declared resident limits".to_owned(),
        ));
    }
    Ok(TilePlan {
        source_length,
        overview_level,
        tiles,
        resident_bytes,
    })
}

fn exact_focus_indexes(
    source_length: u64,
    config: TilePlanConfig,
) -> Result<BTreeSet<u64>, DomainError> {
    let Some(focus) = config.focus.filter(|focus| !focus.is_empty()) else {
        return Ok(BTreeSet::new());
    };
    let last_source_tile = source_length.saturating_sub(1) / config.tile_bytes;
    let first = focus.start / config.tile_bytes;
    let last = focus.end.saturating_sub(1) / config.tile_bytes;
    let radius = u64::from(config.focus_radius_tiles);
    let start = first.saturating_sub(radius);
    let end = last.saturating_add(radius).min(last_source_tile);
    let count = end.saturating_sub(start).saturating_add(1);
    if usize::try_from(count).unwrap_or(usize::MAX) >= config.maximum_tiles {
        return Err(DomainError::ResourceLimit(
            "exact focus consumes the entire tile budget".to_owned(),
        ));
    }
    Ok((start..=end).collect())
}

const fn overview_level(source_length: u64, tile_bytes: u64, maximum_tiles: u64) -> (u8, u64) {
    let mut level = 0_u8;
    let mut coverage = tile_bytes;
    while source_length.div_ceil(coverage) > maximum_tiles && level < 63 {
        let Some(next) = coverage.checked_mul(2) else {
            break;
        };
        coverage = next;
        level = level.saturating_add(1);
    }
    (level, coverage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tib_overview_stays_within_sixteen_mib() -> Result<(), DomainError> {
        let plan = plan_source_tiles(
            SourceId(9),
            SourceGeneration(3),
            1_u64 << 40,
            TilePlanConfig::default(),
            b"p1-defaults",
        )?;
        assert!(plan.overview_level > 0);
        assert!(plan.is_sampled());
        assert!(plan.tiles.len() <= 64);
        assert!(plan.resident_bytes <= 16 * 1024 * 1024);
        assert_eq!(plan.tiles.first().map(|tile| tile.coverage.start), Some(0));
        assert_eq!(
            plan.tiles.last().map(|tile| tile.coverage.end),
            Some(1_u64 << 40)
        );
        Ok(())
    }

    #[test]
    fn focus_tiles_are_exact_and_keys_change_with_parameters() -> Result<(), DomainError> {
        let config = TilePlanConfig {
            focus: Some(ByteRange::new(900_000, 920_000)?),
            ..TilePlanConfig::default()
        };
        let first = plan_source_tiles(
            SourceId(1),
            SourceGeneration(0),
            128 * 1024 * 1024,
            config,
            b"first",
        )?;
        let second = plan_source_tiles(
            SourceId(1),
            SourceGeneration(0),
            128 * 1024 * 1024,
            config,
            b"second",
        )?;
        assert!(first.tiles.iter().any(|tile| {
            tile.key.precision == TilePrecision::Exact && tile.coverage.contains(900_000)
        }));
        assert_ne!(
            first.tiles.first().map(|tile| &tile.key.parameter_digest),
            second.tiles.first().map(|tile| &tile.key.parameter_digest)
        );
        Ok(())
    }

    #[test]
    fn small_sources_are_exact_without_duplicate_focus_tiles() -> Result<(), DomainError> {
        let config = TilePlanConfig {
            focus: Some(ByteRange::new(8, 16)?),
            ..TilePlanConfig::default()
        };
        let plan = plan_source_tiles(SourceId(1), SourceGeneration(0), 1024, config, b"small")?;
        assert_eq!(plan.tiles.len(), 1);
        assert!(!plan.is_sampled());
        assert_eq!(plan.resident_bytes, 1024);
        Ok(())
    }
}
