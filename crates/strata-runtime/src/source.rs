//! Immutable source handles and bounded overview materialization.

use std::{path::Path, sync::Arc};

use strata_analysis::tiles::{TilePlan, TilePlanConfig, TilePrecision, plan_source_tiles};
use strata_core::{ByteRange, ByteRangeSet, DomainError, Priority, SourceGeneration, SourceId};
use strata_source::{
    ByteSource, HashProgress, LocalFileIdentity, LocalFileSource, ReadRequest, RetainedByteSource,
    SourceDescriptor,
};

/// Default exact prefix retained for contiguous legacy views.
pub const DEFAULT_PREVIEW_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone)]
enum AttachedSourceKind {
    Local(Arc<LocalFileSource>),
    Retained(Arc<RetainedByteSource>),
}

/// Immutable source-generation handle shared by GUI, CLI, and background work.
///
/// Cloning this value clones only an [`Arc`]. Source bytes remain owned by the
/// read-only source implementation and are accessed through bounded requests.
#[derive(Debug, Clone)]
pub struct AttachedSource {
    kind: AttachedSourceKind,
}

impl AttachedSource {
    /// Opens a regular local file read-only and captures its stable identity.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the path is unreadable, cannot be inspected,
    /// or does not refer to a regular file.
    pub fn open_local(
        path: impl AsRef<Path>,
        source_id: SourceId,
        generation: SourceGeneration,
    ) -> std::io::Result<Self> {
        LocalFileSource::open(path, source_id, generation).map(|source| Self {
            kind: AttachedSourceKind::Local(Arc::new(source)),
        })
    }

    /// Retains an immutable in-memory source and seals its digest immediately.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::RangeOverflow`] when the byte length cannot be
    /// represented by the source contract.
    pub fn retained(
        source_id: SourceId,
        generation: SourceGeneration,
        display_name: impl Into<String>,
        bytes: Arc<[u8]>,
    ) -> Result<Self, DomainError> {
        RetainedByteSource::new(source_id, generation, display_name, bytes).map(|source| Self {
            kind: AttachedSourceKind::Retained(Arc::new(source)),
        })
    }

    /// Returns a point-in-time source descriptor.
    #[must_use]
    pub fn descriptor(&self) -> SourceDescriptor {
        match &self.kind {
            AttachedSourceKind::Local(source) => source.descriptor_snapshot(),
            AttachedSourceKind::Retained(source) => source.descriptor(),
        }
    }

    /// Returns the captured kernel identity for a local source.
    #[must_use]
    pub fn local_identity(&self) -> Option<LocalFileIdentity> {
        match &self.kind {
            AttachedSourceKind::Local(source) => Some(source.identity()),
            AttachedSourceKind::Retained(_) => None,
        }
    }

    /// Returns this source behind the object-safe bounded-read contract.
    #[must_use]
    pub fn byte_source(&self) -> Arc<dyn ByteSource> {
        match &self.kind {
            AttachedSourceKind::Local(source) => source.clone(),
            AttachedSourceKind::Retained(source) => source.clone(),
        }
    }

    /// Reads one exact half-open range with an explicit byte ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when identity or generation changed, the range
    /// is invalid, the ceiling is too small, or the source read fails.
    pub fn read_exact(
        &self,
        range: ByteRange,
        priority: Priority,
        maximum_bytes: u64,
    ) -> Result<Vec<u8>, DomainError> {
        if range.is_empty() {
            return Ok(Vec::new());
        }
        let descriptor = self.descriptor();
        let request = ReadRequest {
            source_id: descriptor.id,
            generation: descriptor.generation,
            ranges: ByteRangeSet {
                ranges: vec![range],
            },
            priority,
            maximum_bytes,
        };
        match &self.kind {
            AttachedSourceKind::Local(source) => source.read_bounded(&request),
            AttachedSourceKind::Retained(source) => source.read_bounded(&request),
        }
        .map(|chunk| chunk.bytes)
    }

    /// Advances a local source's canonical digest by a bounded number of bytes.
    ///
    /// Retained sources are already sealed and return their completed state
    /// without additional work.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the budget is zero, source identity changed,
    /// checked arithmetic fails, or a local read fails.
    pub fn advance_digest(&self, maximum_bytes: u64) -> Result<HashProgress, DomainError> {
        match &self.kind {
            AttachedSourceKind::Local(source) => source.advance_hash(maximum_bytes),
            AttachedSourceKind::Retained(source) => {
                if maximum_bytes == 0 {
                    return Err(DomainError::ResourceLimit(
                        "hash budget must be greater than zero".to_owned(),
                    ));
                }
                let descriptor = source.descriptor();
                Ok(HashProgress {
                    state: descriptor.digest_state,
                    bytes_hashed: descriptor.length.unwrap_or(0),
                    total_bytes: descriptor.length.unwrap_or(0),
                    content_digest: descriptor.content_digest,
                })
            }
        }
    }
}

/// Bounded source-overview parameters independent of any frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceOverviewConfig {
    /// Maximum exact prefix retained for contiguous views.
    pub preview_bytes: u64,
    /// Tile count, byte budget, and optional exact focus policy.
    pub tile_plan: TilePlanConfig,
}

impl Default for SourceOverviewConfig {
    fn default() -> Self {
        Self {
            preview_bytes: DEFAULT_PREVIEW_BYTES,
            tile_plan: TilePlanConfig::default(),
        }
    }
}

/// One resident tile with both logical coverage and exact read provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentSourceTile {
    /// Full logical source coverage represented by this tile.
    pub coverage: ByteRange,
    /// Exact source range retained in `bytes`.
    pub read_range: ByteRange,
    /// Pyramid level chosen by the deterministic planner.
    pub level: u8,
    /// Whether `bytes` are exact for all of `coverage` or a disclosed sample.
    pub precision: TilePrecision,
    /// Digest of planner parameters used by downstream cache keys.
    pub parameter_digest: String,
    /// Exact bytes read from `read_range`.
    pub bytes: Vec<u8>,
}

/// Bounded preview and tile payloads for one immutable source generation.
#[derive(Debug, Clone)]
pub struct SourceOverview {
    /// Source from which every resident byte was read.
    pub source: AttachedSource,
    /// Exact prefix range retained in `preview_bytes`.
    pub preview_range: ByteRange,
    /// Exact prefix for legacy contiguous consumers.
    pub preview_bytes: Vec<u8>,
    /// Deterministic tile plan, including resident-byte accounting.
    pub plan: TilePlan,
    /// Tile payloads in the same order as `plan.tiles`.
    pub resident_tiles: Vec<ResidentSourceTile>,
}

impl SourceOverview {
    /// Returns whether any resident tile samples a larger logical range.
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        self.plan.is_sampled()
    }
}

/// Materializes a bounded overview without reading the whole source.
///
/// # Errors
///
/// Returns [`DomainError`] when the source has no known length, configuration
/// is invalid, planning overflows, identity changes, or any bounded read fails.
pub fn build_source_overview(
    source: AttachedSource,
    config: SourceOverviewConfig,
    parameter_bytes: &[u8],
) -> Result<SourceOverview, DomainError> {
    if config.preview_bytes == 0 {
        return Err(DomainError::ResourceLimit(
            "source preview budget must be greater than zero".to_owned(),
        ));
    }
    let descriptor = source.descriptor();
    let source_length = descriptor
        .length
        .ok_or_else(|| DomainError::UnsupportedCapability("source length is unknown".to_owned()))?;
    let preview_range = ByteRange::new(0, source_length.min(config.preview_bytes))?;
    let preview_bytes =
        source.read_exact(preview_range, Priority::Interactive, config.preview_bytes)?;
    let plan = plan_source_tiles(
        descriptor.id,
        descriptor.generation,
        source_length,
        config.tile_plan,
        parameter_bytes,
    )?;
    let mut resident_tiles = Vec::with_capacity(plan.tiles.len());
    for tile in &plan.tiles {
        let bytes = source.read_exact(
            tile.read_range,
            Priority::Interactive,
            config.tile_plan.tile_bytes,
        )?;
        resident_tiles.push(ResidentSourceTile {
            coverage: tile.coverage,
            read_range: tile.read_range,
            level: tile.key.level,
            precision: tile.key.precision,
            parameter_digest: tile.key.parameter_digest.clone(),
            bytes,
        });
    }
    Ok(SourceOverview {
        source,
        preview_range,
        preview_bytes,
        plan,
        resident_tiles,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs::OpenOptions,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use strata_source::DigestState;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn retained_overview_is_exact_and_digest_sealed() -> Result<(), DomainError> {
        let bytes = Arc::<[u8]>::from((0_u8..=255).cycle().take(1024).collect::<Vec<_>>());
        let source =
            AttachedSource::retained(SourceId(4), SourceGeneration(2), "runtime fixture", bytes)?;
        let overview =
            build_source_overview(source.clone(), SourceOverviewConfig::default(), b"test")?;
        assert_eq!(overview.preview_bytes.len(), 1024);
        assert!(!overview.is_sampled());
        assert_eq!(source.advance_digest(1)?.state, DigestState::Sealed);
        Ok(())
    }

    #[test]
    fn overview_respects_preview_and_resident_budgets() -> Result<(), DomainError> {
        let bytes = Arc::<[u8]>::from(vec![0x5a; 1024]);
        let source =
            AttachedSource::retained(SourceId(5), SourceGeneration(0), "bounded fixture", bytes)?;
        let config = SourceOverviewConfig {
            preview_bytes: 128,
            tile_plan: TilePlanConfig {
                tile_bytes: 128,
                maximum_tiles: 8,
                maximum_resident_bytes: 1024,
                focus: None,
                focus_radius_tiles: 1,
            },
        };
        let overview = build_source_overview(source, config, b"bounded")?;
        assert_eq!(overview.preview_bytes.len(), 128);
        assert_eq!(overview.plan.resident_bytes, 1024);
        assert_eq!(overview.resident_tiles.len(), 8);
        Ok(())
    }

    #[test]
    fn sparse_hundred_gib_source_keeps_a_tiny_resident_working_set() -> Result<(), Box<dyn Error>> {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "strata-runtime-sparse-{}-{counter}.bin",
            std::process::id()
        ));
        let result = (|| -> Result<(), Box<dyn Error>> {
            let file = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&path)?;
            let logical_length = 100_u64 * 1024 * 1024 * 1024;
            file.set_len(logical_length)?;
            drop(file);
            let source = AttachedSource::open_local(&path, SourceId(77), SourceGeneration(5))?;
            let overview = build_source_overview(
                source,
                SourceOverviewConfig {
                    preview_bytes: 4 * 1024,
                    tile_plan: TilePlanConfig {
                        tile_bytes: 4 * 1024,
                        maximum_tiles: 8,
                        maximum_resident_bytes: 32 * 1024,
                        focus: None,
                        focus_radius_tiles: 1,
                    },
                },
                b"sparse-100-gib",
            )?;
            assert_eq!(overview.plan.source_length, logical_length);
            assert_eq!(overview.preview_bytes.len(), 4 * 1024);
            assert!(overview.plan.resident_bytes <= 32 * 1024);
            assert!(overview.plan.resident_bytes > 0);
            assert!(overview.resident_tiles.len() <= 8);
            assert!(overview.is_sampled());
            Ok(())
        })();
        let _ = std::fs::remove_file(path);
        result
    }
}
