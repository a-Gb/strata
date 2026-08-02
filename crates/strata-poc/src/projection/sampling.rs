//! Bounded sampling and feature extraction for reusable projection records.

use super::color::{entropy_color, spectral_color};
use super::coordinates::{
    entropy_terrain_position, orbit_position, sequence_position, trigram_position,
};
use super::model::{ProjectionDomain, ProjectionSample, ProjectionSamplingConfig};

/// Samples reusable three-byte projection records without exceeding the point budget.
#[cfg(test)]
pub(crate) fn sample_projection_samples(
    bytes: &[u8],
    stride: usize,
    point_budget: usize,
) -> Vec<ProjectionSample> {
    sample_projection_samples_at_offset(bytes, 0, stride, point_budget)
}

/// Samples a byte range while retaining its absolute source-file offsets.
pub(crate) fn sample_projection_samples_at_offset(
    bytes: &[u8],
    base_offset: usize,
    stride: usize,
    point_budget: usize,
) -> Vec<ProjectionSample> {
    sample_projection_samples_with_config(
        bytes,
        base_offset,
        ProjectionSamplingConfig::legacy(stride),
        point_budget,
    )
}

pub(crate) fn sample_projection_samples_with_config(
    bytes: &[u8],
    base_offset: usize,
    config: ProjectionSamplingConfig,
    point_budget: usize,
) -> Vec<ProjectionSample> {
    let source_length = base_offset.saturating_add(bytes.len());
    sample_projection_samples_in_source(bytes, base_offset, source_length, config, point_budget)
}

pub(crate) fn sample_projection_samples_in_source(
    bytes: &[u8],
    base_offset: usize,
    source_length: usize,
    config: ProjectionSamplingConfig,
    point_budget: usize,
) -> Vec<ProjectionSample> {
    if point_budget == 0 {
        return Vec::new();
    }
    let Some(analysis_span) = projection_analysis_span(config) else {
        return Vec::new();
    };
    let Some(last_start) = bytes.len().checked_sub(analysis_span) else {
        return Vec::new();
    };
    let base_hop = match config.domain {
        ProjectionDomain::Byte => 1,
        ProjectionDomain::Word | ProjectionDomain::Window => config.hop_bytes.max(1),
        ProjectionDomain::Region => config.aggregation_bytes.max(1),
    };
    let eligible = last_start / base_hop + 1;
    let sample_factor = eligible.div_ceil(point_budget).max(1);
    let sample_step = base_hop.saturating_mul(sample_factor).max(1);
    let denominator = bytes.len().saturating_sub(1).max(1);

    (0..=last_start)
        .step_by(sample_step)
        .filter_map(|offset| {
            projection_sample(
                bytes,
                base_offset,
                offset,
                analysis_span,
                config,
                denominator,
                source_length,
            )
        })
        .collect()
}

/// Builds one exact projection sample anchored at a requested absolute source offset.
///
/// This supplements bounded uniform sampling for sparse evidence such as short
/// signature matches. The returned sample retains the same deterministic point
/// identity and contributor semantics as ordinary samples.
pub(crate) fn sample_projection_sample_at_source_offset(
    bytes: &[u8],
    base_offset: usize,
    source_length: usize,
    config: ProjectionSamplingConfig,
    source_offset: usize,
) -> Option<ProjectionSample> {
    let analysis_span = projection_analysis_span(config)?;
    let last_start = bytes.len().checked_sub(analysis_span)?;
    let requested_offset = source_offset.checked_sub(base_offset)?;
    if requested_offset >= bytes.len() {
        return None;
    }
    let relative_offset = requested_offset.min(last_start);
    projection_sample(
        bytes,
        base_offset,
        relative_offset,
        analysis_span,
        config,
        bytes.len().saturating_sub(1).max(1),
        source_length,
    )
}

fn projection_analysis_span(config: ProjectionSamplingConfig) -> Option<usize> {
    if config.lag == 0 {
        return None;
    }
    let word_bytes = usize::from(config.word_bits / 8).max(1);
    match config.domain {
        ProjectionDomain::Byte => config
            .lag
            .checked_mul(2)
            .and_then(|span| span.checked_add(1)),
        ProjectionDomain::Word => Some(word_bytes),
        ProjectionDomain::Window => Some(config.window_bytes),
        ProjectionDomain::Region => Some(config.aggregation_bytes),
    }
    .filter(|span| *span > 0)
}

#[allow(clippy::too_many_arguments)]
fn projection_sample(
    bytes: &[u8],
    base_offset: usize,
    offset: usize,
    analysis_span: usize,
    config: ProjectionSamplingConfig,
    denominator: usize,
    source_length: usize,
) -> Option<ProjectionSample> {
    let (second_offset, third_offset) = match config.domain {
        ProjectionDomain::Byte => {
            let second = offset.checked_add(config.lag)?;
            (second, second.checked_add(config.lag)?)
        }
        ProjectionDomain::Word | ProjectionDomain::Window | ProjectionDomain::Region => (
            offset.checked_add(analysis_span / 2)?,
            offset.checked_add(analysis_span.saturating_sub(1))?,
        ),
    };
    let first = *bytes.get(offset)?;
    let second = *bytes.get(second_offset)?;
    let third = *bytes.get(third_offset)?;
    let progress = ratio(offset, denominator);
    let analysis_end = offset.checked_add(analysis_span)?.min(bytes.len());
    let analysis = bytes.get(offset..analysis_end)?;
    let entropy = entropy_for_window(analysis);
    let change_rate = change_rate(analysis);
    let unique_fraction = unique_fraction(analysis);
    let triplet = trigram_position(first, second, third);
    let orbit = orbit_position(first, second, third);
    let sequence = sequence_position(first, third, progress);
    let terrain = entropy_terrain_position(first, third, progress, entropy);
    let absolute_first = base_offset.checked_add(offset)?;
    let absolute_second = base_offset.checked_add(second_offset)?;
    let absolute_third = base_offset.checked_add(third_offset)?;
    let absolute_end = base_offset.checked_add(analysis_end)?;
    let point_id = u64::try_from(absolute_first).ok()?;

    Some(ProjectionSample {
        positions: [triplet, orbit, sequence, terrain],
        terrain_flat: [terrain[0], 0.0, terrain[2]],
        colors: [
            spectral_color(progress, first),
            entropy_color(entropy, first),
        ],
        bytes: [first, second, third],
        relative_offset: absolute_first,
        source_length: source_length.max(absolute_end),
        entropy,
        change_rate,
        unique_fraction,
        analysis_range: [absolute_first, absolute_end],
        point_id,
        source_offsets: [absolute_first, absolute_second, absolute_third],
        p1: None,
    })
}

#[cfg(test)]
pub(super) fn local_entropy(bytes: &[u8], offset: usize) -> f32 {
    let end = offset.saturating_add(64).min(bytes.len());
    let Some(window) = bytes.get(offset..end) else {
        return 0.0;
    };
    entropy_for_window(window)
}

#[allow(clippy::cast_precision_loss)]
fn entropy_for_window(window: &[u8]) -> f32 {
    if window.is_empty() {
        return 0.0;
    }
    let mut counts = [0_u16; 256];
    for byte in window {
        counts[usize::from(*byte)] = counts[usize::from(*byte)].saturating_add(1);
    }
    let length = window.len() as f32;
    let entropy = counts
        .iter()
        .filter(|count| **count > 0)
        .fold(0.0, |accumulator, count| {
            let probability = f32::from(*count) / length;
            probability.mul_add(-probability.log2(), accumulator)
        });
    let maximum_entropy = (window.len().min(256) as f32).log2().max(1.0);
    (entropy / maximum_entropy).clamp(0.0, 1.0)
}

#[allow(clippy::cast_precision_loss)]
fn change_rate(window: &[u8]) -> f32 {
    let pair_count = window.len().saturating_sub(1);
    if pair_count == 0 {
        return 0.0;
    }
    let total = window
        .windows(2)
        .map(|pair| f32::from(pair[0].abs_diff(pair[1])) / 255.0)
        .sum::<f32>();
    (total / pair_count as f32).clamp(0.0, 1.0)
}

#[allow(clippy::cast_precision_loss)]
fn unique_fraction(window: &[u8]) -> f32 {
    if window.is_empty() {
        return 0.0;
    }
    let mut seen = [false; 256];
    for byte in window {
        seen[usize::from(*byte)] = true;
    }
    let unique = seen.into_iter().filter(|present| *present).count();
    (unique as f32 / window.len().min(256) as f32).clamp(0.0, 1.0)
}

#[allow(clippy::cast_precision_loss)]
fn ratio(numerator: usize, denominator: usize) -> f32 {
    numerator as f32 / denominator as f32
}
