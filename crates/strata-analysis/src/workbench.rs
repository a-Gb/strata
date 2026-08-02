//! Bounded, deterministic evidence discovery for the POC workbench.
//!
//! These helpers never mutate their source slice. Every lead names the exact
//! source ranges it inspected, and transform output is returned as a separate
//! value so callers can keep provenance explicit.

use std::collections::{BTreeMap, BTreeSet};

use strata_core::{ByteRange, DomainError};

use crate::poc::{
    DiscoveryConfig, DiscoveryEvidence, MAX_DISCOVERY_FINDINGS, MAX_DISCOVERY_WINDOWS,
    discover_findings,
};
use crate::signatures::{SignatureMatchEvidence, SignatureScanReport};

/// Maximum source prefix examined by one workbench pass.
pub const MAX_WORKBENCH_BYTES: usize = 4 * 1024 * 1024;
/// Maximum leads returned by one workbench pass.
pub const MAX_WORKBENCH_LEADS: usize = 64;
/// Maximum candidate period checked for record-width evidence.
pub const MAX_WORKBENCH_PERIOD: usize = 512;
/// Prefix cap used while ranking transform candidates.
pub const MAX_TRANSFORM_RANKING_BYTES: usize = 64 * 1024;
/// Maximum local source blocks evaluated for periodicity evidence.
pub const MAX_PERIODICITY_BLOCKS: usize = 64;
const PERIODICITY_BLOCK_BYTES: usize = 256;

/// Bounded options for [`analyze_workbench`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkbenchConfig {
    /// Maximum contiguous source bytes inspected from offset zero.
    pub max_inspected_bytes: usize,
    /// Block size used to measure entropy transitions.
    pub entropy_block_size: usize,
    /// Fixed window size used for exact repeat and XOR-correlation checks.
    pub repeat_window_size: usize,
    /// Maximum fixed windows indexed for repeat checks.
    pub max_repeat_windows: usize,
    /// Largest period considered for record-width evidence.
    pub max_period: usize,
    /// Maximum ranked heterogeneous leads returned.
    pub max_leads: usize,
}

impl Default for WorkbenchConfig {
    fn default() -> Self {
        Self {
            max_inspected_bytes: 256 * 1024,
            entropy_block_size: 64,
            repeat_window_size: 16,
            max_repeat_windows: 1_024,
            max_period: 128,
            max_leads: 24,
        }
    }
}

/// Stable, deterministic identifier for a workbench lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkbenchLeadId(pub u64);

/// The detector that produced a [`WorkbenchLead`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WorkbenchLeadKind {
    /// A sharp change in adjacent block Shannon entropy.
    EntropyBoundary,
    /// Two fixed source windows compare byte-for-byte equal.
    ExactRepeat,
    /// A byte period has above-baseline exact positional agreement.
    Periodicity,
    /// A known magic sequence occurs at an exact source offset.
    EmbeddedSignature,
    /// A reversible transform has explicit supporting evidence.
    TransformCandidate,
    /// An external catalog pattern matched exact source bytes.
    CatalogSignature,
}

/// Recognized byte signatures. A match is a lead, never a type assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EmbeddedSignature {
    /// Portable Network Graphics prefix.
    Png,
    /// Executable and Linkable Format prefix.
    Elf,
    /// Portable Document Format prefix.
    Pdf,
    /// ZIP local-file header prefix.
    Zip,
    /// DOS MZ executable prefix.
    Mz,
}

/// A reversible transform supported by this dependency-free POC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReversibleTransform {
    /// XOR every byte with a constant key. Applying the same transform restores input.
    XorByte(u8),
}

/// Interpretation of the evidence delta from a reversible transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransformAssessment {
    /// Measurements materially improve after the transform.
    Supported,
    /// Measurements do not materially change in either direction.
    Neutral,
    /// Measurements materially degrade after the transform.
    Contradicted,
}

/// Compact byte-distribution measurements used by the workbench and transforms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactMetrics {
    /// Number of bytes measured.
    pub byte_count: u64,
    /// Distinct byte values observed, from zero through 256.
    pub distinct_byte_count: u16,
    /// Shannon entropy in bits per byte.
    pub entropy_bits: f64,
    /// Fraction of zero bytes.
    pub zero_fraction: f64,
    /// Fraction of ASCII letters, digits, whitespace, or common punctuation.
    pub text_likelihood: f64,
}

/// Before/after evidence for a transform candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformEvaluation {
    /// Reversible operation evaluated without mutating the source.
    pub transform: ReversibleTransform,
    /// Exact source range used for both measurements.
    pub source_range: ByteRange,
    /// Explicit evidence interpretation.
    pub assessment: TransformAssessment,
    /// Measurements on the original source bytes.
    pub before: CompactMetrics,
    /// Measurements on separately derived transformed bytes.
    pub after: CompactMetrics,
    /// `after.text_likelihood - before.text_likelihood`.
    pub text_likelihood_delta: f64,
    /// `after.zero_fraction - before.zero_fraction`.
    pub zero_fraction_delta: f64,
    /// `after.entropy_bits - before.entropy_bits`.
    pub entropy_delta_bits: f64,
}

/// Detector-specific evidence for a ranked lead.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkbenchEvidence {
    /// Adjacent exact blocks with an entropy discontinuity.
    EntropyBoundary {
        /// Entropy of the range before the boundary.
        before_entropy_bits: f64,
        /// Entropy of the range after the boundary.
        after_entropy_bits: f64,
        /// Absolute entropy difference in bits per byte.
        entropy_delta_bits: f64,
    },
    /// Evidence for two exact equal windows.
    ExactRepeat {
        /// Number of source bytes compared exactly.
        matching_byte_count: u64,
        /// Candidate start spacing after deterministic bounded sampling.
        sampled_window_step: u64,
    },
    /// Exact positional agreement at a candidate period.
    Periodicity {
        /// Candidate record width or period in bytes.
        period_bytes: u64,
        /// Number of positions compared at that period.
        compared_positions: u64,
        /// Number of positions with equal bytes.
        matching_positions: u64,
    },
    /// Exact byte signature evidence.
    EmbeddedSignature {
        /// Signature pattern found at `source_ranges[0]`.
        signature: EmbeddedSignature,
    },
    /// Before/after evidence for a non-mutating reversible transform.
    TransformCandidate(TransformEvaluation),
    /// Two exact source ranges related by a reversible XOR transform.
    XorCorrelatedTransform {
        /// Explicit reversible operation and key.
        transform: ReversibleTransform,
        /// First exact range in the relationship.
        source_range: ByteRange,
        /// Second exact range, where every byte is `source ^ key`.
        transformed_range: ByteRange,
        /// Exact number of aligned byte pairs satisfying the relation.
        correlated_byte_count: u64,
        /// Distinct byte values in either range; XOR preserves this count.
        distinct_byte_count: u16,
        /// Source spacing of sampled fixed windows before adjacent pairs merged.
        sampled_window_step: u64,
    },
    /// Exact external-catalog pattern evidence with retained provenance.
    CatalogSignature(SignatureMatchEvidence),
}

/// A ranked, immutable-source discovery lead for the workbench.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbenchLead {
    /// Stable ID derived from detector kind, exact ranges, and parameter bytes.
    pub id: WorkbenchLeadId,
    /// Detector category.
    pub kind: WorkbenchLeadKind,
    /// Exact half-open source ranges supporting this lead.
    pub source_ranges: Vec<ByteRange>,
    /// Deterministic score in the inclusive range `0.0..=1.0`.
    pub confidence: f64,
    /// Measurements sufficient for a UI to explain the ranking.
    pub evidence: WorkbenchEvidence,
}

/// Complete bounded result returned by [`analyze_workbench`].
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbenchReport {
    /// Contiguous prefix analyzed by this report.
    pub inspected_range: ByteRange,
    /// Compact measurements for that exact prefix.
    pub metrics: CompactMetrics,
    /// Heterogeneous leads in stable ranked order.
    pub leads: Vec<WorkbenchLead>,
}

/// Analyzes a bounded immutable source prefix and ranks heterogeneous leads.
///
/// The report can include entropy boundaries, exact repeats, periodicity,
/// embedded magic sequences, and reversible XOR transform candidates. Each
/// result is evidence, not an automatic parser or file-type conclusion.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransform`] for invalid or unbounded options,
/// and [`DomainError::RangeOverflow`] when a range cannot fit in `u64`.
pub fn analyze_workbench(
    data: &[u8],
    config: WorkbenchConfig,
) -> Result<WorkbenchReport, DomainError> {
    validate_config(config)?;
    let length = data.len().min(config.max_inspected_bytes);
    let inspected = data.get(..length).ok_or(DomainError::RangeOverflow)?;
    let inspected_range = range_from_usize(0, length)?;
    let metrics = compact_metrics(inspected)?;
    if inspected.is_empty() {
        return Ok(WorkbenchReport {
            inspected_range,
            metrics,
            leads: Vec::new(),
        });
    }

    let mut leads = Vec::new();
    leads.extend(entropy_boundary_leads(
        inspected,
        config.entropy_block_size,
    )?);
    leads.extend(exact_repeat_leads(
        inspected,
        config.repeat_window_size,
        config.max_repeat_windows,
    )?);
    leads.extend(periodicity_leads(inspected, config.max_period)?);
    leads.extend(signature_leads(inspected)?);
    leads.extend(correlated_transform_leads(
        inspected,
        config.repeat_window_size,
        config.max_repeat_windows,
    )?);
    leads.extend(transform_leads(inspected)?);
    sort_leads(&mut leads);
    leads = diversify_leads(leads, config.max_leads);
    Ok(WorkbenchReport {
        inspected_range,
        metrics,
        leads,
    })
}

/// Converts a bounded external signature scan into ordinary workbench leads.
///
/// Catalog matches enter the same selection, ranking, provenance, and notebook
/// path as built-in evidence without changing the immutable source.
#[must_use]
pub fn catalog_signature_leads(report: &SignatureScanReport) -> Vec<WorkbenchLead> {
    report
        .matches
        .iter()
        .map(|matched| WorkbenchLead {
            id: lead_id(
                WorkbenchLeadKind::CatalogSignature,
                &[matched.source_range],
                matched.id,
            ),
            kind: WorkbenchLeadKind::CatalogSignature,
            source_ranges: vec![matched.source_range],
            confidence: matched.confidence,
            evidence: WorkbenchEvidence::CatalogSignature(matched.evidence.clone()),
        })
        .collect()
}

fn sort_leads(leads: &mut [WorkbenchLead]) {
    leads.sort_unstable_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn diversify_leads(leads: Vec<WorkbenchLead>, limit: usize) -> Vec<WorkbenchLead> {
    let mut selected = Vec::with_capacity(limit);
    let mut selected_ids = BTreeSet::new();
    let mut represented_kinds = BTreeSet::new();
    for lead in &leads {
        if selected.len() == limit {
            break;
        }
        if represented_kinds.insert(lead.kind) {
            selected_ids.insert(lead.id);
            selected.push(lead.clone());
        }
    }
    for lead in leads {
        if selected.len() == limit {
            break;
        }
        if selected_ids.insert(lead.id) {
            selected.push(lead);
        }
    }
    sort_leads(&mut selected);
    selected
}

/// Applies a reversible transform to a new buffer, leaving `source` unchanged.
#[must_use]
pub fn apply_reversible_transform(source: &[u8], transform: ReversibleTransform) -> Vec<u8> {
    match transform {
        ReversibleTransform::XorByte(key) => source.iter().map(|byte| *byte ^ key).collect(),
    }
}

/// Evaluates a transform over one exact source range with before/after deltas.
///
/// Applying the returned transform again restores the original bytes for every
/// currently supported transform.
///
/// # Errors
///
/// Returns [`DomainError::InvalidRange`] if `source_range` is outside `data`,
/// and [`DomainError::RangeOverflow`] for unrepresentable range conversion.
pub fn evaluate_transform_candidate(
    data: &[u8],
    source_range: ByteRange,
    transform: ReversibleTransform,
) -> Result<TransformEvaluation, DomainError> {
    let source = slice_for_range(data, source_range)?;
    let before = compact_metrics(source)?;
    let derived = apply_reversible_transform(source, transform);
    let after = compact_metrics(&derived)?;
    let text_likelihood_delta = after.text_likelihood - before.text_likelihood;
    let zero_fraction_delta = after.zero_fraction - before.zero_fraction;
    let entropy_delta_bits = after.entropy_bits - before.entropy_bits;
    let assessment = if text_likelihood_delta >= 0.15 && after.text_likelihood >= 0.65 {
        TransformAssessment::Supported
    } else if text_likelihood_delta <= -0.15 {
        TransformAssessment::Contradicted
    } else {
        TransformAssessment::Neutral
    };
    Ok(TransformEvaluation {
        transform,
        source_range,
        assessment,
        before,
        after,
        text_likelihood_delta,
        zero_fraction_delta,
        entropy_delta_bits,
    })
}

fn validate_config(config: WorkbenchConfig) -> Result<(), DomainError> {
    if config.max_inspected_bytes == 0
        || config.max_inspected_bytes > MAX_WORKBENCH_BYTES
        || config.entropy_block_size == 0
        || config.repeat_window_size == 0
        || config.max_repeat_windows == 0
        || config.max_repeat_windows > MAX_WORKBENCH_LEADS * 64
        || config.max_period == 0
        || config.max_period > MAX_WORKBENCH_PERIOD
        || config.max_leads == 0
        || config.max_leads > MAX_WORKBENCH_LEADS
    {
        return Err(DomainError::InvalidTransform(
            "invalid bounded workbench configuration".to_owned(),
        ));
    }
    Ok(())
}

fn entropy_boundary_leads(
    data: &[u8],
    block_size: usize,
) -> Result<Vec<WorkbenchLead>, DomainError> {
    let mut blocks = Vec::new();
    for offset in (0..data.len()).step_by(block_size) {
        let end = offset
            .checked_add(block_size)
            .ok_or(DomainError::RangeOverflow)?
            .min(data.len());
        let range = range_from_usize(offset, end)?;
        blocks.push((
            range,
            compact_metrics(data.get(offset..end).ok_or(DomainError::RangeOverflow)?)?.entropy_bits,
        ));
    }
    let mut leads = Vec::new();
    for pair in blocks.windows(2) {
        let before = pair[0];
        let after = pair[1];
        let delta = (before.1 - after.1).abs();
        if delta >= 0.50 {
            let ranges = [before.0, after.0];
            leads.push(WorkbenchLead {
                id: lead_id(WorkbenchLeadKind::EntropyBoundary, &ranges, 0),
                kind: WorkbenchLeadKind::EntropyBoundary,
                source_ranges: ranges.to_vec(),
                confidence: (delta / 8.0).clamp(0.0, 1.0),
                evidence: WorkbenchEvidence::EntropyBoundary {
                    before_entropy_bits: before.1,
                    after_entropy_bits: after.1,
                    entropy_delta_bits: delta,
                },
            });
        }
    }
    Ok(leads)
}

fn exact_repeat_leads(
    data: &[u8],
    window_size: usize,
    max_windows: usize,
) -> Result<Vec<WorkbenchLead>, DomainError> {
    let available = data.len() / window_size;
    if available < 2 {
        return Ok(Vec::new());
    }
    let step_windows = available.div_ceil(max_windows).max(1);
    let sampled_step = window_size
        .checked_mul(step_windows)
        .ok_or(DomainError::RangeOverflow)?;
    let mut index = BTreeMap::<Vec<u8>, usize>::new();
    let mut leads = Vec::new();
    for index_window in (0..available).step_by(step_windows) {
        let offset = index_window
            .checked_mul(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let end = offset
            .checked_add(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let window = data.get(offset..end).ok_or(DomainError::RangeOverflow)?;
        if let Some(&first) = index.get(window) {
            let ranges = [
                range_from_usize(
                    first,
                    first
                        .checked_add(window_size)
                        .ok_or(DomainError::RangeOverflow)?,
                )?,
                range_from_usize(offset, end)?,
            ];
            leads.push(WorkbenchLead {
                id: lead_id(WorkbenchLeadKind::ExactRepeat, &ranges, 0),
                kind: WorkbenchLeadKind::ExactRepeat,
                source_ranges: ranges.to_vec(),
                confidence: 1.0,
                evidence: WorkbenchEvidence::ExactRepeat {
                    matching_byte_count: u64::try_from(window_size)
                        .map_err(|_| DomainError::RangeOverflow)?,
                    sampled_window_step: u64::try_from(sampled_step)
                        .map_err(|_| DomainError::RangeOverflow)?,
                },
            });
        } else {
            index.insert(window.to_vec(), offset);
        }
    }
    Ok(leads)
}

fn periodicity_leads(data: &[u8], max_period: usize) -> Result<Vec<WorkbenchLead>, DomainError> {
    if data.len() < 4 {
        return Ok(Vec::new());
    }
    let block_size = PERIODICITY_BLOCK_BYTES.min(data.len());
    let base_step = (block_size / 4).max(1);
    let possible_blocks = data.len().saturating_sub(block_size) / base_step + 1;
    let sample_jump = possible_blocks.div_ceil(MAX_PERIODICITY_BLOCKS).max(1);
    let sampled_step = base_step
        .checked_mul(sample_jump)
        .ok_or(DomainError::RangeOverflow)?;
    let mut leads = Vec::new();
    for offset in (0..=data.len() - block_size).step_by(sampled_step) {
        let end = offset
            .checked_add(block_size)
            .ok_or(DomainError::RangeOverflow)?;
        let block = data.get(offset..end).ok_or(DomainError::RangeOverflow)?;
        if compact_metrics(block)?.distinct_byte_count < 4 {
            continue;
        }
        let Some((period, matching, compared)) = best_periodicity(block, max_period) else {
            continue;
        };
        let confidence = ratio_usize(matching, compared)?;
        if confidence < 0.35 {
            continue;
        }
        let range = range_from_usize(offset, end)?;
        leads.push(WorkbenchLead {
            id: lead_id(
                WorkbenchLeadKind::Periodicity,
                &[range],
                u64::try_from(period).map_err(|_| DomainError::RangeOverflow)?,
            ),
            kind: WorkbenchLeadKind::Periodicity,
            source_ranges: vec![range],
            confidence,
            evidence: WorkbenchEvidence::Periodicity {
                period_bytes: u64::try_from(period).map_err(|_| DomainError::RangeOverflow)?,
                compared_positions: u64::try_from(compared)
                    .map_err(|_| DomainError::RangeOverflow)?,
                matching_positions: u64::try_from(matching)
                    .map_err(|_| DomainError::RangeOverflow)?,
            },
        });
    }
    Ok(leads)
}

fn best_periodicity(data: &[u8], max_period: usize) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    for period in 2..=max_period.min(data.len() / 2) {
        let compared = data.len() - period;
        let matching = data
            .iter()
            .zip(&data[period..])
            .filter(|(left, right)| left == right)
            .count();
        if best.is_none_or(|(_, old_matching, old_compared)| {
            matching * old_compared > old_matching * compared
        }) {
            best = Some((period, matching, compared));
        }
    }
    best
}

fn signature_leads(data: &[u8]) -> Result<Vec<WorkbenchLead>, DomainError> {
    let signatures: &[(EmbeddedSignature, &[u8])] = &[
        (EmbeddedSignature::Png, b"\x89PNG\r\n\x1a\n"),
        (EmbeddedSignature::Elf, b"\x7fELF"),
        (EmbeddedSignature::Pdf, b"%PDF-"),
        (EmbeddedSignature::Zip, b"PK\x03\x04"),
        (EmbeddedSignature::Mz, b"MZ"),
    ];
    let mut leads = Vec::new();
    for &(signature, needle) in signatures {
        for offset in data
            .windows(needle.len())
            .enumerate()
            .filter_map(|(offset, window)| (window == needle).then_some(offset))
        {
            let range = range_from_usize(
                offset,
                offset
                    .checked_add(needle.len())
                    .ok_or(DomainError::RangeOverflow)?,
            )?;
            leads.push(WorkbenchLead {
                id: lead_id(
                    WorkbenchLeadKind::EmbeddedSignature,
                    &[range],
                    signature as u64,
                ),
                kind: WorkbenchLeadKind::EmbeddedSignature,
                source_ranges: vec![range],
                confidence: 1.0,
                evidence: WorkbenchEvidence::EmbeddedSignature { signature },
            });
        }
    }
    Ok(leads)
}

fn transform_leads(data: &[u8]) -> Result<Vec<WorkbenchLead>, DomainError> {
    let length = data.len().min(MAX_TRANSFORM_RANKING_BYTES);
    let range = range_from_usize(0, length)?;
    let mut ranked = Vec::new();
    for key in 1_u8..=u8::MAX {
        let score = text_likelihood(data.get(..length).ok_or(DomainError::RangeOverflow)?, key)?;
        ranked.push((key, score));
    }
    ranked.sort_unstable_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut leads = Vec::new();
    for (key, _) in ranked.into_iter().take(3) {
        let evaluation =
            evaluate_transform_candidate(data, range, ReversibleTransform::XorByte(key))?;
        if evaluation.assessment == TransformAssessment::Supported {
            let confidence = evaluation.after.text_likelihood;
            leads.push(WorkbenchLead {
                id: lead_id(
                    WorkbenchLeadKind::TransformCandidate,
                    &[range],
                    u64::from(key),
                ),
                kind: WorkbenchLeadKind::TransformCandidate,
                source_ranges: vec![range],
                confidence,
                evidence: WorkbenchEvidence::TransformCandidate(evaluation),
            });
        }
    }
    Ok(leads)
}

fn correlated_transform_leads(
    data: &[u8],
    window_size: usize,
    max_windows: usize,
) -> Result<Vec<WorkbenchLead>, DomainError> {
    let config = DiscoveryConfig {
        max_inspected_bytes: data.len(),
        repeated_window_size: window_size,
        max_windows: max_windows.min(MAX_DISCOVERY_WINDOWS),
        max_findings: MAX_DISCOVERY_FINDINGS,
        ..DiscoveryConfig::default()
    };
    discover_findings(data, config)?
        .into_iter()
        .filter_map(|finding| match finding.evidence {
            DiscoveryEvidence::XorCorrelatedWindow {
                key,
                sampled_window_step,
                correlated_byte_count,
                distinct_byte_count,
                ..
            } => Some((
                finding.source_ranges,
                key,
                sampled_window_step,
                correlated_byte_count,
                distinct_byte_count,
            )),
            _ => None,
        })
        .map(
            |(ranges, key, sampled_window_step, correlated_byte_count, distinct_byte_count)| {
                let Some((&source_range, rest)) = ranges.split_first() else {
                    return Err(DomainError::Internal(
                        "POC XOR correlation omitted a source range".to_owned(),
                    ));
                };
                let Some(&transformed_range) = rest.first() else {
                    return Err(DomainError::Internal(
                        "POC XOR correlation omitted a transformed range".to_owned(),
                    ));
                };
                let transform = ReversibleTransform::XorByte(key);
                Ok(WorkbenchLead {
                    id: lead_id(
                        WorkbenchLeadKind::TransformCandidate,
                        &[source_range, transformed_range],
                        u64::from(key),
                    ),
                    kind: WorkbenchLeadKind::TransformCandidate,
                    source_ranges: vec![source_range, transformed_range],
                    confidence: 1.0,
                    evidence: WorkbenchEvidence::XorCorrelatedTransform {
                        transform,
                        source_range,
                        transformed_range,
                        correlated_byte_count,
                        distinct_byte_count,
                        sampled_window_step,
                    },
                })
            },
        )
        .collect()
}

fn compact_metrics(data: &[u8]) -> Result<CompactMetrics, DomainError> {
    let byte_count = u64::try_from(data.len()).map_err(|_| DomainError::RangeOverflow)?;
    if data.is_empty() {
        return Ok(CompactMetrics {
            byte_count,
            distinct_byte_count: 0,
            entropy_bits: 0.0,
            zero_fraction: 0.0,
            text_likelihood: 0.0,
        });
    }
    let mut bins = [0_u32; 256];
    for &byte in data {
        bins[usize::from(byte)] += 1;
    }
    let count = f64::from(u32::try_from(data.len()).map_err(|_| DomainError::RangeOverflow)?);
    let entropy_bits = bins
        .iter()
        .filter(|&&n| n != 0)
        .map(|&n| {
            let p = f64::from(n) / count;
            -p * p.log2()
        })
        .sum();
    let distinct_byte_count = u16::try_from(bins.iter().filter(|&&n| n != 0).count())
        .map_err(|_| DomainError::RangeOverflow)?;
    Ok(CompactMetrics {
        byte_count,
        distinct_byte_count,
        entropy_bits,
        zero_fraction: f64::from(bins[0]) / count,
        text_likelihood: text_likelihood(data, 0)?,
    })
}

fn text_likelihood(data: &[u8], key: u8) -> Result<f64, DomainError> {
    if data.is_empty() {
        return Ok(0.0);
    }
    let score: f64 = data
        .iter()
        .map(|byte| match *byte ^ key {
            b'A'..=b'Z' | b'a'..=b'z' => 1.0,
            b' ' => 0.96,
            b'0'..=b'9' => 0.88,
            b'\t' | b'\n' | b'\r' => 0.72,
            b'.' | b',' | b':' | b';' | b'-' | b'_' | b'/' | b'\\' => 0.58,
            0x21..=0x7e => 0.28,
            _ => 0.0,
        })
        .sum();
    let count = f64::from(u32::try_from(data.len()).map_err(|_| DomainError::RangeOverflow)?);
    Ok(score / count)
}

fn ratio_usize(numerator: usize, denominator: usize) -> Result<f64, DomainError> {
    let numerator = u32::try_from(numerator).map_err(|_| DomainError::RangeOverflow)?;
    let denominator = u32::try_from(denominator).map_err(|_| DomainError::RangeOverflow)?;
    Ok(f64::from(numerator) / f64::from(denominator))
}

fn slice_for_range(data: &[u8], range: ByteRange) -> Result<&[u8], DomainError> {
    let start = usize::try_from(range.start).map_err(|_| DomainError::RangeOverflow)?;
    let end = usize::try_from(range.end).map_err(|_| DomainError::RangeOverflow)?;
    if start > end || end > data.len() {
        return Err(DomainError::InvalidRange {
            start: range.start,
            end: range.end,
        });
    }
    data.get(start..end).ok_or(DomainError::RangeOverflow)
}

fn range_from_usize(start: usize, end: usize) -> Result<ByteRange, DomainError> {
    ByteRange::new(
        u64::try_from(start).map_err(|_| DomainError::RangeOverflow)?,
        u64::try_from(end).map_err(|_| DomainError::RangeOverflow)?,
    )
}

fn lead_id(kind: WorkbenchLeadKind, ranges: &[ByteRange], parameter: u64) -> WorkbenchLeadId {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in std::iter::once(kind as u8).chain(parameter.to_le_bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    for range in ranges {
        for byte in range
            .start
            .to_le_bytes()
            .into_iter()
            .chain(range.end.to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    WorkbenchLeadId(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_has_ranked_heterogeneous_leads_with_exact_ranges() -> Result<(), DomainError> {
        let mut data = vec![0_u8; 32];
        data.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        data.extend_from_slice(&[9, 8, 7, 6, 9, 8, 7, 6, 9, 8, 7, 6]);
        let report = analyze_workbench(
            &data,
            WorkbenchConfig {
                entropy_block_size: 8,
                repeat_window_size: 8,
                max_period: 16,
                ..WorkbenchConfig::default()
            },
        )?;
        assert!(
            report
                .leads
                .iter()
                .any(|lead| lead.kind == WorkbenchLeadKind::EntropyBoundary)
        );
        assert!(
            report
                .leads
                .iter()
                .any(|lead| lead.kind == WorkbenchLeadKind::ExactRepeat)
        );
        assert!(
            report
                .leads
                .iter()
                .any(|lead| lead.kind == WorkbenchLeadKind::Periodicity)
        );
        assert!(
            report
                .leads
                .iter()
                .any(|lead| lead.kind == WorkbenchLeadKind::EmbeddedSignature)
        );
        assert!(report.leads.iter().all(|lead| {
            lead.source_ranges
                .iter()
                .all(|range| range.end <= report.inspected_range.end)
        }));
        assert_eq!(
            report,
            analyze_workbench(
                &data,
                WorkbenchConfig {
                    entropy_block_size: 8,
                    repeat_window_size: 8,
                    max_period: 16,
                    ..WorkbenchConfig::default()
                }
            )?
        );
        Ok(())
    }

    #[test]
    fn xor_transform_is_reversible_and_evidence_is_supported() -> Result<(), DomainError> {
        let key = 0xa5;
        let plain = b"STRATA WORKBENCH TRANSFORM EVIDENCE\n";
        let encoded = apply_reversible_transform(plain, ReversibleTransform::XorByte(key));
        let range = ByteRange::new(
            0,
            u64::try_from(encoded.len()).map_err(|_| DomainError::RangeOverflow)?,
        )?;
        let evaluation =
            evaluate_transform_candidate(&encoded, range, ReversibleTransform::XorByte(key))?;
        assert_eq!(evaluation.assessment, TransformAssessment::Supported);
        assert!(evaluation.text_likelihood_delta > 0.5);
        assert_eq!(
            apply_reversible_transform(&encoded, ReversibleTransform::XorByte(key)),
            plain
        );
        Ok(())
    }

    #[test]
    fn xor_transform_can_be_contradicted() -> Result<(), DomainError> {
        let plain = b"clean readable text with spaces";
        let range = ByteRange::new(
            0,
            u64::try_from(plain.len()).map_err(|_| DomainError::RangeOverflow)?,
        )?;
        let evaluation =
            evaluate_transform_candidate(plain, range, ReversibleTransform::XorByte(0xa5))?;
        assert_eq!(evaluation.assessment, TransformAssessment::Contradicted);
        Ok(())
    }

    #[test]
    fn mixed_demo_reports_all_five_lead_kinds() -> Result<(), DomainError> {
        let mut data = vec![0_u8; 64];
        let motif = [
            0x10, 0x24, 0x37, 0x49, 0x5b, 0x6d, 0x7f, 0x81, 0x93, 0xa5, 0xb7, 0xc9, 0xdb, 0xed,
            0xfe, 0x0f,
        ];
        for _ in 0..16 {
            data.extend(motif);
        }

        let mut correlated_source = Vec::with_capacity(256);
        for record in 0_u8..16 {
            correlated_source.extend([
                record,
                record.wrapping_mul(7),
                0x42,
                0x99,
                record.wrapping_add(19),
                0x31,
                0xc4,
                record.rotate_left(3),
                0x5e,
                record.wrapping_mul(11),
                0x73,
                0x08,
                record ^ 0xaa,
                0xf0,
                record.wrapping_add(3),
                0x1d,
            ]);
        }
        data.extend_from_slice(&correlated_source);
        data.extend(correlated_source.iter().map(|byte| *byte ^ 0xa7));
        data.extend([0x55_u8; 192]);
        data.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        data.extend([0x42_u8; 56]);
        data.extend([0xee_u8; 448]);
        assert_eq!(data.len(), 1_536);

        let report = analyze_workbench(
            &data,
            WorkbenchConfig {
                entropy_block_size: 64,
                repeat_window_size: 16,
                max_period: 64,
                max_leads: 64,
                ..WorkbenchConfig::default()
            },
        )?;
        for kind in [
            WorkbenchLeadKind::EntropyBoundary,
            WorkbenchLeadKind::ExactRepeat,
            WorkbenchLeadKind::Periodicity,
            WorkbenchLeadKind::EmbeddedSignature,
            WorkbenchLeadKind::TransformCandidate,
        ] {
            assert!(
                report.leads.iter().any(|lead| lead.kind == kind),
                "missing {kind:?}; found {:?}",
                report
                    .leads
                    .iter()
                    .map(|lead| lead.kind)
                    .collect::<Vec<_>>()
            );
        }
        let source_range = ByteRange::new(320, 576)?;
        let transformed_range = ByteRange::new(576, 832)?;
        assert!(report.leads.iter().any(|lead| matches!(
            lead.evidence,
            WorkbenchEvidence::XorCorrelatedTransform {
                transform: ReversibleTransform::XorByte(0xa7),
                source_range: found_source_range,
                transformed_range: found_transformed_range,
                correlated_byte_count: 256,
                ..
            } if found_source_range == source_range && found_transformed_range == transformed_range
        )));
        Ok(())
    }
}
