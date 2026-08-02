//! Bounded CPU reference algorithms for Strata's P1 analytical projections.
#![allow(clippy::cast_precision_loss)] // Normalized renderer coordinates are explicitly f32.

use std::f32::consts::TAU;

use strata_core::{ByteRange, DomainError};

/// Semantic identity shared by CPU and GPU P1 feature implementations.
pub const P1_PROJECTION_SEMANTICS: &str = "strata.projection-p1/v1";

/// Explicit switches prevent expensive feature families from running when hidden.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct P1FeatureRequest {
    /// Compute bounded recurrence and prior-match evidence.
    pub recurrence: bool,
    /// Compute a local discrete Fourier spectrum.
    pub spectrum: bool,
    /// Compute deterministic recursive feature partitions.
    pub hierarchy: bool,
}

/// Bounded parameters for all P1 CPU reference algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct P1AnalysisConfig {
    /// Candidate fixed-record stride rendered by Alignment Lattice.
    pub alignment_stride: usize,
    /// Largest stride included in automatic ranking.
    pub alignment_max_stride: usize,
    /// Exact window compared for recurrence.
    pub recurrence_window: usize,
    /// Prior bytes searched for a related window.
    pub recurrence_search_bytes: usize,
    /// Maximum prior candidates tested per datum.
    pub recurrence_candidate_budget: usize,
    /// Minimum matching-byte percentage retained as a recurrence partner.
    pub recurrence_threshold_percent: u8,
    /// Exact byte window transformed by the local DFT.
    pub spectrum_window: usize,
    /// Maximum non-DC frequency bins tested.
    pub spectrum_bins: usize,
    /// Maximum recursive block depth.
    pub hierarchy_max_depth: u8,
    /// Smallest block eligible for another split.
    pub hierarchy_min_block: usize,
    /// Minimum feature-discontinuity percentage required to split.
    pub hierarchy_threshold_percent: u8,
}

impl Default for P1AnalysisConfig {
    fn default() -> Self {
        Self {
            alignment_stride: 16,
            alignment_max_stride: 128,
            recurrence_window: 16,
            recurrence_search_bytes: 4096,
            recurrence_candidate_budget: 64,
            recurrence_threshold_percent: 75,
            spectrum_window: 64,
            spectrum_bins: 32,
            hierarchy_max_depth: 6,
            hierarchy_min_block: 64,
            hierarchy_threshold_percent: 18,
        }
    }
}

impl P1AnalysisConfig {
    /// Rejects unbounded or nonsensical work before analysis.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidTransform`] when a parameter exceeds its declared bound.
    pub fn validate(self) -> Result<(), DomainError> {
        if !(1..=4096).contains(&self.alignment_stride)
            || !(2..=4096).contains(&self.alignment_max_stride)
            || !(4..=4096).contains(&self.recurrence_window)
            || !(4..=16 * 1024 * 1024).contains(&self.recurrence_search_bytes)
            || !(1..=4096).contains(&self.recurrence_candidate_budget)
            || self.recurrence_threshold_percent > 100
            || !(8..=4096).contains(&self.spectrum_window)
            || !(1..=256).contains(&self.spectrum_bins)
            || self.hierarchy_max_depth > 16
            || !(8..=16 * 1024 * 1024).contains(&self.hierarchy_min_block)
            || self.hierarchy_threshold_percent > 100
        {
            return Err(DomainError::InvalidTransform(
                "P1 projection parameters exceed their bounded domains".to_owned(),
            ));
        }
        Ok(())
    }
}

/// One candidate record width and its normalized periodic-similarity score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignmentCandidate {
    /// Candidate stride in bytes.
    pub stride: usize,
    /// Score in `0..=1`; a hypothesis, not format evidence by itself.
    pub score: f32,
}

/// Per-datum coordinates and evidence shared by the six P1 projections.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P1FeatureRecord {
    /// Stable source-derived identity supplied by the caller.
    pub point_id: u64,
    /// Alignment Lattice coordinates.
    pub alignment: [f32; 3],
    /// Sparse best-partner Recurrence Plane coordinates.
    pub recurrence: [f32; 3],
    /// Repetition Skyline coordinates.
    pub repetition: [f32; 3],
    /// Local Spectral Waterfall coordinates.
    pub spectrum: [f32; 3],
    /// Fixed-basis Hamming Hypercube coordinates.
    pub hypercube: [f32; 3],
    /// Hierarchical Block Volume coordinates.
    pub hierarchy: [f32; 3],
    /// Exact prior source range supporting recurrence/repetition, when retained.
    pub partner_range: Option<ByteRange>,
    /// Best recurrence similarity in `0..=1`.
    pub recurrence_score: f32,
    /// Exact repeated prefix length after the configured recurrence window.
    pub match_length: usize,
    /// Dominant non-DC frequency bin.
    pub dominant_frequency_bin: usize,
    /// Normalized magnitude of that bin.
    pub spectral_magnitude: f32,
    /// Leaf depth assigned by the hierarchy heuristic.
    pub hierarchy_depth: u8,
}

/// Bounded P1 artifact for one exact resident tile payload.
#[derive(Debug, Clone, PartialEq)]
pub struct P1TileArtifact {
    /// Exact resident source range used for computation.
    pub resident_range: ByteRange,
    /// Whether a larger logical tile is represented by this resident sample.
    pub sampled_overview: bool,
    /// Ranked record-width hypotheses.
    pub alignment_candidates: Vec<AlignmentCandidate>,
    /// Records in the same order as requested source ranges.
    pub records: Vec<P1FeatureRecord>,
}

/// Computes all requested P1 features for exact sample ranges inside one tile.
///
/// # Errors
///
/// Returns an error when parameters are unbounded, ranges escape the resident tile, or checked
/// source-offset arithmetic overflows.
pub fn analyze_p1_tile(
    bytes: &[u8],
    base_offset: u64,
    source_length: u64,
    sample_ranges: &[ByteRange],
    config: P1AnalysisConfig,
    request: P1FeatureRequest,
    sampled_overview: bool,
) -> Result<P1TileArtifact, DomainError> {
    config.validate()?;
    let byte_length = u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?;
    let resident_end = base_offset
        .checked_add(byte_length)
        .ok_or(DomainError::RangeOverflow)?;
    let resident_range = ByteRange::new(base_offset, resident_end)?;
    if source_length < resident_end {
        return Err(DomainError::InvalidRange {
            start: base_offset,
            end: resident_end,
        });
    }
    if sample_ranges.iter().any(|range| {
        range.start < resident_range.start
            || range.end > resident_range.end
            || range.start >= range.end
    }) {
        return Err(DomainError::InvalidTransform(
            "P1 sample range falls outside its resident tile".to_owned(),
        ));
    }

    let hierarchy = request
        .hierarchy
        .then(|| hierarchy_leaves(bytes, config))
        .transpose()?;
    let alignment_candidates = rank_alignment_strides(bytes, config.alignment_max_stride, 8);
    let mut records = Vec::with_capacity(sample_ranges.len());
    for range in sample_ranges {
        let local_start = usize::try_from(range.start.saturating_sub(base_offset))
            .map_err(|_| DomainError::RangeOverflow)?;
        let byte = *bytes.get(local_start).ok_or(DomainError::RangeOverflow)?;
        let point_id = range.start;
        let recurrence = if request.recurrence {
            recurrence_feature(bytes, base_offset, source_length, local_start, config)?
        } else {
            RecurrenceFeature::empty(range.start, source_length)
        };
        let spectrum = if request.spectrum {
            spectrum_feature(bytes, range.start, local_start, source_length, config)
        } else {
            SpectrumFeature::empty(range.start, source_length)
        };
        let hierarchy_feature = hierarchy.as_ref().map_or_else(
            || HierarchyFeature::empty(range.start, source_length),
            |leaves| hierarchy_position(leaves, local_start, range.start, source_length),
        );
        records.push(P1FeatureRecord {
            point_id,
            alignment: alignment_position(
                range.start,
                source_length,
                byte,
                config.alignment_stride,
            ),
            recurrence: recurrence.position,
            repetition: recurrence.repetition_position,
            spectrum: spectrum.position,
            hypercube: hamming_projection(byte),
            hierarchy: hierarchy_feature.position,
            partner_range: recurrence.partner_range,
            recurrence_score: recurrence.score,
            match_length: recurrence.match_length,
            dominant_frequency_bin: spectrum.dominant_bin,
            spectral_magnitude: spectrum.magnitude,
            hierarchy_depth: hierarchy_feature.depth,
        });
    }
    Ok(P1TileArtifact {
        resident_range,
        sampled_overview,
        alignment_candidates,
        records,
    })
}

/// Ranks fixed-record stride hypotheses using exact lag similarity.
#[must_use]
pub fn rank_alignment_strides(
    bytes: &[u8],
    maximum_stride: usize,
    result_limit: usize,
) -> Vec<AlignmentCandidate> {
    let maximum_stride = maximum_stride.min(bytes.len().saturating_sub(1));
    let mut candidates = (2..=maximum_stride)
        .filter_map(|stride| {
            let pairs = bytes.len().saturating_sub(stride);
            (pairs > 0).then(|| {
                let score = bytes.iter().zip(bytes.iter().skip(stride)).fold(
                    0.0_f32,
                    |total, (&first, &second)| {
                        let equality: f32 = if first == second { 1.0 } else { 0.0 };
                        let proximity = 1.0 - (f32::from(first.abs_diff(second)) / 255.0);
                        total + equality.mul_add(0.7, proximity * 0.3)
                    },
                ) / pairs as f32;
                AlignmentCandidate { stride, score }
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|first, second| {
        second
            .score
            .total_cmp(&first.score)
            .then_with(|| first.stride.cmp(&second.stride))
    });
    candidates.truncate(result_limit);
    candidates
}

#[derive(Debug, Clone, Copy)]
struct RecurrenceFeature {
    position: [f32; 3],
    repetition_position: [f32; 3],
    partner_range: Option<ByteRange>,
    score: f32,
    match_length: usize,
}

impl RecurrenceFeature {
    fn empty(source_start: u64, source_length: u64) -> Self {
        let address = normalized_u64(source_start, source_length.saturating_sub(1).max(1));
        Self {
            position: [address, address, -1.0],
            repetition_position: [address, -1.0, -1.0],
            partner_range: None,
            score: 0.0,
            match_length: 0,
        }
    }
}

fn recurrence_feature(
    bytes: &[u8],
    base_offset: u64,
    source_length: u64,
    local_start: usize,
    config: P1AnalysisConfig,
) -> Result<RecurrenceFeature, DomainError> {
    let window = config
        .recurrence_window
        .min(bytes.len().saturating_sub(local_start));
    if window < 4 || local_start == 0 {
        return Ok(RecurrenceFeature::empty(
            base_offset.saturating_add(u64::try_from(local_start).unwrap_or(u64::MAX)),
            source_length,
        ));
    }
    let search_start = local_start.saturating_sub(config.recurrence_search_bytes);
    let candidate_count = local_start.saturating_sub(search_start);
    let step = candidate_count
        .div_ceil(config.recurrence_candidate_budget)
        .max(1);
    let target = bytes
        .get(local_start..local_start.saturating_add(window))
        .ok_or(DomainError::RangeOverflow)?;
    let mut best: Option<(usize, usize)> = None;
    for candidate in (search_start..local_start).step_by(step) {
        let Some(candidate_window) = bytes.get(candidate..candidate.saturating_add(window)) else {
            continue;
        };
        let matches = target
            .iter()
            .zip(candidate_window)
            .filter(|(first, second)| first == second)
            .count();
        if best.is_none_or(|(best_matches, best_offset)| {
            matches > best_matches || (matches == best_matches && candidate > best_offset)
        }) {
            best = Some((matches, candidate));
        }
    }
    let Some((matches, partner_local)) = best else {
        return Ok(RecurrenceFeature::empty(
            base_offset.saturating_add(u64::try_from(local_start).unwrap_or(u64::MAX)),
            source_length,
        ));
    };
    let score = matches as f32 / window as f32;
    if score * 100.0 < f32::from(config.recurrence_threshold_percent) {
        return Ok(RecurrenceFeature::empty(
            base_offset.saturating_add(u64::try_from(local_start).unwrap_or(u64::MAX)),
            source_length,
        ));
    }
    let maximum_match = config.recurrence_window.saturating_mul(16).max(window);
    let mut match_length = 0_usize;
    while match_length < maximum_match
        && bytes.get(local_start.saturating_add(match_length))
            == bytes.get(partner_local.saturating_add(match_length))
    {
        match_length = match_length.saturating_add(1);
    }
    let source_start = base_offset
        .checked_add(u64::try_from(local_start).map_err(|_| DomainError::RangeOverflow)?)
        .ok_or(DomainError::RangeOverflow)?;
    let partner_start = base_offset
        .checked_add(u64::try_from(partner_local).map_err(|_| DomainError::RangeOverflow)?)
        .ok_or(DomainError::RangeOverflow)?;
    let partner_end = partner_start
        .checked_add(u64::try_from(window).map_err(|_| DomainError::RangeOverflow)?)
        .ok_or(DomainError::RangeOverflow)?;
    let partner_range = ByteRange::new(partner_start, partner_end)?;
    let denominator = source_length.saturating_sub(1).max(1);
    let distance = source_start.saturating_sub(partner_start);
    let distance_scale = if source_length > 1 {
        (distance.max(1) as f32).ln() / (source_length as f32).ln().max(1.0)
    } else {
        0.0
    };
    Ok(RecurrenceFeature {
        position: [
            normalized_u64(source_start, denominator),
            normalized_u64(partner_start, denominator),
            score.mul_add(2.0, -1.0),
        ],
        repetition_position: [
            normalized_u64(source_start, denominator),
            distance_scale.mul_add(2.0, -1.0),
            (match_length as f32 / maximum_match as f32)
                .clamp(0.0, 1.0)
                .mul_add(2.0, -1.0),
        ],
        partner_range: Some(partner_range),
        score,
        match_length,
    })
}

#[derive(Debug, Clone, Copy)]
struct SpectrumFeature {
    position: [f32; 3],
    dominant_bin: usize,
    magnitude: f32,
}

impl SpectrumFeature {
    fn empty(source_start: u64, source_length: u64) -> Self {
        Self {
            position: [
                normalized_u64(source_start, source_length.saturating_sub(1).max(1)),
                -1.0,
                -1.0,
            ],
            dominant_bin: 0,
            magnitude: 0.0,
        }
    }
}

fn spectrum_feature(
    bytes: &[u8],
    source_start: u64,
    local_start: usize,
    source_length: u64,
    config: P1AnalysisConfig,
) -> SpectrumFeature {
    let window_length = config
        .spectrum_window
        .min(bytes.len().saturating_sub(local_start));
    let Some(window) = bytes.get(local_start..local_start.saturating_add(window_length)) else {
        return SpectrumFeature::empty(source_start, source_length);
    };
    if window.len() < 8 {
        return SpectrumFeature::empty(source_start, source_length);
    }
    let mean = window.iter().map(|byte| f32::from(*byte)).sum::<f32>() / window.len() as f32;
    let maximum_bin = config.spectrum_bins.min(window.len() / 2).max(1);
    let mut dominant_bin = 1_usize;
    let mut dominant_magnitude = 0.0_f32;
    for bin in 1..=maximum_bin {
        let mut real = 0.0_f32;
        let mut imaginary = 0.0_f32;
        for (index, byte) in window.iter().enumerate() {
            let angle = TAU * bin as f32 * index as f32 / window.len() as f32;
            let centered = f32::from(*byte) - mean;
            real += centered * angle.cos();
            imaginary -= centered * angle.sin();
        }
        let magnitude = real.hypot(imaginary) / (window.len() as f32 * 127.5);
        if magnitude > dominant_magnitude {
            dominant_bin = bin;
            dominant_magnitude = magnitude;
        }
    }
    let magnitude = dominant_magnitude.clamp(0.0, 1.0);
    SpectrumFeature {
        position: [
            normalized_u64(source_start, source_length.saturating_sub(1).max(1)),
            (dominant_bin as f32 / maximum_bin as f32).mul_add(2.0, -1.0),
            magnitude.mul_add(2.0, -1.0),
        ],
        dominant_bin,
        magnitude,
    }
}

fn alignment_position(source_start: u64, source_length: u64, byte: u8, stride: usize) -> [f32; 3] {
    let stride_u64 = u64::try_from(stride).unwrap_or(u64::MAX).max(1);
    let residue = source_start % stride_u64;
    let block = source_start / stride_u64;
    let block_count = source_length.div_ceil(stride_u64).max(1);
    [
        normalized_u64(residue, stride_u64.saturating_sub(1).max(1)),
        (f32::from(byte) / 255.0).mul_add(2.0, -1.0),
        normalized_u64(block, block_count.saturating_sub(1).max(1)),
    ]
}

/// Projects an eight-bit vector through one fixed, cross-file-comparable basis.
#[must_use]
pub fn hamming_projection(byte: u8) -> [f32; 3] {
    const BASIS: [[f32; 3]; 8] = [
        [0.58, 0.00, 0.00],
        [-0.58, 0.00, 0.00],
        [0.00, 0.58, 0.00],
        [0.00, -0.58, 0.00],
        [0.00, 0.00, 0.58],
        [0.00, 0.00, -0.58],
        [0.41, 0.41, 0.41],
        [-0.41, -0.41, -0.41],
    ];
    let mut position = [0.0_f32; 3];
    for (bit, basis) in BASIS.iter().enumerate() {
        let sign = if byte & (1 << bit) == 0 { -1.0 } else { 1.0 };
        for axis in 0..3 {
            position[axis] += basis[axis] * sign;
        }
    }
    for axis in &mut position {
        *axis = (*axis / 2.0).clamp(-1.0, 1.0);
    }
    position
}

#[derive(Debug, Clone, Copy)]
struct HierarchyLeaf {
    start: usize,
    end: usize,
    depth: u8,
    slot: usize,
}

#[derive(Debug, Clone, Copy)]
struct HierarchyFeature {
    position: [f32; 3],
    depth: u8,
}

impl HierarchyFeature {
    fn empty(source_start: u64, source_length: u64) -> Self {
        Self {
            position: [
                normalized_u64(source_start, source_length.saturating_sub(1).max(1)),
                0.0,
                -1.0,
            ],
            depth: 0,
        }
    }
}

fn hierarchy_leaves(
    bytes: &[u8],
    config: P1AnalysisConfig,
) -> Result<Vec<HierarchyLeaf>, DomainError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let mut prefix_sum = Vec::with_capacity(bytes.len().saturating_add(1));
    let mut prefix_printable = Vec::with_capacity(bytes.len().saturating_add(1));
    prefix_sum.push(0_u64);
    prefix_printable.push(0_u64);
    for &byte in bytes {
        let sum = prefix_sum
            .last()
            .copied()
            .unwrap_or(0)
            .checked_add(u64::from(byte))
            .ok_or(DomainError::RangeOverflow)?;
        prefix_sum.push(sum);
        let printable = prefix_printable
            .last()
            .copied()
            .unwrap_or(0)
            .checked_add(u64::from(byte.is_ascii_graphic() || byte == b' '))
            .ok_or(DomainError::RangeOverflow)?;
        prefix_printable.push(printable);
    }
    let mut pending = vec![(0_usize, bytes.len(), 0_u8)];
    let mut leaves = Vec::new();
    while let Some((start, end, depth)) = pending.pop() {
        let length = end.saturating_sub(start);
        let split = if depth < config.hierarchy_max_depth
            && length >= config.hierarchy_min_block.saturating_mul(2)
        {
            best_hierarchy_split(
                &prefix_sum,
                &prefix_printable,
                start,
                end,
                config.hierarchy_min_block,
                config.hierarchy_threshold_percent,
            )
        } else {
            None
        };
        if let Some(split) = split {
            pending.push((split, end, depth.saturating_add(1)));
            pending.push((start, split, depth.saturating_add(1)));
        } else {
            leaves.push(HierarchyLeaf {
                start,
                end,
                depth,
                slot: 0,
            });
        }
    }
    leaves.sort_by_key(|leaf| leaf.start);
    for (slot, leaf) in leaves.iter_mut().enumerate() {
        leaf.slot = slot;
    }
    Ok(leaves)
}

fn best_hierarchy_split(
    prefix_sum: &[u64],
    prefix_printable: &[u64],
    start: usize,
    end: usize,
    minimum: usize,
    threshold_percent: u8,
) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for split in (start.saturating_add(minimum)..=end.saturating_sub(minimum)).step_by(minimum) {
        let left_len = split.saturating_sub(start);
        let right_len = end.saturating_sub(split);
        let left_mean =
            prefix_sum[split].saturating_sub(prefix_sum[start]) as f32 / left_len as f32;
        let right_mean =
            prefix_sum[end].saturating_sub(prefix_sum[split]) as f32 / right_len as f32;
        let left_printable = prefix_printable[split].saturating_sub(prefix_printable[start]) as f32
            / left_len as f32;
        let right_printable =
            prefix_printable[end].saturating_sub(prefix_printable[split]) as f32 / right_len as f32;
        let score = ((left_mean - right_mean).abs() / 255.0)
            .mul_add(0.65, (left_printable - right_printable).abs() * 0.35);
        if best.is_none_or(|(best_score, _)| score > best_score) {
            best = Some((score, split));
        }
    }
    best.filter(|(score, _)| *score * 100.0 >= f32::from(threshold_percent))
        .map(|(_, split)| split)
}

fn hierarchy_position(
    leaves: &[HierarchyLeaf],
    local_start: usize,
    source_start: u64,
    source_length: u64,
) -> HierarchyFeature {
    let Some(leaf) = leaves
        .iter()
        .find(|leaf| leaf.start <= local_start && local_start < leaf.end)
    else {
        return HierarchyFeature::empty(source_start, source_length);
    };
    let local = local_start.saturating_sub(leaf.start);
    let length = leaf.end.saturating_sub(leaf.start).max(1);
    let slot_denominator = leaves.len().saturating_sub(1).max(1);
    HierarchyFeature {
        position: [
            normalized_usize(leaf.slot, slot_denominator),
            normalized_usize(local, length.saturating_sub(1).max(1)),
            normalized_usize(usize::from(leaf.depth), 16),
        ],
        depth: leaf.depth,
    }
}

#[allow(clippy::cast_precision_loss)]
fn normalized_u64(value: u64, denominator: u64) -> f32 {
    (value as f32 / denominator.max(1) as f32).mul_add(2.0, -1.0)
}

#[allow(clippy::cast_precision_loss)]
fn normalized_usize(value: usize, denominator: usize) -> f32 {
    (value as f32 / denominator.max(1) as f32).mul_add(2.0, -1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(length: usize, window: usize, hop: usize) -> Result<Vec<ByteRange>, DomainError> {
        (0..=length.saturating_sub(window))
            .step_by(hop)
            .map(|start| {
                ByteRange::new(
                    u64::try_from(start).map_err(|_| DomainError::RangeOverflow)?,
                    u64::try_from(start.saturating_add(window))
                        .map_err(|_| DomainError::RangeOverflow)?,
                )
            })
            .collect()
    }

    #[test]
    fn alignment_ranking_retains_the_known_record_width() {
        let mut bytes = Vec::new();
        for record in 0_u8..64 {
            bytes.extend_from_slice(&[0x53, record, 0, 0, 0x7f, 0, 0, 0]);
            bytes.extend_from_slice(&[0x54, record, 0, 0, 0x3f, 0, 0, 0]);
        }
        let ranked = rank_alignment_strides(&bytes, 64, 12);
        assert!(ranked.iter().any(|candidate| candidate.stride == 16));
    }

    #[test]
    fn recurrence_retains_an_exact_partner_range() -> Result<(), DomainError> {
        let bytes = b"0123456789abcdef----0123456789abcdef";
        let samples = vec![ByteRange::new(20, 36)?];
        let artifact = analyze_p1_tile(
            bytes,
            0,
            u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?,
            &samples,
            P1AnalysisConfig {
                recurrence_search_bytes: 64,
                ..P1AnalysisConfig::default()
            },
            P1FeatureRequest {
                recurrence: true,
                ..P1FeatureRequest::default()
            },
            false,
        )?;
        assert_eq!(
            artifact.records[0].partner_range,
            Some(ByteRange::new(0, 16)?)
        );
        assert_eq!(artifact.records[0].match_length, 16);
        Ok(())
    }

    #[test]
    fn spectrum_finds_period_four_signal() -> Result<(), DomainError> {
        let bytes = [0_u8, 255, 0, 255].repeat(32);
        let artifact = analyze_p1_tile(
            &bytes,
            0,
            u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?,
            &[ByteRange::new(0, 64)?],
            P1AnalysisConfig::default(),
            P1FeatureRequest {
                spectrum: true,
                ..P1FeatureRequest::default()
            },
            false,
        )?;
        assert_eq!(artifact.records[0].dominant_frequency_bin, 32);
        assert!(artifact.records[0].spectral_magnitude > 0.9);
        Ok(())
    }

    #[test]
    fn hierarchy_splits_a_strong_feature_boundary() -> Result<(), DomainError> {
        let mut bytes = vec![0_u8; 256];
        bytes.extend(std::iter::repeat_n(0xff, 256));
        let artifact = analyze_p1_tile(
            &bytes,
            0,
            u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?,
            &ranges(bytes.len(), 32, 32)?,
            P1AnalysisConfig::default(),
            P1FeatureRequest {
                hierarchy: true,
                ..P1FeatureRequest::default()
            },
            false,
        )?;
        assert!(
            artifact
                .records
                .iter()
                .any(|record| record.hierarchy_depth > 0)
        );
        Ok(())
    }

    #[test]
    fn fixed_hamming_basis_is_deterministic_and_bit_sensitive() {
        let zero = hamming_projection(0);
        let repeated = hamming_projection(0);
        assert!(
            zero.into_iter()
                .zip(repeated)
                .all(|(first, second)| (first - second).abs() < f32::EPSILON)
        );
        assert!(
            hamming_projection(0)
                .into_iter()
                .zip(hamming_projection(1))
                .any(|(first, second)| (first - second).abs() > f32::EPSILON)
        );
        assert!(
            hamming_projection(1)
                .into_iter()
                .zip(hamming_projection(2))
                .any(|(first, second)| (first - second).abs() > f32::EPSILON)
        );
    }
}
