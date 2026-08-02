//! Bounded structural and XOR-oriented discovery over immutable bytes.

use std::collections::BTreeMap;

use strata_core::{ByteRange, DomainError};

/// Hard cap for one POC discovery pass, independent of caller-supplied settings.
pub const MAX_DISCOVERY_BYTES: usize = 4 * 1024 * 1024;

/// Hard cap for fixed windows retained by one POC discovery pass.
pub const MAX_DISCOVERY_WINDOWS: usize = 4_096;

/// Hard cap for results retained by one POC discovery pass.
pub const MAX_DISCOVERY_FINDINGS: usize = 64;

/// Minimum number of distinct values in an XOR-correlated window.
///
/// This suppresses all-zero, all-`0xff`, and similarly low-information padding
/// from becoming a deobfuscation candidate.
pub const MIN_XOR_CORRELATED_DISTINCT_BYTES: usize = 4;

/// Bounded parameters for [`discover_findings`].
///
/// The configured limits are clamped by neither the implementation nor the
/// source: invalid values are rejected so the reported evidence always states
/// the actual bounded pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscoveryConfig {
    /// Maximum contiguous source bytes inspected from offset zero.
    pub max_inspected_bytes: usize,
    /// Width of each exact byte window compared for repeats.
    pub repeated_window_size: usize,
    /// Maximum fixed windows retained while looking for repeated regions.
    pub max_windows: usize,
    /// Maximum total findings retained by the bounded pass.
    pub max_findings: usize,
    /// Minimum text-likelihood score for a non-identity XOR candidate.
    pub xor_minimum_confidence: f64,
    /// Minimum improvement over the untransformed source for an XOR candidate.
    pub xor_minimum_gain: f64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            max_inspected_bytes: 256 * 1024,
            repeated_window_size: 16,
            max_windows: 1_024,
            max_findings: 12,
            xor_minimum_confidence: 0.78,
            xor_minimum_gain: 0.20,
        }
    }
}

/// A deterministic identifier for a discovery finding.
///
/// It is a stable FNV-1a digest of the finding kind, source ranges, and
/// hypothesis parameters. It is an identity for this POC, not a cryptographic
/// content digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiscoveryFindingId(pub u64);

/// The discovery technique that produced a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscoveryKind {
    /// Two fixed source windows were byte-for-byte identical.
    RepeatedWindow,
    /// Two source windows differ by one nonzero XOR key at every byte position.
    XorCorrelatedWindow,
    /// A non-identity single-byte XOR makes the inspected source more text-like.
    SingleByteXor,
}

/// Reproducible measurements backing a [`DiscoveryFinding`].
#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryEvidence {
    /// Evidence for a pair of exactly equal byte windows.
    RepeatedWindow {
        /// Contiguous prefix inspected by the bounded scan.
        inspected_range: ByteRange,
        /// Exact source distance between compared fixed-window starts.
        sampled_window_step: u64,
        /// Number of bytes compared and found identical.
        identical_byte_count: u64,
    },
    /// Evidence for two regions related by an exact non-identity bytewise XOR.
    XorCorrelatedWindow {
        /// Explicit XOR key for `source_ranges[0][i] ^ source_ranges[1][i]`.
        key: u8,
        /// Contiguous prefix inspected by the bounded scan.
        inspected_range: ByteRange,
        /// Exact source distance between sampled fixed-window starts.
        sampled_window_step: u64,
        /// Number of source-byte pairs satisfying the stated XOR relation.
        correlated_byte_count: u64,
        /// Distinct byte values in either linked range; XOR preserves this count.
        distinct_byte_count: u16,
    },
    /// Evidence for a candidate single-byte XOR transform.
    SingleByteXor {
        /// Explicit XOR key; the POC never reports the identity key as a hypothesis.
        key: u8,
        /// Contiguous prefix inspected by the bounded scan.
        inspected_range: ByteRange,
        /// Text-likelihood score before applying the candidate key.
        baseline_text_likelihood: f64,
        /// Text-likelihood score after applying the candidate key.
        decoded_text_likelihood: f64,
    },
}

/// One ranked, source-backed result from [`discover_findings`].
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveryFinding {
    /// Stable identifier derived from the kind, ranges, and hypothesis parameters.
    pub id: DiscoveryFindingId,
    /// The deterministic discovery technique.
    pub kind: DiscoveryKind,
    /// Exact source ranges supporting this result. Ranges are half-open.
    pub source_ranges: Vec<ByteRange>,
    /// Score in the inclusive range `0.0..=1.0`; never a proof of file format.
    pub confidence: f64,
    /// Measurements and parameters sufficient to reproduce this POC result.
    pub evidence: DiscoveryEvidence,
}

/// Runs bounded repeat and single-byte-XOR discovery over an immutable source.
///
/// The pass examines only the contiguous prefix described by
/// [`DiscoveryConfig::max_inspected_bytes`]. Repeats are exact equality of
/// sampled fixed windows. XOR findings are hypotheses: a candidate is emitted
/// only when its decoded bytes improve a conservative ASCII-text likelihood by
/// the configured threshold. No finding claims that a candidate is a decoded
/// file or a confirmed format.
///
/// Results are deterministic and ordered by technique, confidence, and then
/// stable identifier. XOR-correlated windows use an index normalized by each
/// window's first byte, rather than a quadratic pair scan. Empty input produces
/// no findings.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransform`] for invalid or unbounded settings,
/// and [`DomainError::RangeOverflow`] if a source range cannot fit in `u64`.
pub fn discover_findings(
    data: &[u8],
    config: DiscoveryConfig,
) -> Result<Vec<DiscoveryFinding>, DomainError> {
    validate_discovery_config(config)?;
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let inspected_length = data.len().min(config.max_inspected_bytes);
    let inspected = data
        .get(..inspected_length)
        .ok_or(DomainError::RangeOverflow)?;
    let inspected_end = u64::try_from(inspected_length).map_err(|_| DomainError::RangeOverflow)?;
    let inspected_range = ByteRange::new(0, inspected_end)?;
    let repeated_limit = config.max_findings.div_ceil(3);
    let correlated_xor_limit = config.max_findings.div_ceil(3);
    let xor_limit = config
        .max_findings
        .saturating_sub(repeated_limit)
        .saturating_sub(correlated_xor_limit);

    let mut findings = repeated_window_findings(
        inspected,
        inspected_range,
        config.repeated_window_size,
        config.max_windows,
        repeated_limit,
    )?;
    findings.extend(xor_correlated_window_findings(
        inspected,
        inspected_range,
        config.repeated_window_size,
        config.max_windows,
        correlated_xor_limit,
    )?);
    findings.extend(single_byte_xor_findings(
        inspected,
        inspected_range,
        config.xor_minimum_confidence,
        config.xor_minimum_gain,
        xor_limit,
    )?);
    findings.sort_unstable_by(|first, second| {
        first
            .kind
            .cmp(&second.kind)
            .then_with(|| second.confidence.total_cmp(&first.confidence))
            .then_with(|| first.id.cmp(&second.id))
    });
    Ok(findings)
}

impl Ord for DiscoveryKind {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_tag().cmp(&other.as_tag())
    }
}

impl PartialOrd for DiscoveryKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl DiscoveryKind {
    const fn as_tag(self) -> u8 {
        match self {
            Self::RepeatedWindow => 1,
            Self::XorCorrelatedWindow => 2,
            Self::SingleByteXor => 3,
        }
    }
}

fn validate_discovery_config(config: DiscoveryConfig) -> Result<(), DomainError> {
    let valid_confidence = (0.0..=1.0).contains(&config.xor_minimum_confidence);
    let valid_gain = (0.0..=1.0).contains(&config.xor_minimum_gain);
    if config.max_inspected_bytes == 0
        || config.max_inspected_bytes > MAX_DISCOVERY_BYTES
        || config.repeated_window_size == 0
        || config.max_windows == 0
        || config.max_windows > MAX_DISCOVERY_WINDOWS
        || config.max_findings == 0
        || config.max_findings > MAX_DISCOVERY_FINDINGS
        || !valid_confidence
        || !valid_gain
    {
        return Err(DomainError::InvalidTransform(
            "POC discovery configuration exceeds a bounded valid range".to_owned(),
        ));
    }
    Ok(())
}

fn repeated_window_findings(
    data: &[u8],
    inspected_range: ByteRange,
    window_size: usize,
    max_windows: usize,
    finding_limit: usize,
) -> Result<Vec<DiscoveryFinding>, DomainError> {
    let available_windows = data.len() / window_size;
    if available_windows < 2 {
        return Ok(Vec::new());
    }

    let sample_window_step = available_windows.div_ceil(max_windows).max(1);
    let source_step = window_size
        .checked_mul(sample_window_step)
        .ok_or(DomainError::RangeOverflow)?;
    let mut first_offsets = BTreeMap::<Vec<u8>, usize>::new();
    let mut findings = Vec::with_capacity(finding_limit);

    for window_index in (0..available_windows).step_by(sample_window_step) {
        let offset = window_index
            .checked_mul(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let end = offset
            .checked_add(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let window = data.get(offset..end).ok_or(DomainError::RangeOverflow)?;
        if let Some(&first_offset) = first_offsets.get(window) {
            let first_range = byte_range(first_offset, window_size)?;
            let repeated_range = byte_range(offset, window_size)?;
            let length = u64::try_from(window_size).map_err(|_| DomainError::RangeOverflow)?;
            findings.push(DiscoveryFinding {
                id: stable_finding_id(
                    DiscoveryKind::RepeatedWindow,
                    &[first_range, repeated_range],
                    0,
                ),
                kind: DiscoveryKind::RepeatedWindow,
                source_ranges: vec![first_range, repeated_range],
                confidence: 1.0,
                evidence: DiscoveryEvidence::RepeatedWindow {
                    inspected_range,
                    sampled_window_step: u64::try_from(source_step)
                        .map_err(|_| DomainError::RangeOverflow)?,
                    identical_byte_count: length,
                },
            });
            if findings.len() == finding_limit {
                break;
            }
        } else {
            first_offsets.insert(window.to_vec(), offset);
        }
    }
    Ok(findings)
}

#[derive(Debug, Clone, Copy)]
struct XorWindowPair {
    first_offset: usize,
    second_offset: usize,
    length: usize,
    key: u8,
    distinct_byte_count: u16,
}

fn xor_correlated_window_findings(
    data: &[u8],
    inspected_range: ByteRange,
    window_size: usize,
    max_windows: usize,
    finding_limit: usize,
) -> Result<Vec<DiscoveryFinding>, DomainError> {
    if finding_limit == 0 {
        return Ok(Vec::new());
    }
    let available_windows = data.len() / window_size;
    if available_windows < 2 {
        return Ok(Vec::new());
    }

    let sample_window_step = available_windows.div_ceil(max_windows).max(1);
    let source_step = window_size
        .checked_mul(sample_window_step)
        .ok_or(DomainError::RangeOverflow)?;
    let mut first_offsets = BTreeMap::<Vec<u8>, (usize, u16)>::new();
    let mut pairs = Vec::with_capacity(max_windows);

    for window_index in (0..available_windows).step_by(sample_window_step) {
        let offset = window_index
            .checked_mul(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let end = offset
            .checked_add(window_size)
            .ok_or(DomainError::RangeOverflow)?;
        let window = data.get(offset..end).ok_or(DomainError::RangeOverflow)?;
        let distinct_byte_count = distinct_byte_count(window);
        if usize::from(distinct_byte_count) < MIN_XOR_CORRELATED_DISTINCT_BYTES {
            continue;
        }
        let normalized = xor_normalized_window(window)?;
        if let Some(&(first_offset, first_distinct_count)) = first_offsets.get(&normalized) {
            let key = data[first_offset] ^ window[0];
            if key != 0 {
                pairs.push(XorWindowPair {
                    first_offset,
                    second_offset: offset,
                    length: window_size,
                    key,
                    distinct_byte_count: first_distinct_count.min(distinct_byte_count),
                });
            }
        } else {
            first_offsets.insert(normalized, (offset, distinct_byte_count));
        }
    }

    let source_step = u64::try_from(source_step).map_err(|_| DomainError::RangeOverflow)?;
    merge_xor_window_pairs(pairs)
        .into_iter()
        .take(finding_limit)
        .map(|pair| {
            let first_range = byte_range(pair.first_offset, pair.length)?;
            let second_range = byte_range(pair.second_offset, pair.length)?;
            let correlated_byte_count =
                u64::try_from(pair.length).map_err(|_| DomainError::RangeOverflow)?;
            Ok(DiscoveryFinding {
                id: stable_finding_id(
                    DiscoveryKind::XorCorrelatedWindow,
                    &[first_range, second_range],
                    pair.key,
                ),
                kind: DiscoveryKind::XorCorrelatedWindow,
                source_ranges: vec![first_range, second_range],
                confidence: 1.0,
                evidence: DiscoveryEvidence::XorCorrelatedWindow {
                    key: pair.key,
                    inspected_range,
                    sampled_window_step: source_step,
                    correlated_byte_count,
                    distinct_byte_count: pair.distinct_byte_count,
                },
            })
        })
        .collect()
}

fn xor_normalized_window(window: &[u8]) -> Result<Vec<u8>, DomainError> {
    let Some((&first, rest)) = window.split_first() else {
        return Err(DomainError::InvalidTransform(
            "POC XOR correlation window must be nonempty".to_owned(),
        ));
    };
    let mut normalized = Vec::with_capacity(window.len());
    normalized.push(0);
    normalized.extend(rest.iter().map(|byte| *byte ^ first));
    Ok(normalized)
}

fn distinct_byte_count(window: &[u8]) -> u16 {
    let mut seen = [false; 256];
    let mut count = 0_u16;
    for &byte in window {
        let index = usize::from(byte);
        if !seen[index] {
            seen[index] = true;
            count += 1;
        }
    }
    count
}

fn merge_xor_window_pairs(pairs: Vec<XorWindowPair>) -> Vec<XorWindowPair> {
    let mut merged = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let Some(previous) = merged.last_mut() else {
            merged.push(pair);
            continue;
        };
        let first_end = previous.first_offset.checked_add(previous.length);
        let second_end = previous.second_offset.checked_add(previous.length);
        if previous.key == pair.key
            && first_end == Some(pair.first_offset)
            && second_end == Some(pair.second_offset)
        {
            if let Some(length) = previous.length.checked_add(pair.length) {
                previous.length = length;
                previous.distinct_byte_count =
                    previous.distinct_byte_count.max(pair.distinct_byte_count);
                continue;
            }
        }
        merged.push(pair);
    }
    merged
}

fn single_byte_xor_findings(
    data: &[u8],
    inspected_range: ByteRange,
    minimum_confidence: f64,
    minimum_gain: f64,
    finding_limit: usize,
) -> Result<Vec<DiscoveryFinding>, DomainError> {
    let baseline = text_likelihood(data, 0);
    let mut candidates = Vec::new();
    for key in 1_u8..=u8::MAX {
        let decoded = text_likelihood(data, key);
        let gain = decoded - baseline;
        if decoded >= minimum_confidence && gain >= minimum_gain {
            candidates.push((key, decoded));
        }
    }
    candidates.sort_unstable_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| first.0.cmp(&second.0))
    });

    candidates
        .into_iter()
        .take(finding_limit)
        .map(|(key, confidence)| {
            Ok(DiscoveryFinding {
                id: stable_finding_id(DiscoveryKind::SingleByteXor, &[inspected_range], key),
                kind: DiscoveryKind::SingleByteXor,
                source_ranges: vec![inspected_range],
                confidence,
                evidence: DiscoveryEvidence::SingleByteXor {
                    key,
                    inspected_range,
                    baseline_text_likelihood: baseline,
                    decoded_text_likelihood: confidence,
                },
            })
        })
        .collect()
}

fn byte_range(offset: usize, length: usize) -> Result<ByteRange, DomainError> {
    let end = offset
        .checked_add(length)
        .ok_or(DomainError::RangeOverflow)?;
    let start = u64::try_from(offset).map_err(|_| DomainError::RangeOverflow)?;
    let end = u64::try_from(end).map_err(|_| DomainError::RangeOverflow)?;
    ByteRange::new(start, end)
}

#[allow(clippy::cast_precision_loss)]
fn text_likelihood(data: &[u8], key: u8) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let score: f64 = data
        .iter()
        .map(|byte| match *byte ^ key {
            b'A'..=b'Z' | b'a'..=b'z' => 1.0,
            b' ' => 0.96,
            b'0'..=b'9' => 0.88,
            b'\t' | b'\n' | b'\r' => 0.72,
            b'.' | b',' | b':' | b';' | b'-' | b'_' | b'/' | b'\\' | b'(' | b')' => 0.58,
            0x21..=0x7e => 0.28,
            _ => 0.0,
        })
        .sum();
    score / data.len() as f64
}

fn stable_finding_id(
    kind: DiscoveryKind,
    ranges: &[ByteRange],
    parameter: u8,
) -> DiscoveryFindingId {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = fnv1a_step(hash, kind.as_tag());
    hash = fnv1a_step(hash, parameter);
    for range in ranges {
        for byte in range.start.to_le_bytes() {
            hash = fnv1a_step(hash, byte);
        }
        for byte in range.end.to_le_bytes() {
            hash = fnv1a_step(hash, byte);
        }
    }
    DiscoveryFindingId(hash)
}

fn fnv1a_step(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
}
