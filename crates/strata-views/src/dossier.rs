//! Source-free, UI-independent dossier for one exact byte selection.
//!
//! A dossier is the connective tissue between projections: it describes what was measured,
//! which investigation objects intersect the bytes, and which reversible action can be taken
//! next. It intentionally retains no source bytes or local source path.

use strata_analysis::poc::{ByteClass, EntropyBlock, classify_byte};
use strata_core::{ByteRange, ByteRangeSet, EvidenceId};

use crate::{
    investigation::{
        CorrelationId, CorrelationStrength, ExactProvenance, FindingId, FindingStatus,
        HypothesisId, HypothesisStatus, InvestigationModel,
    },
    workbench::{
        BranchId, BranchModel, BranchStatus, ComparisonArchaeology, ComparisonClassification,
        ComparisonRegionId, RegionId, RegionModel, RegionRelationshipId,
    },
};

/// Hard bound on retained cross-links in one dossier.
pub const MAX_DOSSIER_LINKS: usize = 32;

/// Inputs used to derive one immutable dossier.
pub struct DossierContext<'a> {
    /// Complete immutable source snapshot. Only bytes covered by `selection` are inspected.
    pub source_bytes: &'a [u8],
    /// Exact selection being described.
    pub selection: ExactProvenance,
    /// Exact entropy blocks available for the same source generation.
    pub entropy_blocks: &'a [EntropyBlock],
    /// Optional canonical artifact digest for the entropy/structure context.
    pub structure_artifact_digest: Option<&'a str>,
    /// Investigation findings, evidence, correlations, and hypotheses.
    pub investigation: &'a InvestigationModel,
    /// Living region graph.
    pub regions: &'a RegionModel,
    /// Reversible hypothesis branches.
    pub branches: &'a BranchModel,
    /// Optional paired-source comparison.
    pub comparison: Option<&'a ComparisonArchaeology>,
}

/// Deterministic measurements over the selected bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct DossierMetrics {
    /// Exact number of selected bytes, including discontiguous ranges.
    pub byte_count: u64,
    /// Number of distinct byte values present.
    pub distinct_values: u16,
    /// Shannon entropy measured over the complete selection.
    pub shannon_entropy_bits: f64,
    /// Number of printable non-space ASCII bytes.
    pub printable_ascii_count: u64,
    /// Number of ASCII whitespace bytes.
    pub whitespace_count: u64,
    /// Number of zero bytes.
    pub zero_count: u64,
    /// Number of `0xff` bytes.
    pub all_ones_count: u64,
    /// Number of bytes with the high bit set, excluding `0xff`.
    pub high_bit_count: u64,
    /// Lowest byte value among values tied for the highest frequency.
    pub dominant_byte: u8,
    /// Exact occurrence count for `dominant_byte`.
    pub dominant_count: u64,
}

/// Exact structural-artifact coverage overlapping the selection.
#[derive(Debug, Clone, PartialEq)]
pub struct DossierStructureContext {
    /// Number of entropy blocks intersecting at least one exact range.
    pub overlapping_blocks: usize,
    /// Selected bytes covered by the supplied blocks.
    pub covered_bytes: u64,
    /// Whether entropy blocks cover every selected byte.
    pub complete: bool,
    /// Minimum entropy among overlapping blocks.
    pub minimum_entropy_bits: f64,
    /// Byte-overlap-weighted mean entropy.
    pub mean_entropy_bits: f64,
    /// Maximum entropy among overlapping blocks.
    pub maximum_entropy_bits: f64,
    /// Canonical structure artifact digest, when available.
    pub artifact_digest: Option<String>,
}

/// Stable target behind a dossier cross-link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierLinkTarget {
    /// Analyzer or analyst finding.
    Finding(FindingId),
    /// Promoted evidence record.
    Evidence(EvidenceId),
    /// Explicit relationship between findings.
    Correlation(CorrelationId),
    /// Testable hypothesis.
    Hypothesis(HypothesisId),
    /// Living source region.
    Region(RegionId),
    /// Typed relationship between living regions.
    RegionRelationship(RegionRelationshipId),
    /// Reversible transform branch.
    Branch(BranchId),
    /// Exact paired-source comparison region.
    Comparison(ComparisonRegionId),
}

/// Compact analyst-facing state for a linked record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierLinkState {
    /// Observed but not promoted or tested.
    Candidate,
    /// Explicitly promoted, corroborated, supported, or pinned.
    Supported,
    /// Tested but inconclusive, or an active exploratory branch.
    Tested,
    /// Dismissed, rejected, or discarded.
    Rejected,
    /// A structural/context record without analyst disposition.
    Context,
}

/// One bounded cross-link whose provenance intersects the exact selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DossierLink {
    /// Stable typed target for navigation or mutation.
    pub target: DossierLinkTarget,
    /// Short record title.
    pub title: String,
    /// Concise rationale, measurement, or status detail.
    pub detail: String,
    /// Normalized analyst-facing state.
    pub state: DossierLinkState,
    /// Exact provenance carried by the linked record.
    pub provenance: ExactProvenance,
}

/// Reproducible action suggested by the dossier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierActionKind {
    /// Inspect byte classes and entropy around the range.
    OpenStructure,
    /// Inspect ordered-byte transitions within the range.
    OpenGrammar,
    /// Query the source for structurally similar windows.
    QueryResonance,
    /// Inspect the exact bytes as addressable 3D voxels.
    OpenProjection,
    /// Compare the same offsets against source B.
    CompareSelection,
    /// Evaluate a reversible single-byte XOR branch.
    TestXorBranch,
    /// Promote an overlapping candidate finding.
    PromoteEvidence,
}

/// Availability and rationale for one next action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DossierAction {
    /// Action identifier consumed by a GUI, CLI, or command palette.
    pub kind: DossierActionKind,
    /// Compact control label.
    pub label: String,
    /// Why the action is relevant or currently unavailable.
    pub rationale: String,
    /// Whether the current exact selection satisfies the action contract.
    pub enabled: bool,
}

/// Source-free investigation object derived from one exact selection.
#[derive(Debug, Clone, PartialEq)]
pub struct InvestigationDossier {
    /// Exact source snapshot and ranges described by this dossier.
    pub provenance: ExactProvenance,
    /// Cautious observation label; never an encryption, compression, or malware verdict.
    pub observed_profile: String,
    /// Deterministic selection measurements.
    pub metrics: DossierMetrics,
    /// Structure artifact context when blocks overlap the selection.
    pub structure: Option<DossierStructureContext>,
    /// Bounded links to findings, evidence, regions, hypotheses, branches, and comparisons.
    pub links: Vec<DossierLink>,
    /// Stable next actions in recommended order.
    pub actions: Vec<DossierAction>,
    /// True when additional intersecting links were omitted by the hard bound.
    pub links_truncated: bool,
}

/// Invalid dossier input that would make the result inexact or misleading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierError {
    /// Selection contains no ranges or no bytes.
    EmptySelection,
    /// Ranges are empty, unordered, or overlapping.
    InvalidSelection,
    /// At least one exact range falls outside the supplied source snapshot.
    SelectionOutOfBounds,
    /// Platform or byte-count arithmetic overflowed.
    ArithmeticOverflow,
}

impl core::fmt::Display for DossierError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DossierError {}

/// Builds a deterministic source-free dossier from exact source ranges.
///
/// # Errors
///
/// Returns [`DossierError`] when selection provenance is empty, invalid, outside the supplied
/// source snapshot, or cannot be represented without arithmetic overflow.
pub fn build_investigation_dossier(
    context: DossierContext<'_>,
) -> Result<InvestigationDossier, DossierError> {
    validate_selection(&context.selection.ranges, context.source_bytes.len())?;
    let histogram = selection_histogram(context.source_bytes, &context.selection.ranges)?;
    let metrics = metrics_from_histogram(&histogram)?;
    let observed_profile = observed_profile(&metrics).to_owned();
    let structure = structure_context(
        &context.selection.ranges,
        context.entropy_blocks,
        metrics.byte_count,
        context.structure_artifact_digest,
    );
    let (links, links_truncated) = collect_links(&context);
    let actions = dossier_actions(&metrics, &links, context.comparison.is_some());
    Ok(InvestigationDossier {
        provenance: context.selection,
        observed_profile,
        metrics,
        structure,
        links,
        actions,
        links_truncated,
    })
}

fn validate_selection(ranges: &ByteRangeSet, source_length: usize) -> Result<(), DossierError> {
    if ranges.ranges.is_empty() {
        return Err(DossierError::EmptySelection);
    }
    let source_length =
        u64::try_from(source_length).map_err(|_| DossierError::ArithmeticOverflow)?;
    let mut previous_end = None;
    let mut selected_bytes = 0_u64;
    for range in &ranges.ranges {
        if range.is_empty() || previous_end.is_some_and(|end| range.start < end) {
            return Err(DossierError::InvalidSelection);
        }
        if range.end > source_length {
            return Err(DossierError::SelectionOutOfBounds);
        }
        selected_bytes = selected_bytes
            .checked_add(range.len())
            .ok_or(DossierError::ArithmeticOverflow)?;
        previous_end = Some(range.end);
    }
    if selected_bytes == 0 {
        return Err(DossierError::EmptySelection);
    }
    Ok(())
}

fn selection_histogram(
    source_bytes: &[u8],
    ranges: &ByteRangeSet,
) -> Result<[u64; 256], DossierError> {
    let mut histogram = [0_u64; 256];
    for range in &ranges.ranges {
        let start = usize::try_from(range.start).map_err(|_| DossierError::ArithmeticOverflow)?;
        let end = usize::try_from(range.end).map_err(|_| DossierError::ArithmeticOverflow)?;
        let bytes = source_bytes
            .get(start..end)
            .ok_or(DossierError::SelectionOutOfBounds)?;
        for &byte in bytes {
            histogram[usize::from(byte)] = histogram[usize::from(byte)]
                .checked_add(1)
                .ok_or(DossierError::ArithmeticOverflow)?;
        }
    }
    Ok(histogram)
}

fn metrics_from_histogram(histogram: &[u64; 256]) -> Result<DossierMetrics, DossierError> {
    let byte_count = histogram.iter().try_fold(0_u64, |total, count| {
        total
            .checked_add(*count)
            .ok_or(DossierError::ArithmeticOverflow)
    })?;
    if byte_count == 0 {
        return Err(DossierError::EmptySelection);
    }
    let mut distinct_values = 0_u16;
    let mut entropy = 0.0;
    let mut dominant_byte = 0_u8;
    let mut dominant_count = 0_u64;
    let mut class_counts = [0_u64; 6];
    for (value, &count) in histogram.iter().enumerate() {
        if count == 0 {
            continue;
        }
        distinct_values = distinct_values
            .checked_add(1)
            .ok_or(DossierError::ArithmeticOverflow)?;
        let probability = u64_to_f64(count) / u64_to_f64(byte_count);
        entropy -= probability * probability.log2();
        if count > dominant_count {
            dominant_count = count;
            dominant_byte = u8::try_from(value).map_err(|_| DossierError::ArithmeticOverflow)?;
        }
        let class_index =
            match classify_byte(u8::try_from(value).map_err(|_| DossierError::ArithmeticOverflow)?)
            {
                ByteClass::Zero => 0,
                ByteClass::AllOnes => 1,
                ByteClass::Whitespace => 2,
                ByteClass::PrintableAscii => 3,
                ByteClass::Control => 4,
                ByteClass::HighBit => 5,
            };
        class_counts[class_index] = class_counts[class_index]
            .checked_add(count)
            .ok_or(DossierError::ArithmeticOverflow)?;
    }
    Ok(DossierMetrics {
        byte_count,
        distinct_values,
        shannon_entropy_bits: entropy,
        printable_ascii_count: class_counts[3],
        whitespace_count: class_counts[2],
        zero_count: class_counts[0],
        all_ones_count: class_counts[1],
        high_bit_count: class_counts[5],
        dominant_byte,
        dominant_count,
    })
}

fn observed_profile(metrics: &DossierMetrics) -> &'static str {
    if metrics.zero_count == metrics.byte_count {
        "Uniform zero bytes"
    } else if metrics.all_ones_count == metrics.byte_count {
        "Uniform 0xff bytes; erased or padding candidate"
    } else if metrics
        .printable_ascii_count
        .saturating_add(metrics.whitespace_count)
        .saturating_mul(10)
        >= metrics.byte_count.saturating_mul(8)
    {
        "Text-like byte distribution"
    } else if metrics.shannon_entropy_bits >= 7.2 {
        "High-diversity bytes; no compression or encryption verdict"
    } else if metrics.distinct_values <= 4 {
        "Low-diversity structured bytes"
    } else {
        "Mixed byte structure"
    }
}

fn structure_context(
    selection: &ByteRangeSet,
    blocks: &[EntropyBlock],
    selected_bytes: u64,
    artifact_digest: Option<&str>,
) -> Option<DossierStructureContext> {
    let mut overlapping_blocks = 0_usize;
    let mut covered_bytes = 0_u64;
    let mut weighted_entropy = 0.0;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for block in blocks {
        let Some(block_end) = block.offset.checked_add(block.length) else {
            continue;
        };
        let block_range = ByteRange {
            start: block.offset,
            end: block_end,
        };
        let overlap = selection
            .ranges
            .iter()
            .filter_map(|range| overlap_length(*range, block_range))
            .sum::<u64>();
        if overlap == 0 {
            continue;
        }
        overlapping_blocks = overlapping_blocks.saturating_add(1);
        covered_bytes = covered_bytes.saturating_add(overlap);
        weighted_entropy += block.shannon_entropy_bits * u64_to_f64(overlap);
        minimum = minimum.min(block.shannon_entropy_bits);
        maximum = maximum.max(block.shannon_entropy_bits);
    }
    if overlapping_blocks == 0 || covered_bytes == 0 {
        return None;
    }
    Some(DossierStructureContext {
        overlapping_blocks,
        covered_bytes,
        complete: covered_bytes == selected_bytes,
        minimum_entropy_bits: minimum,
        mean_entropy_bits: weighted_entropy / u64_to_f64(covered_bytes),
        maximum_entropy_bits: maximum,
        artifact_digest: artifact_digest.map(ToOwned::to_owned),
    })
}

fn u64_to_f64(value: u64) -> f64 {
    const TWO_TO_32: f64 = 4_294_967_296.0;
    let high = u32::try_from(value >> 32).map_or(u32::MAX, core::convert::identity);
    let low = u32::try_from(value & u64::from(u32::MAX)).map_or(u32::MAX, core::convert::identity);
    f64::from(high).mul_add(TWO_TO_32, f64::from(low))
}

fn collect_links(context: &DossierContext<'_>) -> (Vec<DossierLink>, bool) {
    let mut links = Vec::new();
    collect_investigation_links(&mut links, context);
    collect_region_links(&mut links, context);
    collect_branch_links(&mut links, context);
    collect_comparison_links(&mut links, context);
    let links_truncated = links.len() > MAX_DOSSIER_LINKS;
    links.truncate(MAX_DOSSIER_LINKS);
    (links, links_truncated)
}

fn collect_investigation_links(links: &mut Vec<DossierLink>, context: &DossierContext<'_>) {
    let selection = &context.selection;
    for finding in context.investigation.findings() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::Finding(finding.id),
                title: finding.title.clone(),
                detail: finding.detail.clone(),
                state: finding_state(finding.status),
                provenance: finding.provenance.clone(),
            },
        );
    }
    for evidence in context.investigation.evidence() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::Evidence(evidence.id),
                title: "Promoted evidence".to_owned(),
                detail: evidence.claim.clone(),
                state: DossierLinkState::Supported,
                provenance: evidence.provenance.clone(),
            },
        );
    }
    for correlation in context.investigation.correlations() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::Correlation(correlation.id),
                title: format!("Correlation · {} records", correlation.finding_ids.len()),
                detail: correlation.rationale.clone(),
                state: correlation_state(correlation.strength),
                provenance: correlation.provenance.clone(),
            },
        );
    }
    for hypothesis in context.investigation.hypotheses() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::Hypothesis(hypothesis.id),
                title: "Hypothesis".to_owned(),
                detail: hypothesis.statement.clone(),
                state: hypothesis_state(hypothesis.status),
                provenance: hypothesis.provenance.clone(),
            },
        );
    }
}

fn collect_region_links(links: &mut Vec<DossierLink>, context: &DossierContext<'_>) {
    let selection = &context.selection;
    for region in context.regions.regions() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::Region(region.id),
                title: region.label.clone(),
                detail: format!("Living region · {:?}", region.kind),
                state: DossierLinkState::Context,
                provenance: region.provenance.clone(),
            },
        );
    }
    for relationship in context.regions.relationships() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::RegionRelationship(relationship.id),
                title: format!("Region link · {:?}", relationship.kind),
                detail: relationship.rationale.clone(),
                state: DossierLinkState::Context,
                provenance: relationship.provenance.clone(),
            },
        );
    }
}

fn collect_branch_links(links: &mut Vec<DossierLink>, context: &DossierContext<'_>) {
    let selection = &context.selection;
    for branch in context.branches.branches() {
        push_link(
            links,
            selection,
            DossierLink {
                target: DossierLinkTarget::Branch(branch.id),
                title: branch.label.clone(),
                detail: "Reversible transform branch; loss none".to_owned(),
                state: branch_state(branch.status),
                provenance: branch.provenance.clone(),
            },
        );
    }
}

fn collect_comparison_links(links: &mut Vec<DossierLink>, context: &DossierContext<'_>) {
    let selection = &context.selection;
    if let Some(comparison) = context.comparison {
        for region in comparison.regions() {
            let provenance = region
                .left
                .as_ref()
                .filter(|provenance| same_snapshot(selection, provenance))
                .unwrap_or(&region.right);
            if !same_snapshot(selection, provenance) {
                continue;
            }
            push_link(
                links,
                selection,
                DossierLink {
                    target: DossierLinkTarget::Comparison(region.id),
                    title: format!("Comparison · {}", comparison_label(region.classification)),
                    detail: region.explanation.clone(),
                    state: DossierLinkState::Context,
                    provenance: provenance.clone(),
                },
            );
        }
    }
}

fn push_link(links: &mut Vec<DossierLink>, selection: &ExactProvenance, link: DossierLink) {
    if same_snapshot(selection, &link.provenance)
        && range_sets_overlap(&selection.ranges, &link.provenance.ranges)
    {
        links.push(link);
    }
}

fn dossier_actions(
    metrics: &DossierMetrics,
    links: &[DossierLink],
    comparison_available: bool,
) -> Vec<DossierAction> {
    let candidate_finding = links.iter().any(|link| {
        matches!(link.target, DossierLinkTarget::Finding(_))
            && link.state == DossierLinkState::Candidate
    });
    vec![
        DossierAction {
            kind: DossierActionKind::OpenStructure,
            label: "Structure".to_owned(),
            rationale: "Inspect exact byte classes and surrounding entropy".to_owned(),
            enabled: true,
        },
        DossierAction {
            kind: DossierActionKind::OpenGrammar,
            label: "Grammar".to_owned(),
            rationale: if metrics.byte_count >= 2 {
                "Measure ordered-byte transitions inside the selection".to_owned()
            } else {
                "Select at least two bytes for a transition".to_owned()
            },
            enabled: metrics.byte_count >= 2,
        },
        DossierAction {
            kind: DossierActionKind::QueryResonance,
            label: "Find echoes".to_owned(),
            rationale: if metrics.byte_count >= 4 {
                "Query structurally similar windows across the source".to_owned()
            } else {
                "Select at least four bytes for a useful query".to_owned()
            },
            enabled: metrics.byte_count >= 4,
        },
        DossierAction {
            kind: DossierActionKind::OpenProjection,
            label: "3D voxels".to_owned(),
            rationale: if metrics.byte_count >= 3 {
                "Inspect exact three-byte contributors without losing offsets".to_owned()
            } else {
                "Select at least three bytes for one voxel".to_owned()
            },
            enabled: metrics.byte_count >= 3,
        },
        DossierAction {
            kind: DossierActionKind::CompareSelection,
            label: "A ↔ B".to_owned(),
            rationale: if comparison_available {
                "Inspect the same offsets and classified deltas in source B".to_owned()
            } else {
                "Attach comparison source B first".to_owned()
            },
            enabled: comparison_available,
        },
        DossierAction {
            kind: DossierActionKind::TestXorBranch,
            label: "Test XOR".to_owned(),
            rationale: "Create a reversible branch over these exact bytes".to_owned(),
            enabled: true,
        },
        DossierAction {
            kind: DossierActionKind::PromoteEvidence,
            label: "Promote".to_owned(),
            rationale: if candidate_finding {
                "Promote the first overlapping candidate with exact provenance".to_owned()
            } else {
                "No unreviewed finding intersects the selection".to_owned()
            },
            enabled: candidate_finding,
        },
    ]
}

const fn finding_state(status: FindingStatus) -> DossierLinkState {
    match status {
        FindingStatus::Candidate => DossierLinkState::Candidate,
        FindingStatus::Promoted => DossierLinkState::Supported,
        FindingStatus::Dismissed => DossierLinkState::Rejected,
    }
}

const fn correlation_state(status: CorrelationStrength) -> DossierLinkState {
    match status {
        CorrelationStrength::Candidate => DossierLinkState::Candidate,
        CorrelationStrength::Corroborated => DossierLinkState::Supported,
        CorrelationStrength::Rejected => DossierLinkState::Rejected,
    }
}

const fn hypothesis_state(status: HypothesisStatus) -> DossierLinkState {
    match status {
        HypothesisStatus::Draft => DossierLinkState::Candidate,
        HypothesisStatus::Tested => DossierLinkState::Tested,
        HypothesisStatus::Supported => DossierLinkState::Supported,
        HypothesisStatus::Rejected => DossierLinkState::Rejected,
    }
}

const fn branch_state(status: BranchStatus) -> DossierLinkState {
    match status {
        BranchStatus::Draft => DossierLinkState::Candidate,
        BranchStatus::Active => DossierLinkState::Tested,
        BranchStatus::Pinned => DossierLinkState::Supported,
        BranchStatus::Discarded => DossierLinkState::Rejected,
    }
}

const fn comparison_label(classification: ComparisonClassification) -> &'static str {
    match classification {
        ComparisonClassification::Unchanged => "unchanged",
        ComparisonClassification::Moved => "moved",
        ComparisonClassification::Modified => "modified",
        ComparisonClassification::New => "new",
    }
}

fn same_snapshot(first: &ExactProvenance, second: &ExactProvenance) -> bool {
    first.source_id == second.source_id && first.generation == second.generation
}

fn range_sets_overlap(first: &ByteRangeSet, second: &ByteRangeSet) -> bool {
    first.ranges.iter().any(|left| {
        second
            .ranges
            .iter()
            .any(|right| overlap_length(*left, *right).is_some())
    })
}

fn overlap_length(first: ByteRange, second: ByteRange) -> Option<u64> {
    let start = first.start.max(second.start);
    let end = first.end.min(second.end);
    if start < end { Some(end - start) } else { None }
}

#[cfg(test)]
mod tests {
    use strata_core::{ByteRange, ByteRangeSet, SourceGeneration, SourceId};

    use super::*;
    use crate::investigation::{Finding, InvestigationModel};

    fn provenance(start: u64, end: u64) -> ExactProvenance {
        ExactProvenance {
            source_id: SourceId(7),
            generation: SourceGeneration(3),
            ranges: ByteRangeSet {
                ranges: vec![ByteRange { start, end }],
            },
        }
    }

    fn empty_context<'a>(
        bytes: &'a [u8],
        selection: ExactProvenance,
        investigation: &'a InvestigationModel,
        regions: &'a RegionModel,
        branches: &'a BranchModel,
    ) -> DossierContext<'a> {
        DossierContext {
            source_bytes: bytes,
            selection,
            entropy_blocks: &[],
            structure_artifact_digest: None,
            investigation,
            regions,
            branches,
            comparison: None,
        }
    }

    #[test]
    fn metrics_are_exact_deterministic_and_source_free() -> Result<(), DossierError> {
        let bytes = b"/Users/example/private.bin\0\xffAAAA";
        let investigation = InvestigationModel::new();
        let regions = RegionModel::new();
        let branches = BranchModel::new();
        let first = build_investigation_dossier(empty_context(
            bytes,
            provenance(
                0,
                u64::try_from(bytes.len()).map_err(|_| DossierError::ArithmeticOverflow)?,
            ),
            &investigation,
            &regions,
            &branches,
        ))?;
        let second = build_investigation_dossier(empty_context(
            bytes,
            provenance(
                0,
                u64::try_from(bytes.len()).map_err(|_| DossierError::ArithmeticOverflow)?,
            ),
            &investigation,
            &regions,
            &branches,
        ))?;
        assert_eq!(first, second);
        assert_eq!(first.metrics.byte_count, bytes.len() as u64);
        let debug = format!("{first:?}");
        assert!(!debug.contains("/Users/example"));
        assert!(!debug.contains("private.bin"));
        Ok(())
    }

    #[test]
    fn only_intersecting_records_become_links() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = [0_u8; 32];
        let mut investigation = InvestigationModel::new();
        investigation.add_finding(Finding {
            id: FindingId(1),
            title: "inside".to_owned(),
            detail: "intersects".to_owned(),
            status: FindingStatus::Candidate,
            provenance: provenance(8, 12),
        })?;
        investigation.add_finding(Finding {
            id: FindingId(2),
            title: "outside".to_owned(),
            detail: "does not intersect".to_owned(),
            status: FindingStatus::Candidate,
            provenance: provenance(20, 24),
        })?;
        let regions = RegionModel::new();
        let branches = BranchModel::new();
        let dossier = build_investigation_dossier(empty_context(
            &bytes,
            provenance(10, 14),
            &investigation,
            &regions,
            &branches,
        ))?;
        assert_eq!(dossier.links.len(), 1);
        assert_eq!(dossier.links[0].title, "inside");
        assert!(
            dossier
                .actions
                .iter()
                .any(|action| action.kind == DossierActionKind::PromoteEvidence && action.enabled)
        );
        Ok(())
    }

    #[test]
    fn entropy_context_discloses_partial_coverage() -> Result<(), DossierError> {
        let bytes = [0_u8; 16];
        let blocks = [EntropyBlock {
            offset: 4,
            length: 4,
            shannon_entropy_bits: 2.0,
        }];
        let investigation = InvestigationModel::new();
        let regions = RegionModel::new();
        let branches = BranchModel::new();
        let dossier = build_investigation_dossier(DossierContext {
            source_bytes: &bytes,
            selection: provenance(4, 12),
            entropy_blocks: &blocks,
            structure_artifact_digest: Some("abc123"),
            investigation: &investigation,
            regions: &regions,
            branches: &branches,
            comparison: None,
        })?;
        let structure = dossier.structure.ok_or(DossierError::InvalidSelection)?;
        assert_eq!(structure.covered_bytes, 4);
        assert!(!structure.complete);
        assert_eq!(structure.artifact_digest.as_deref(), Some("abc123"));
        Ok(())
    }

    #[test]
    fn out_of_bounds_selection_is_rejected() {
        let bytes = [0_u8; 8];
        let investigation = InvestigationModel::new();
        let regions = RegionModel::new();
        let branches = BranchModel::new();
        let result = build_investigation_dossier(empty_context(
            &bytes,
            provenance(7, 9),
            &investigation,
            &regions,
            &branches,
        ));
        assert_eq!(result, Err(DossierError::SelectionOutOfBounds));
    }
}
