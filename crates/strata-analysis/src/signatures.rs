//! Strict, bounded ingestion and matching for external signature knowledge packs.
//!
//! The initial adapter consumes the UFSC `0.1.x` JSON envelope produced by
//! an external UFSC producer. Catalog metadata remains candidate evidence: a byte
//! pattern match never becomes a parser or file-type verdict by itself.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use strata_core::{ByteRange, DomainError};

/// Maximum accepted serialized signature pack size.
pub const MAX_SIGNATURE_PACK_BYTES: usize = 16 * 1024 * 1024;
/// Maximum records accepted from one external pack.
pub const MAX_SIGNATURE_RECORDS: usize = 20_000;
/// Maximum bytes in one normalized signature pattern.
pub const MAX_SIGNATURE_PATTERN_BYTES: usize = 256;
/// Maximum prefix searched for relaxed embedded occurrences.
pub const MAX_SIGNATURE_SCAN_BYTES: usize = 16 * 1024 * 1024;
/// Maximum matches returned by one signature scan.
pub const MAX_SIGNATURE_MATCHES: usize = 512;
const MAX_RAW_SIGNATURE_MATCHES: usize = 4_096;
const MAX_METADATA_VALUES: usize = 24;
const MAX_METADATA_BYTES: usize = 512;

/// Why a catalog pattern was tested at a particular source location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignatureMatchMode {
    /// The match obeys the catalog's declared beginning, fixed, or end offset.
    DeclaredOffset,
    /// A beginning-of-file signature was found elsewhere by bounded search.
    EmbeddedSearch,
}

/// Supported normalized offset policy from an external catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignatureOffsetPolicy {
    /// Pattern begins at source offset zero.
    Beginning,
    /// Pattern begins at this absolute source byte offset.
    Fixed(u64),
    /// Pattern ends this many bytes before the end of the source.
    End(u64),
}

/// Source attribution retained from the external signature database.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignatureSource {
    /// Human-readable source name.
    pub name: String,
    /// Source URL as supplied by the catalog.
    pub url: String,
    /// Retrieval timestamp as supplied by the catalog.
    pub retrieved_at: String,
}

/// One possible interpretation attached to a matched byte pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureCandidate {
    /// Stable catalog record identifier.
    pub id: String,
    /// Best available human-readable label.
    pub label: String,
    /// Catalog categories, normalized to lowercase.
    pub categories: Vec<String>,
    /// Associated file extensions without interpretation.
    pub extensions: Vec<String>,
    /// Associated MIME types without interpretation.
    pub mime_types: Vec<String>,
    /// Strength string claimed by the source catalog; not used as confidence.
    pub declared_strength: String,
    /// Exact source records supporting this catalog candidate.
    pub sources: Vec<SignatureSource>,
}

/// Reproducible evidence attached to one exact source match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureMatchEvidence {
    /// Producer name from the source pack envelope.
    pub catalog_name: String,
    /// Producer schema/data version from the source pack envelope.
    pub catalog_version: String,
    /// SHA-256 digest of the exact imported JSON bytes.
    pub catalog_digest: String,
    /// Canonical pattern with `??` for wildcard bytes.
    pub pattern_hex: String,
    /// Number of exact bytes supporting the match.
    pub exact_byte_count: u16,
    /// Distinct exact byte values contributing information to the pattern.
    pub distinct_exact_byte_count: u16,
    /// Number of wildcard bytes ignored during comparison.
    pub wildcard_byte_count: u16,
    /// Whether the match used the declared offset or a relaxed embedded search.
    pub mode: SignatureMatchMode,
    /// Offset policies represented by the matching candidates.
    pub offset_policies: Vec<SignatureOffsetPolicy>,
    /// Possible catalog interpretations sharing the exact matched pattern.
    pub candidates: Vec<SignatureCandidate>,
}

/// One exact catalog-backed source match.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureMatch {
    /// Stable deterministic identity derived from pack, pattern, mode, and range.
    pub id: u64,
    /// Exact half-open source range occupied by the matched pattern.
    pub source_range: ByteRange,
    /// Derived evidence score in `0.0..=1.0`; never a format verdict.
    pub confidence: f64,
    /// Catalog and byte-pattern evidence explaining the score.
    pub evidence: SignatureMatchEvidence,
}

/// Explicit resource and discovery bounds for one scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureScanConfig {
    /// Prefix length searched for embedded occurrences.
    pub max_scan_bytes: usize,
    /// Maximum retained matches after deterministic ranking.
    pub max_matches: usize,
    /// Whether beginning-of-file patterns may be searched elsewhere.
    pub include_embedded: bool,
    /// Minimum exact bytes required for relaxed embedded matching.
    pub min_embedded_exact_bytes: usize,
}

impl Default for SignatureScanConfig {
    fn default() -> Self {
        Self {
            max_scan_bytes: 1024 * 1024,
            max_matches: 64,
            include_embedded: true,
            min_embedded_exact_bytes: 4,
        }
    }
}

/// Bounded result of matching a catalog against immutable source bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureScanReport {
    /// Prefix searched for relaxed embedded occurrences.
    pub embedded_inspected_range: ByteRange,
    /// Ranked exact source matches.
    pub matches: Vec<SignatureMatch>,
    /// Whether raw or ranked results exceeded an explicit bound.
    pub truncated: bool,
}

/// Counts explaining which external records became executable rules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureImportStats {
    /// Records present in the deserialized envelope.
    pub input_records: usize,
    /// Records accepted as bounded matching rules.
    pub accepted_rules: usize,
    /// Records skipped because they had no pattern.
    pub skipped_missing_pattern: usize,
    /// Records skipped because multiple patterns had ambiguous semantics.
    pub skipped_multiple_patterns: usize,
    /// Records skipped because the byte pattern was invalid or too weak.
    pub skipped_invalid_pattern: usize,
    /// Records skipped because their offset/container policy is unsupported.
    pub skipped_unsupported_offset: usize,
}

impl SignatureImportStats {
    /// Total records deliberately excluded by the strict adapter.
    #[must_use]
    pub const fn skipped_records(self) -> usize {
        self.skipped_missing_pattern
            + self.skipped_multiple_patterns
            + self.skipped_invalid_pattern
            + self.skipped_unsupported_offset
    }
}

/// A compiled, immutable external signature catalog.
#[derive(Debug, Clone)]
pub struct SignatureCatalog {
    name: String,
    version: String,
    generated_at: Option<String>,
    digest: String,
    stats: SignatureImportStats,
    rules: Vec<SignatureRule>,
    embedded_index: Vec<Vec<AnchorCandidate>>,
}

impl SignatureCatalog {
    /// Imports a strict subset of a UFSC `0.1.x` JSON envelope.
    ///
    /// Unsupported container paths and variable offsets are counted and
    /// skipped. Unknown JSON fields are ignored so source-specific raw records
    /// are never retained in analysis memory.
    ///
    /// # Errors
    ///
    /// Returns [`SignatureCatalogError`] when the pack exceeds a hard bound,
    /// is not valid JSON, or omits required envelope identity.
    pub fn from_ufsc_json(bytes: &[u8]) -> Result<Self, SignatureCatalogError> {
        if bytes.len() > MAX_SIGNATURE_PACK_BYTES {
            return Err(SignatureCatalogError::ResourceLimit(format!(
                "signature pack is {} bytes; maximum is {MAX_SIGNATURE_PACK_BYTES}",
                bytes.len()
            )));
        }
        let envelope: UfscEnvelope = serde_json::from_slice(bytes)
            .map_err(|error| SignatureCatalogError::InvalidJson(error.to_string()))?;
        validate_envelope_identity(&envelope)?;
        if envelope.signatures.len() > MAX_SIGNATURE_RECORDS {
            return Err(SignatureCatalogError::ResourceLimit(format!(
                "signature pack has {} records; maximum is {MAX_SIGNATURE_RECORDS}",
                envelope.signatures.len()
            )));
        }

        let digest = format!("{:x}", Sha256::digest(bytes));
        let mut stats = SignatureImportStats {
            input_records: envelope.signatures.len(),
            ..SignatureImportStats::default()
        };
        let mut rules = Vec::with_capacity(envelope.signatures.len());
        for record in envelope.signatures {
            match signature_rule(record) {
                RuleOutcome::Accepted(rule) => {
                    stats.accepted_rules = stats.accepted_rules.saturating_add(1);
                    rules.push(*rule);
                }
                RuleOutcome::MissingPattern => {
                    stats.skipped_missing_pattern = stats.skipped_missing_pattern.saturating_add(1);
                }
                RuleOutcome::MultiplePatterns => {
                    stats.skipped_multiple_patterns =
                        stats.skipped_multiple_patterns.saturating_add(1);
                }
                RuleOutcome::InvalidPattern => {
                    stats.skipped_invalid_pattern = stats.skipped_invalid_pattern.saturating_add(1);
                }
                RuleOutcome::UnsupportedOffset => {
                    stats.skipped_unsupported_offset =
                        stats.skipped_unsupported_offset.saturating_add(1);
                }
            }
        }
        rules.sort_by(|left, right| {
            left.candidate
                .id
                .cmp(&right.candidate.id)
                .then_with(|| left.pattern.canonical.cmp(&right.pattern.canonical))
        });
        let embedded_index = build_embedded_index(&rules);
        Ok(Self {
            name: envelope.project_name,
            version: envelope.version,
            generated_at: envelope.generated_at,
            digest,
            stats,
            rules,
            embedded_index,
        })
    }

    /// Producer name declared by the pack.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Producer version declared by the pack.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Optional generation timestamp declared by the pack.
    #[must_use]
    pub fn generated_at(&self) -> Option<&str> {
        self.generated_at.as_deref()
    }

    /// SHA-256 digest of the exact imported pack bytes.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Import acceptance and rejection counts.
    #[must_use]
    pub const fn stats(&self) -> SignatureImportStats {
        self.stats
    }

    /// Rules eligible for offset-relaxed embedded search after safety filters.
    #[must_use]
    pub fn embedded_rule_count(&self) -> usize {
        self.embedded_index.iter().map(Vec::len).sum()
    }

    /// Matches supported rules against immutable source bytes.
    ///
    /// Declared fixed/end offsets may be checked anywhere in the supplied
    /// source. Relaxed embedded discovery is restricted to the reported prefix.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] for invalid bounds or unrepresentable ranges.
    pub fn scan(
        &self,
        data: &[u8],
        config: SignatureScanConfig,
    ) -> Result<SignatureScanReport, DomainError> {
        validate_scan_config(config)?;
        let scan_length = data.len().min(config.max_scan_bytes);
        let inspected_end = u64::try_from(scan_length).map_err(|_| DomainError::RangeOverflow)?;
        let embedded_inspected_range = ByteRange::new(0, inspected_end)?;
        if data.is_empty() || self.rules.is_empty() {
            return Ok(SignatureScanReport {
                embedded_inspected_range,
                matches: Vec::new(),
                truncated: false,
            });
        }

        let mut raw = Vec::new();
        let mut truncated = false;
        for (rule_index, rule) in self.rules.iter().enumerate() {
            let Some(start) = declared_start(rule, data.len()) else {
                continue;
            };
            if pattern_matches(data, start, &rule.pattern) {
                if raw.len() == MAX_RAW_SIGNATURE_MATCHES {
                    truncated = true;
                    break;
                }
                raw.push(RawMatch {
                    rule_index,
                    start,
                    mode: SignatureMatchMode::DeclaredOffset,
                });
            }
        }

        if config.include_embedded && !truncated {
            'scan: for position in 0..scan_length {
                let Some(&byte) = data.get(position) else {
                    return Err(DomainError::RangeOverflow);
                };
                for anchor in &self.embedded_index[usize::from(byte)] {
                    let rule = self.rules.get(anchor.rule_index).ok_or_else(|| {
                        DomainError::Internal("invalid signature index".to_owned())
                    })?;
                    if rule.pattern.exact_count < config.min_embedded_exact_bytes {
                        continue;
                    }
                    let Some(start) = position.checked_sub(anchor.pattern_offset) else {
                        continue;
                    };
                    if start == 0 || !pattern_fits_prefix(start, &rule.pattern, scan_length) {
                        continue;
                    }
                    if pattern_matches(data, start, &rule.pattern) {
                        if raw.len() == MAX_RAW_SIGNATURE_MATCHES {
                            truncated = true;
                            break 'scan;
                        }
                        raw.push(RawMatch {
                            rule_index: anchor.rule_index,
                            start,
                            mode: SignatureMatchMode::EmbeddedSearch,
                        });
                    }
                }
            }
        }

        let mut matches = aggregate_matches(self, raw)?;
        matches.sort_by(|left, right| {
            right
                .confidence
                .total_cmp(&left.confidence)
                .then_with(|| left.source_range.start.cmp(&right.source_range.start))
                .then_with(|| left.id.cmp(&right.id))
        });
        if matches.len() > config.max_matches {
            matches.truncate(config.max_matches);
            truncated = true;
        }
        Ok(SignatureScanReport {
            embedded_inspected_range,
            matches,
            truncated,
        })
    }
}

/// Error returned while importing an external signature catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureCatalogError {
    /// Serialized input exceeded a declared bound.
    ResourceLimit(String),
    /// Serialized input was not valid JSON for the UFSC adapter.
    InvalidJson(String),
    /// Required producer identity was absent or unsupported.
    InvalidEnvelope(String),
}

impl core::fmt::Display for SignatureCatalogError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ResourceLimit(message)
            | Self::InvalidJson(message)
            | Self::InvalidEnvelope(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SignatureCatalogError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PatternByte {
    Exact(u8),
    Wildcard,
}

#[derive(Debug, Clone)]
struct SignaturePattern {
    bytes: Vec<PatternByte>,
    canonical: String,
    exact_count: usize,
    distinct_exact_count: usize,
}

#[derive(Debug, Clone)]
struct SignatureRule {
    pattern: SignaturePattern,
    offset: SignatureOffsetPolicy,
    candidate: SignatureCandidate,
}

#[derive(Debug, Clone, Copy)]
struct AnchorCandidate {
    rule_index: usize,
    pattern_offset: usize,
}

#[derive(Debug, Clone, Copy)]
struct RawMatch {
    rule_index: usize,
    start: usize,
    mode: SignatureMatchMode,
}

#[derive(Debug, Deserialize)]
struct UfscEnvelope {
    #[serde(default)]
    project_name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    generated_at: Option<String>,
    #[serde(default)]
    signatures: Vec<UfscSignature>,
}

#[derive(Debug, Deserialize)]
struct UfscSignature {
    #[serde(rename = "_id", default)]
    id: String,
    #[serde(default)]
    hex_signatures: Vec<String>,
    #[serde(default)]
    offsets: Vec<UfscOffset>,
    #[serde(default)]
    signature_strength: String,
    #[serde(default)]
    common_names: Vec<String>,
    #[serde(default)]
    file_extensions: Vec<String>,
    #[serde(default)]
    mime_types: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    description_human: Option<String>,
    #[serde(default)]
    sources: Vec<UfscSource>,
}

#[derive(Debug, Deserialize)]
struct UfscOffset {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    value_numeric: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UfscSource {
    #[serde(default)]
    source_name: String,
    #[serde(default)]
    source_url: String,
    #[serde(default)]
    retrieved_at: String,
}

enum RuleOutcome {
    Accepted(Box<SignatureRule>),
    MissingPattern,
    MultiplePatterns,
    InvalidPattern,
    UnsupportedOffset,
}

fn validate_envelope_identity(envelope: &UfscEnvelope) -> Result<(), SignatureCatalogError> {
    if envelope.project_name.trim().is_empty() || envelope.project_name.len() > MAX_METADATA_BYTES {
        return Err(SignatureCatalogError::InvalidEnvelope(
            "signature pack requires a bounded project_name".to_owned(),
        ));
    }
    if envelope.version.trim().is_empty() || envelope.version.len() > MAX_METADATA_BYTES {
        return Err(SignatureCatalogError::InvalidEnvelope(
            "signature pack requires a bounded version".to_owned(),
        ));
    }
    if !envelope.version.starts_with("0.1.") && envelope.version != "0.1" {
        return Err(SignatureCatalogError::InvalidEnvelope(format!(
            "unsupported UFSC signature pack version {}",
            envelope.version
        )));
    }
    Ok(())
}

fn signature_rule(record: UfscSignature) -> RuleOutcome {
    if record.hex_signatures.is_empty() {
        return RuleOutcome::MissingPattern;
    }
    if record.hex_signatures.len() != 1 {
        return RuleOutcome::MultiplePatterns;
    }
    let Some(raw_pattern) = record.hex_signatures.first() else {
        return RuleOutcome::MissingPattern;
    };
    let Some(pattern) = parse_pattern(raw_pattern) else {
        return RuleOutcome::InvalidPattern;
    };
    if record.offsets.len() != 1 {
        return RuleOutcome::UnsupportedOffset;
    }
    let Some(offset) = record.offsets.first().and_then(parse_offset_policy) else {
        return RuleOutcome::UnsupportedOffset;
    };
    if record.id.trim().is_empty() || record.id.len() > MAX_METADATA_BYTES {
        return RuleOutcome::InvalidPattern;
    }
    let label = record
        .common_names
        .iter()
        .find(|name| !name.trim().is_empty())
        .cloned()
        .or_else(|| {
            record
                .description_human
                .filter(|label| !label.trim().is_empty())
        })
        .unwrap_or_else(|| "Unnamed catalog signature".to_owned());
    let candidate = SignatureCandidate {
        id: bounded_text(&record.id),
        label: bounded_text(&label),
        categories: bounded_values(record.categories, true),
        extensions: bounded_values(record.file_extensions, true),
        mime_types: bounded_values(record.mime_types, true),
        declared_strength: bounded_text(&record.signature_strength),
        sources: bounded_sources(record.sources),
    };
    RuleOutcome::Accepted(Box::new(SignatureRule {
        pattern,
        offset,
        candidate,
    }))
}

fn parse_pattern(raw: &str) -> Option<SignaturePattern> {
    let tokens = raw.split_whitespace().collect::<Vec<_>>();
    let tokens = if tokens.len() == 1 && !raw.contains('?') && raw.len() % 2 == 0 && raw.len() > 2 {
        raw.as_bytes()
            .chunks_exact(2)
            .map(std::str::from_utf8)
            .collect::<Result<Vec<_>, _>>()
            .ok()?
    } else {
        tokens
    };
    if tokens.is_empty() || tokens.len() > MAX_SIGNATURE_PATTERN_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(tokens.len());
    let mut exact_count = 0_usize;
    let mut distinct_exact = BTreeSet::new();
    for token in tokens {
        if token == "??" {
            bytes.push(PatternByte::Wildcard);
        } else if token.len() == 2 {
            let value = u8::from_str_radix(token, 16).ok()?;
            bytes.push(PatternByte::Exact(value));
            exact_count = exact_count.saturating_add(1);
            distinct_exact.insert(value);
        } else {
            return None;
        }
    }
    if exact_count < 2 {
        return None;
    }
    let canonical = bytes
        .iter()
        .map(|byte| match byte {
            PatternByte::Exact(value) => format!("{value:02X}"),
            PatternByte::Wildcard => "??".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(SignaturePattern {
        bytes,
        canonical,
        exact_count,
        distinct_exact_count: distinct_exact.len(),
    })
}

fn parse_offset_policy(offset: &UfscOffset) -> Option<SignatureOffsetPolicy> {
    match offset.kind.trim().to_ascii_lowercase().as_str() {
        "bof" => Some(SignatureOffsetPolicy::Beginning),
        "fixed" => nonnegative_offset(offset.value_numeric).map(SignatureOffsetPolicy::Fixed),
        "eof" | "footer" => {
            nonnegative_offset(offset.value_numeric.or(Some(0))).map(SignatureOffsetPolicy::End)
        }
        _ => None,
    }
}

fn nonnegative_offset(value: Option<i64>) -> Option<u64> {
    u64::try_from(value?).ok()
}

fn bounded_text(value: &str) -> String {
    value.chars().take(MAX_METADATA_BYTES).collect()
}

fn bounded_values(values: Vec<String>, lowercase: bool) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let value = bounded_text(value.trim());
            if value.is_empty() {
                return None;
            }
            let value = if lowercase {
                value.to_ascii_lowercase()
            } else {
                value
            };
            seen.insert(value.clone()).then_some(value)
        })
        .take(MAX_METADATA_VALUES)
        .collect()
}

fn bounded_sources(sources: Vec<UfscSource>) -> Vec<SignatureSource> {
    let mut seen = BTreeSet::new();
    sources
        .into_iter()
        .filter_map(|source| {
            let source = SignatureSource {
                name: bounded_text(source.source_name.trim()),
                url: bounded_text(source.source_url.trim()),
                retrieved_at: bounded_text(source.retrieved_at.trim()),
            };
            (!source.name.is_empty() && seen.insert(source.clone())).then_some(source)
        })
        .take(MAX_METADATA_VALUES)
        .collect()
}

fn build_embedded_index(rules: &[SignatureRule]) -> Vec<Vec<AnchorCandidate>> {
    let mut index = (0..256).map(|_| Vec::new()).collect::<Vec<_>>();
    let eligible = rules
        .iter()
        .enumerate()
        .filter_map(|(rule_index, rule)| {
            (rule.offset == SignatureOffsetPolicy::Beginning && embedded_rule_is_specific(rule, 4))
                .then_some(rule_index)
        })
        .collect::<Vec<_>>();
    let mut anchor_frequency = [0_usize; 256];
    for &rule_index in &eligible {
        let Some(rule) = rules.get(rule_index) else {
            continue;
        };
        let values = rule
            .pattern
            .bytes
            .iter()
            .filter_map(|byte| match byte {
                PatternByte::Exact(value) => Some(*value),
                PatternByte::Wildcard => None,
            })
            .collect::<BTreeSet<_>>();
        for value in values {
            anchor_frequency[usize::from(value)] =
                anchor_frequency[usize::from(value)].saturating_add(1);
        }
    }
    for rule_index in eligible {
        let Some(rule) = rules.get(rule_index) else {
            continue;
        };
        let anchor = rule
            .pattern
            .bytes
            .iter()
            .enumerate()
            .filter_map(|(pattern_offset, byte)| match byte {
                PatternByte::Exact(value) => Some((*value, pattern_offset)),
                PatternByte::Wildcard => None,
            })
            .min_by_key(|(value, pattern_offset)| {
                (anchor_frequency[usize::from(*value)], *pattern_offset)
            });
        if let Some((value, pattern_offset)) = anchor {
            index[usize::from(value)].push(AnchorCandidate {
                rule_index,
                pattern_offset,
            });
        }
    }
    index
}

fn validate_scan_config(config: SignatureScanConfig) -> Result<(), DomainError> {
    if config.max_scan_bytes == 0
        || config.max_scan_bytes > MAX_SIGNATURE_SCAN_BYTES
        || config.max_matches == 0
        || config.max_matches > MAX_SIGNATURE_MATCHES
        || config.min_embedded_exact_bytes < 4
        || config.min_embedded_exact_bytes > MAX_SIGNATURE_PATTERN_BYTES
    {
        return Err(DomainError::InvalidView(
            "invalid bounded signature scan configuration".to_owned(),
        ));
    }
    Ok(())
}

fn declared_start(rule: &SignatureRule, source_length: usize) -> Option<usize> {
    match rule.offset {
        SignatureOffsetPolicy::Beginning => Some(0),
        SignatureOffsetPolicy::Fixed(offset) => usize::try_from(offset).ok(),
        SignatureOffsetPolicy::End(distance) => {
            let distance = usize::try_from(distance).ok()?;
            source_length
                .checked_sub(distance)?
                .checked_sub(rule.pattern.bytes.len())
        }
    }
}

fn pattern_fits_prefix(start: usize, pattern: &SignaturePattern, limit: usize) -> bool {
    start
        .checked_add(pattern.bytes.len())
        .is_some_and(|end| end <= limit)
}

fn pattern_matches(data: &[u8], start: usize, pattern: &SignaturePattern) -> bool {
    let Some(end) = start.checked_add(pattern.bytes.len()) else {
        return false;
    };
    let Some(window) = data.get(start..end) else {
        return false;
    };
    window
        .iter()
        .zip(&pattern.bytes)
        .all(|(actual, expected)| match expected {
            PatternByte::Exact(value) => actual == value,
            PatternByte::Wildcard => true,
        })
}

fn embedded_rule_is_specific(rule: &SignatureRule, minimum_exact: usize) -> bool {
    let pattern = &rule.pattern;
    if pattern.exact_count < minimum_exact || pattern.distinct_exact_count < 3 {
        return false;
    }
    let mut frequencies = [0_usize; 256];
    for byte in &pattern.bytes {
        if let PatternByte::Exact(value) = byte {
            frequencies[usize::from(*value)] = frequencies[usize::from(*value)].saturating_add(1);
        }
    }
    let dominant = frequencies.into_iter().max().unwrap_or(0);
    if dominant.saturating_mul(4) >= pattern.exact_count.saturating_mul(3) {
        return false;
    }
    let all_printable = pattern.bytes.iter().all(|byte| match byte {
        PatternByte::Exact(value) => (0x20..=0x7e).contains(value),
        PatternByte::Wildcard => false,
    });
    let source_count = rule.candidate.sources.len();
    let has_structural_category = rule
        .candidate
        .categories
        .iter()
        .any(|category| !matches!(category.as_str(), "unknown" | "config" | "data"));
    if source_count < 2 && !has_structural_category {
        return false;
    }
    if pattern.exact_count < 6 && source_count < 2 {
        return false;
    }
    let first_exact = pattern.bytes.iter().find_map(|byte| match byte {
        PatternByte::Exact(value) => Some(*value),
        PatternByte::Wildcard => None,
    });
    if all_printable && ((pattern.exact_count < 6 && first_exact != Some(b'%')) || source_count < 2)
    {
        return false;
    }
    true
}

fn aggregate_matches(
    catalog: &SignatureCatalog,
    raw: Vec<RawMatch>,
) -> Result<Vec<SignatureMatch>, DomainError> {
    let mut grouped = BTreeMap::<(usize, usize, String, SignatureMatchMode), Vec<usize>>::new();
    for occurrence in raw {
        let rule = catalog
            .rules
            .get(occurrence.rule_index)
            .ok_or_else(|| DomainError::Internal("invalid signature rule".to_owned()))?;
        let end = occurrence
            .start
            .checked_add(rule.pattern.bytes.len())
            .ok_or(DomainError::RangeOverflow)?;
        grouped
            .entry((
                occurrence.start,
                end,
                rule.pattern.canonical.clone(),
                occurrence.mode,
            ))
            .or_default()
            .push(occurrence.rule_index);
    }

    grouped
        .into_iter()
        .map(|((start, end, pattern_hex, mode), rule_indices)| {
            let first_index = *rule_indices
                .first()
                .ok_or_else(|| DomainError::Internal("empty signature group".to_owned()))?;
            let first = catalog
                .rules
                .get(first_index)
                .ok_or_else(|| DomainError::Internal("invalid signature group".to_owned()))?;
            let mut candidates = rule_indices
                .iter()
                .filter_map(|index| catalog.rules.get(*index))
                .map(|rule| rule.candidate.clone())
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| left.id.cmp(&right.id));
            candidates.dedup_by(|left, right| left.id == right.id);
            let mut policies = rule_indices
                .iter()
                .filter_map(|index| catalog.rules.get(*index))
                .map(|rule| rule.offset)
                .collect::<Vec<_>>();
            policies.sort_unstable();
            policies.dedup();
            let range = ByteRange::new(
                u64::try_from(start).map_err(|_| DomainError::RangeOverflow)?,
                u64::try_from(end).map_err(|_| DomainError::RangeOverflow)?,
            )?;
            let exact_byte_count =
                u16::try_from(first.pattern.exact_count).map_err(|_| DomainError::RangeOverflow)?;
            let distinct_exact_byte_count = u16::try_from(first.pattern.distinct_exact_count)
                .map_err(|_| DomainError::RangeOverflow)?;
            let wildcard_count = first
                .pattern
                .bytes
                .len()
                .saturating_sub(first.pattern.exact_count);
            let wildcard_byte_count =
                u16::try_from(wildcard_count).map_err(|_| DomainError::RangeOverflow)?;
            let confidence = signature_confidence(
                first.pattern.exact_count,
                first.pattern.distinct_exact_count,
                wildcard_count,
                mode,
                &candidates,
            );
            let id = signature_match_id(&catalog.digest, &pattern_hex, range, mode);
            Ok(SignatureMatch {
                id,
                source_range: range,
                confidence,
                evidence: SignatureMatchEvidence {
                    catalog_name: catalog.name.clone(),
                    catalog_version: catalog.version.clone(),
                    catalog_digest: catalog.digest.clone(),
                    pattern_hex,
                    exact_byte_count,
                    distinct_exact_byte_count,
                    wildcard_byte_count,
                    mode,
                    offset_policies: policies,
                    candidates,
                },
            })
        })
        .collect()
}

fn signature_confidence(
    exact_count: usize,
    distinct_exact_count: usize,
    wildcard_count: usize,
    mode: SignatureMatchMode,
    candidates: &[SignatureCandidate],
) -> f64 {
    let exact_count = u16::try_from(exact_count).unwrap_or(u16::MAX);
    let distinct_exact_count = u16::try_from(distinct_exact_count).unwrap_or(u16::MAX);
    let wildcard_count = u16::try_from(wildcard_count).unwrap_or(u16::MAX);
    let specificity = (f64::from(exact_count) / 8.0).clamp(0.0, 1.0);
    let diversity = (f64::from(distinct_exact_count) / 4.0).clamp(0.0, 1.0);
    let offset_support = if mode == SignatureMatchMode::DeclaredOffset {
        0.15
    } else {
        0.0
    };
    let source_count = candidates
        .iter()
        .flat_map(|candidate| &candidate.sources)
        .collect::<BTreeSet<_>>()
        .len();
    let corroborating_sources = u8::try_from(source_count.saturating_sub(1).min(3)).unwrap_or(3);
    let corroboration = f64::from(corroborating_sources) * 0.03;
    let pattern_length = exact_count.saturating_add(wildcard_count).max(1);
    let wildcard_penalty = f64::from(wildcard_count) / f64::from(pattern_length) * 0.16;
    (0.12 + specificity * 0.35 + diversity * 0.25 + offset_support + corroboration
        - wildcard_penalty)
        .clamp(0.0, 1.0)
}

fn signature_match_id(
    catalog_digest: &str,
    pattern_hex: &str,
    range: ByteRange,
    mode: SignatureMatchMode,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mode = match mode {
        SignatureMatchMode::DeclaredOffset => 0_u8,
        SignatureMatchMode::EmbeddedSearch => 1_u8,
    };
    for byte in catalog_digest
        .bytes()
        .chain(pattern_hex.bytes())
        .chain(range.start.to_le_bytes())
        .chain(range.end.to_le_bytes())
        .chain(std::iter::once(mode))
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACK: &str = include_str!("../../../fixtures/signatures/ufsc-minimal-v0.1.json");

    #[test]
    fn strict_import_reports_every_excluded_record() -> Result<(), Box<dyn std::error::Error>> {
        let catalog = SignatureCatalog::from_ufsc_json(PACK.as_bytes())?;
        let stats = catalog.stats();
        assert_eq!(stats.input_records, 5);
        assert_eq!(stats.accepted_rules, 2);
        assert_eq!(stats.skipped_missing_pattern, 1);
        assert_eq!(stats.skipped_multiple_patterns, 1);
        assert_eq!(stats.skipped_unsupported_offset, 1);
        assert_eq!(stats.skipped_records(), 3);
        assert_eq!(catalog.name(), "UniversalFileSignatureCompendium");
        assert_eq!(catalog.version(), "0.1.0");
        Ok(())
    }

    #[test]
    fn scan_distinguishes_declared_and_embedded_matches() -> Result<(), Box<dyn std::error::Error>>
    {
        let catalog = SignatureCatalog::from_ufsc_json(PACK.as_bytes())?;
        let png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        let mut data = png.to_vec();
        data.extend_from_slice(&png);
        data.extend_from_slice(&[0x52, 0x49, 0x46, 0x46, 0x7f, 0x57, 0x41, 0x56, 0x45]);
        let report = catalog.scan(&data, SignatureScanConfig::default())?;
        assert_eq!(report.matches.len(), 3);
        assert!(report.matches.iter().any(|matched| {
            matched.source_range.start == 0
                && matched.evidence.mode == SignatureMatchMode::DeclaredOffset
        }));
        assert!(report.matches.iter().any(|matched| {
            matched.source_range.start == 8
                && matched.evidence.mode == SignatureMatchMode::EmbeddedSearch
        }));
        assert!(report.matches.iter().any(|matched| {
            matched.source_range.start == 16 && matched.evidence.wildcard_byte_count == 1
        }));
        Ok(())
    }

    #[test]
    fn scan_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let catalog = SignatureCatalog::from_ufsc_json(PACK.as_bytes())?;
        let data = b"----\x89PNG\r\n\x1a\n----\x89PNG\r\n\x1a\n";
        let first = catalog.scan(data, SignatureScanConfig::default())?;
        let second = catalog.scan(data, SignatureScanConfig::default())?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn repetitive_magic_does_not_flood_embedded_results() -> Result<(), Box<dyn std::error::Error>>
    {
        let pack = r#"{
          "project_name":"UniversalFileSignatureCompendium",
          "version":"0.1.0",
          "signatures":[{
            "_id":"weak-padding",
            "hex_signatures":["FF FF FF FF"],
            "offsets":[{"type":"bof","value_numeric":0}],
            "common_names":["Weak padding signature"],
            "sources":[{"source_name":"Synthetic","source_url":"","retrieved_at":""}]
          }]
        }"#;
        let catalog = SignatureCatalog::from_ufsc_json(pack.as_bytes())?;
        let report = catalog.scan(&[0xff; 128], SignatureScanConfig::default())?;
        assert_eq!(report.matches.len(), 1);
        assert_eq!(
            report.matches[0].evidence.mode,
            SignatureMatchMode::DeclaredOffset
        );
        assert!(!report.truncated);
        Ok(())
    }
}
