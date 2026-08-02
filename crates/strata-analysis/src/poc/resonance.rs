//! Bounded, deterministic similarity scans over exact byte windows.

use strata_core::DomainError;

use super::statistics::{classify_byte, shannon_entropy_bits};

/// The evidence used to compare a selected byte window with candidate windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResonanceMetric {
    /// Positional byte equality; only identical values produce a perfect score.
    ExactBytes,
    /// Positional equality of coarse byte classes, robust to value substitutions.
    ByteShape,
    /// Similarity of high-nibble distribution and Shannon entropy.
    Texture,
}

/// One sampled candidate in a selection-resonance scan.
#[derive(Debug, Clone, PartialEq)]
pub struct ResonanceMatch {
    /// Absolute source offset of the candidate window.
    pub offset: u64,
    /// Exact candidate and probe window length.
    pub length: u64,
    /// Similarity in the inclusive range `0.0..=1.0`.
    pub score: f64,
}

/// A bounded, deterministic similarity scan driven by one source selection.
#[derive(Debug, Clone, PartialEq)]
pub struct ResonanceScan {
    /// Absolute source offset of the probe window.
    pub probe_offset: u64,
    /// Exact length used for every comparison.
    pub window_size: u64,
    /// Distance between sampled candidate offsets.
    pub sampled_step: u64,
    /// Number of stride-aligned positions before bounded sampling.
    pub total_positions: u64,
    /// Sampled candidates ordered by source offset; the probe is always included.
    pub matches: Vec<ResonanceMatch>,
}

/// Compares a selected source window with bounded candidates across the source.
///
/// `stride` is the minimum candidate spacing. When the number of candidates is
/// greater than `max_samples`, the function increases that spacing
/// deterministically. The exact probe position is included even when it falls
/// between sampled positions. Scores are exact for every returned candidate;
/// coverage of the whole source may be sampled.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransform`] for zero-sized parameters or a
/// probe outside `data`, and [`DomainError::RangeOverflow`] when address
/// arithmetic or source-offset conversion overflows.
pub fn selection_resonance(
    data: &[u8],
    probe_offset: usize,
    window_size: usize,
    stride: usize,
    max_samples: usize,
    metric: ResonanceMetric,
) -> Result<ResonanceScan, DomainError> {
    if window_size == 0 || stride == 0 || max_samples == 0 {
        return Err(DomainError::InvalidTransform(
            "POC resonance window, stride, and sample budget must be nonzero".to_owned(),
        ));
    }
    if data.is_empty() || probe_offset >= data.len() {
        return Err(DomainError::InvalidTransform(
            "POC resonance probe must address a source byte".to_owned(),
        ));
    }

    let window_size = window_size.min(data.len().saturating_sub(probe_offset));
    let probe_end = probe_offset
        .checked_add(window_size)
        .ok_or(DomainError::RangeOverflow)?;
    let probe = data
        .get(probe_offset..probe_end)
        .ok_or(DomainError::RangeOverflow)?;
    let last_start = data.len().saturating_sub(window_size);
    let total_positions = last_start
        .checked_div(stride)
        .and_then(|count| count.checked_add(1))
        .ok_or(DomainError::RangeOverflow)?;
    let sample_jump = total_positions.div_ceil(max_samples).max(1);
    let sampled_step = stride
        .checked_mul(sample_jump)
        .ok_or(DomainError::RangeOverflow)?;
    let source_length = u64::try_from(window_size).map_err(|_| DomainError::RangeOverflow)?;

    let mut matches = Vec::with_capacity(total_positions.min(max_samples).saturating_add(1));
    let mut candidate_offset = 0_usize;
    loop {
        let candidate_end = candidate_offset
            .checked_add(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let candidate = data
            .get(candidate_offset..candidate_end)
            .ok_or(DomainError::RangeOverflow)?;
        matches.push(ResonanceMatch {
            offset: u64::try_from(candidate_offset).map_err(|_| DomainError::RangeOverflow)?,
            length: source_length,
            score: resonance_score(probe, candidate, metric),
        });
        let Some(next_offset) = candidate_offset.checked_add(sampled_step) else {
            return Err(DomainError::RangeOverflow);
        };
        if next_offset > last_start {
            break;
        }
        candidate_offset = next_offset;
    }

    let probe_source_offset =
        u64::try_from(probe_offset).map_err(|_| DomainError::RangeOverflow)?;
    if !matches
        .iter()
        .any(|candidate| candidate.offset == probe_source_offset)
    {
        matches.push(ResonanceMatch {
            offset: probe_source_offset,
            length: source_length,
            score: 1.0,
        });
        matches.sort_unstable_by_key(|candidate| candidate.offset);
    }

    Ok(ResonanceScan {
        probe_offset: probe_source_offset,
        window_size: source_length,
        sampled_step: u64::try_from(sampled_step).map_err(|_| DomainError::RangeOverflow)?,
        total_positions: u64::try_from(total_positions).map_err(|_| DomainError::RangeOverflow)?,
        matches,
    })
}

#[allow(clippy::cast_precision_loss)]
fn resonance_score(probe: &[u8], candidate: &[u8], metric: ResonanceMetric) -> f64 {
    if probe.is_empty() || probe.len() != candidate.len() {
        return 0.0;
    }
    let length = probe.len() as f64;
    match metric {
        ResonanceMetric::ExactBytes => {
            probe
                .iter()
                .zip(candidate)
                .filter(|(first, second)| first == second)
                .count() as f64
                / length
        }
        ResonanceMetric::ByteShape => {
            probe
                .iter()
                .zip(candidate)
                .filter(|(first, second)| classify_byte(**first) == classify_byte(**second))
                .count() as f64
                / length
        }
        ResonanceMetric::Texture => {
            let mut probe_bins = [0_u64; 16];
            let mut candidate_bins = [0_u64; 16];
            for &byte in probe {
                probe_bins[usize::from(byte >> 4)] += 1;
            }
            for &byte in candidate {
                candidate_bins[usize::from(byte >> 4)] += 1;
            }
            let distribution_delta: u64 = probe_bins
                .iter()
                .zip(candidate_bins)
                .map(|(&first, second)| first.abs_diff(second))
                .sum();
            let distribution = 1.0 - (distribution_delta as f64 / (2.0 * length));
            let entropy_delta =
                (shannon_entropy_bits(probe) - shannon_entropy_bits(candidate)).abs();
            let entropy = 1.0 - (entropy_delta / 8.0).clamp(0.0, 1.0);
            distribution.mul_add(0.72, entropy * 0.28).clamp(0.0, 1.0)
        }
    }
}
