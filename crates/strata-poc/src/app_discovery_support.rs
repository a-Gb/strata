//! Stateless discovery, region, comparison, and cohort mappings.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

pub(super) const fn discovery_priority(kind: WorkbenchLeadKind) -> u8 {
    match kind {
        WorkbenchLeadKind::TransformCandidate => 0,
        WorkbenchLeadKind::CatalogSignature | WorkbenchLeadKind::EmbeddedSignature => 1,
        WorkbenchLeadKind::Periodicity => 2,
        WorkbenchLeadKind::EntropyBoundary => 3,
        WorkbenchLeadKind::ExactRepeat => 4,
    }
}

pub(super) const fn investigation_finding_id(id: WorkbenchLeadId, slot: u8) -> FindingId {
    FindingId((id.0 as u128) << 8 | slot as u128)
}

pub(super) const fn investigation_correlation_id(id: WorkbenchLeadId) -> CorrelationId {
    CorrelationId((id.0 as u128) << 8 | 0x40)
}

pub(super) const fn investigation_hypothesis_id(id: WorkbenchLeadId) -> HypothesisId {
    HypothesisId((id.0 as u128) << 8 | 0x80)
}

pub(super) const fn investigation_evidence_id(id: WorkbenchLeadId) -> EvidenceId {
    EvidenceId((id.0 as u128) << 8 | 0xc0)
}

pub(super) fn discovery_provenance(ranges: &[ByteRange], generation: u64) -> ExactProvenance {
    let mut ranges = ranges.to_vec();
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    ranges.dedup();
    ExactProvenance {
        source_id: SourceId(1),
        generation: SourceGeneration(generation),
        ranges: ByteRangeSet { ranges },
    }
}

pub(super) fn build_investigation_model(
    findings: &[WorkbenchLead],
    generation: u64,
) -> Result<InvestigationModel, InvestigationError> {
    let mut model = InvestigationModel::new();
    for finding in findings {
        let provenance = discovery_provenance(&finding.source_ranges, generation);
        model.add_finding(InvestigationFinding {
            id: investigation_finding_id(finding.id, 0),
            title: discovery_title(finding).to_owned(),
            detail: discovery_evidence_summary(finding),
            status: FindingStatus::Candidate,
            provenance: provenance.clone(),
        })?;

        if finding.source_ranges.len() >= 2 {
            let mut members = Vec::with_capacity(finding.source_ranges.len());
            for (index, range) in finding.source_ranges.iter().copied().enumerate() {
                let slot = u8::try_from(index.saturating_add(1))
                    .map_err(|_| InvestigationError::InvalidCorrelationMembers)?;
                let id = investigation_finding_id(finding.id, slot);
                model.add_finding(InvestigationFinding {
                    id,
                    title: format!("Linked range {}", index.saturating_add(1)),
                    detail: format!("0x{:08x}..0x{:08x}", range.start, range.end),
                    status: FindingStatus::Candidate,
                    provenance: discovery_provenance(&[range], generation),
                })?;
                members.push(id);
            }
            model.add_correlation(Correlation {
                id: investigation_correlation_id(finding.id),
                finding_ids: members,
                provenance: provenance.clone(),
                strength: CorrelationStrength::Corroborated,
                rationale: discovery_correlation_rationale(finding),
            })?;
        }

        if finding.kind == WorkbenchLeadKind::TransformCandidate {
            model.add_hypothesis(Hypothesis {
                id: investigation_hypothesis_id(finding.id),
                statement: discovery_hypothesis_statement(finding),
                provenance,
                status: HypothesisStatus::Draft,
                evidence_ids: Vec::new(),
            })?;
        }
    }
    Ok(model)
}

pub(super) const fn discovery_title(finding: &WorkbenchLead) -> &'static str {
    match finding.kind {
        WorkbenchLeadKind::EntropyBoundary => "Entropy boundary",
        WorkbenchLeadKind::ExactRepeat => "Exact repeated bytes",
        WorkbenchLeadKind::Periodicity => "Candidate record width",
        WorkbenchLeadKind::EmbeddedSignature => "Embedded object signature",
        WorkbenchLeadKind::TransformCandidate => "Reversible transform candidate",
        WorkbenchLeadKind::CatalogSignature => "Catalog pattern match",
    }
}

pub(super) fn discovery_evidence_summary(finding: &WorkbenchLead) -> String {
    match &finding.evidence {
        WorkbenchEvidence::EntropyBoundary {
            before_entropy_bits,
            after_entropy_bits,
            entropy_delta_bits,
        } => format!(
            "entropy changes {before_entropy_bits:.2} -> {after_entropy_bits:.2} bits/byte (delta {entropy_delta_bits:.2})"
        ),
        WorkbenchEvidence::ExactRepeat {
            sampled_window_step,
            matching_byte_count,
        } => format!(
            "{matching_byte_count} exact bytes repeat; sampled every {sampled_window_step} source bytes"
        ),
        WorkbenchEvidence::Periodicity {
            period_bytes,
            compared_positions,
            matching_positions,
        } => format!(
            "period {period_bytes} bytes: {matching_positions} of {compared_positions} aligned positions agree"
        ),
        WorkbenchEvidence::EmbeddedSignature { signature } => {
            format!("exact {signature:?} magic bytes; signature is a lead, not a parser claim")
        }
        WorkbenchEvidence::TransformCandidate(evaluation) => format!(
            "{} changes text likelihood {:.0}% -> {:.0}% ({})",
            transform_label(evaluation.transform),
            evaluation.before.text_likelihood * 100.0,
            evaluation.after.text_likelihood * 100.0,
            transform_assessment_label(evaluation.assessment)
        ),
        WorkbenchEvidence::XorCorrelatedTransform {
            transform,
            correlated_byte_count,
            distinct_byte_count,
            sampled_window_step,
            ..
        } => format!(
            "{correlated_byte_count} pairs satisfy A -> {} -> B across {distinct_byte_count} values; sampled step {sampled_window_step}",
            transform_label(*transform)
        ),
        WorkbenchEvidence::CatalogSignature(evidence) => {
            let label = evidence
                .candidates
                .first()
                .map_or("unnamed signature", |candidate| candidate.label.as_str());
            let ambiguity = evidence.candidates.len().saturating_sub(1);
            let mode = match evidence.mode {
                strata_analysis::signatures::SignatureMatchMode::DeclaredOffset => {
                    "declared offset"
                }
                strata_analysis::signatures::SignatureMatchMode::EmbeddedSearch => {
                    "embedded search"
                }
            };
            format!(
                "{label}: {} exact bytes / {} distinct + {} wildcard at {mode}; {ambiguity} competing interpretation(s)",
                evidence.exact_byte_count,
                evidence.distinct_exact_byte_count,
                evidence.wildcard_byte_count
            )
        }
    }
}

pub(super) fn discovery_correlation_rationale(finding: &WorkbenchLead) -> String {
    match finding.kind {
        WorkbenchLeadKind::ExactRepeat => {
            "The linked ranges contain byte-for-byte identical fixed windows.".to_owned()
        }
        WorkbenchLeadKind::TransformCandidate => discovery_evidence_summary(finding),
        WorkbenchLeadKind::EntropyBoundary
        | WorkbenchLeadKind::Periodicity
        | WorkbenchLeadKind::EmbeddedSignature
        | WorkbenchLeadKind::CatalogSignature => {
            "The analyzer supplied multiple exact contributing ranges.".to_owned()
        }
    }
}

pub(super) fn discovery_hypothesis_statement(finding: &WorkbenchLead) -> String {
    match &finding.evidence {
        WorkbenchEvidence::XorCorrelatedTransform { transform, .. } => format!(
            "The second exact range is a reversible {} branch of the first.",
            transform_label(*transform)
        ),
        WorkbenchEvidence::TransformCandidate(evaluation) => format!(
            "Applying {} may improve the measurable interpretation of this range.",
            transform_label(evaluation.transform)
        ),
        WorkbenchEvidence::Periodicity { period_bytes, .. } => {
            format!("A {period_bytes}-byte record interpretation may explain the local repetition.")
        }
        WorkbenchEvidence::EntropyBoundary { .. } => {
            "The adjacent ranges may represent different storage or semantic regimes.".to_owned()
        }
        WorkbenchEvidence::ExactRepeat { .. } => {
            "The exact repeated ranges may share a structural role.".to_owned()
        }
        WorkbenchEvidence::EmbeddedSignature { signature } => format!(
            "The {signature:?} signature may mark a bounded embedded object; carving still requires verification."
        ),
        WorkbenchEvidence::CatalogSignature(evidence) => {
            let label = evidence
                .candidates
                .first()
                .map_or("catalog pattern", |candidate| candidate.label.as_str());
            format!(
                "The exact bytes support the {label} catalog candidate; structure and bounds still require corroboration."
            )
        }
    }
}

pub(super) const fn discovery_next_action(finding: &WorkbenchLead) -> &'static str {
    match finding.kind {
        WorkbenchLeadKind::EntropyBoundary => "Compare both sides and inspect signatures",
        WorkbenchLeadKind::ExactRepeat => "Find all echoes and compare surrounding context",
        WorkbenchLeadKind::Periodicity => "Open the record lab at the candidate stride",
        WorkbenchLeadKind::EmbeddedSignature => "Verify bounds before carving the object",
        WorkbenchLeadKind::TransformCandidate => "Test a reversible branch and inspect deltas",
        WorkbenchLeadKind::CatalogSignature => {
            "Inspect ambiguity and corroborate surrounding structure"
        }
    }
}

pub(super) fn discovery_range_summary(ranges: &[ByteRange]) -> String {
    ranges
        .iter()
        .map(|range| format!("0x{:x}..0x{:x}", range.start, range.end))
        .collect::<Vec<_>>()
        .join("  <->  ")
}

pub(super) const fn discovery_kind_label(kind: WorkbenchLeadKind) -> &'static str {
    match kind {
        WorkbenchLeadKind::EntropyBoundary => "BOUNDARY",
        WorkbenchLeadKind::ExactRepeat => "EXACT REPEAT",
        WorkbenchLeadKind::Periodicity => "PERIODICITY",
        WorkbenchLeadKind::EmbeddedSignature => "SIGNATURE",
        WorkbenchLeadKind::TransformCandidate => "REVERSIBLE TEST",
        WorkbenchLeadKind::CatalogSignature => "CATALOG MATCH",
    }
}

pub(super) const fn discovery_kind_color(kind: WorkbenchLeadKind) -> egui::Color32 {
    match kind {
        WorkbenchLeadKind::EntropyBoundary => egui::Color32::from_rgb(104, 175, 206),
        WorkbenchLeadKind::ExactRepeat => egui::Color32::from_rgb(94, 170, 220),
        WorkbenchLeadKind::Periodicity => egui::Color32::from_rgb(142, 176, 218),
        WorkbenchLeadKind::EmbeddedSignature => egui::Color32::from_rgb(198, 205, 212),
        WorkbenchLeadKind::TransformCandidate => egui::Color32::from_rgb(74, 190, 168),
        WorkbenchLeadKind::CatalogSignature => egui::Color32::from_rgb(154, 166, 174),
    }
}

pub(super) fn discovery_lead_color(finding: &WorkbenchLead) -> egui::Color32 {
    match &finding.evidence {
        WorkbenchEvidence::CatalogSignature(evidence) => signature_category_color(evidence),
        _ => discovery_kind_color(finding.kind),
    }
}

pub(super) const fn finding_status_label(status: FindingStatus) -> &'static str {
    match status {
        FindingStatus::Candidate => "candidate",
        FindingStatus::Promoted => "evidence",
        FindingStatus::Dismissed => "dismissed",
    }
}

pub(super) const fn hypothesis_status_label(status: HypothesisStatus) -> &'static str {
    match status {
        HypothesisStatus::Draft => "draft",
        HypothesisStatus::Tested => "tested",
        HypothesisStatus::Supported => "supported",
        HypothesisStatus::Rejected => "rejected",
    }
}

pub(super) const fn discovery_transform(finding: &WorkbenchLead) -> Option<ReversibleTransform> {
    match &finding.evidence {
        WorkbenchEvidence::TransformCandidate(evaluation) => Some(evaluation.transform),
        WorkbenchEvidence::XorCorrelatedTransform { transform, .. } => Some(*transform),
        WorkbenchEvidence::EntropyBoundary { .. }
        | WorkbenchEvidence::ExactRepeat { .. }
        | WorkbenchEvidence::Periodicity { .. }
        | WorkbenchEvidence::EmbeddedSignature { .. }
        | WorkbenchEvidence::CatalogSignature(_) => None,
    }
}

pub(super) const fn discovery_transform_range(finding: &WorkbenchLead) -> Option<ByteRange> {
    match &finding.evidence {
        WorkbenchEvidence::TransformCandidate(evaluation) => Some(evaluation.source_range),
        WorkbenchEvidence::XorCorrelatedTransform {
            transformed_range, ..
        } => Some(*transformed_range),
        WorkbenchEvidence::EntropyBoundary { .. }
        | WorkbenchEvidence::ExactRepeat { .. }
        | WorkbenchEvidence::Periodicity { .. }
        | WorkbenchEvidence::EmbeddedSignature { .. }
        | WorkbenchEvidence::CatalogSignature(_) => None,
    }
}

pub(super) fn transform_label(transform: ReversibleTransform) -> String {
    match transform {
        ReversibleTransform::XorByte(key) => format!("XOR 0x{key:02x}"),
    }
}

pub(super) const fn transform_assessment_label(assessment: TransformAssessment) -> &'static str {
    match assessment {
        TransformAssessment::Supported => "strengthened",
        TransformAssessment::Neutral => "neutral",
        TransformAssessment::Contradicted => "weakened",
    }
}

pub(super) fn build_detected_region_model(
    findings: &[WorkbenchLead],
    generation: u64,
) -> Result<RegionModel, String> {
    let mut model = RegionModel::new();
    for finding in findings {
        let mut member_ids = Vec::with_capacity(finding.source_ranges.len());
        for (index, range) in finding.source_ranges.iter().copied().enumerate() {
            let index_id = u128::try_from(index.saturating_add(1))
                .map_err(|_| "detected region index overflow".to_owned())?;
            let id = RegionId((u128::from(finding.id.0) << 8) | index_id);
            model
                .add_region(LivingRegion {
                    id,
                    label: format!(
                        "{} · range {}",
                        discovery_title(finding),
                        index.saturating_add(1)
                    ),
                    kind: detected_region_kind(finding.kind),
                    provenance: discovery_provenance(&[range], generation),
                    parent_id: None,
                })
                .map_err(|error| error.to_string())?;
            member_ids.push(id);
        }
        if let [from, to, ..] = member_ids.as_slice() {
            let kind = match (&finding.kind, &finding.evidence) {
                (WorkbenchLeadKind::ExactRepeat, _) => RegionRelationshipKind::Repeats,
                (
                    WorkbenchLeadKind::TransformCandidate,
                    WorkbenchEvidence::XorCorrelatedTransform { .. },
                ) => RegionRelationshipKind::XorEncoded,
                (WorkbenchLeadKind::EntropyBoundary, _) => RegionRelationshipKind::Adjacent,
                _ => RegionRelationshipKind::Similar,
            };
            model
                .add_relationship(RegionRelationship {
                    id: RegionRelationshipId((u128::from(finding.id.0) << 8) | 0xf0),
                    from: *from,
                    to: *to,
                    kind,
                    provenance: discovery_provenance(&finding.source_ranges, generation),
                    rationale: discovery_evidence_summary(finding),
                })
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(model)
}

pub(super) fn detected_region_kind(kind: WorkbenchLeadKind) -> RegionKind {
    match kind {
        WorkbenchLeadKind::EntropyBoundary => RegionKind::Structural,
        WorkbenchLeadKind::ExactRepeat => RegionKind::Custom("repeated".to_owned()),
        WorkbenchLeadKind::Periodicity => RegionKind::Table,
        WorkbenchLeadKind::EmbeddedSignature => RegionKind::Custom("embedded-object".to_owned()),
        WorkbenchLeadKind::CatalogSignature => RegionKind::Custom("catalog-signature".to_owned()),
        WorkbenchLeadKind::TransformCandidate => {
            RegionKind::Custom("transform-candidate".to_owned())
        }
    }
}

pub(super) fn region_kind_label(kind: &RegionKind) -> String {
    match kind {
        RegionKind::Header => "HEADER".to_owned(),
        RegionKind::Table => "TABLE".to_owned(),
        RegionKind::Code => "CODE".to_owned(),
        RegionKind::Text => "TEXT".to_owned(),
        RegionKind::Padding => "PADDING".to_owned(),
        RegionKind::Structural => "STRUCTURAL".to_owned(),
        RegionKind::Custom(label) => label.to_ascii_uppercase(),
    }
}

pub(super) const fn region_kind_color(kind: &RegionKind) -> egui::Color32 {
    match kind {
        RegionKind::Header => egui::Color32::from_rgb(92, 169, 219),
        RegionKind::Table => egui::Color32::from_rgb(48, 170, 196),
        RegionKind::Code => egui::Color32::from_rgb(149, 130, 204),
        RegionKind::Text => egui::Color32::from_rgb(75, 187, 151),
        RegionKind::Padding => egui::Color32::from_rgb(71, 80, 86),
        RegionKind::Structural => egui::Color32::from_rgb(119, 154, 174),
        RegionKind::Custom(_) => egui::Color32::from_rgb(194, 151, 72),
    }
}

pub(super) const fn region_relationship_label(kind: RegionRelationshipKind) -> &'static str {
    match kind {
        RegionRelationshipKind::References => "REFERENCES",
        RegionRelationshipKind::Adjacent => "ADJACENT",
        RegionRelationshipKind::Similar => "SIMILAR",
        RegionRelationshipKind::Repeats => "REPEATS",
        RegionRelationshipKind::XorEncoded => "XOR-ENCODED",
    }
}

pub(super) const fn comparison_class_index(classification: ComparisonClassification) -> usize {
    match classification {
        ComparisonClassification::Unchanged => 0,
        ComparisonClassification::Moved => 1,
        ComparisonClassification::Modified => 2,
        ComparisonClassification::New => 3,
    }
}

pub(super) const fn comparison_class_label(
    classification: ComparisonClassification,
) -> &'static str {
    match classification {
        ComparisonClassification::Unchanged => "UNCHANGED",
        ComparisonClassification::Moved => "MOVED",
        ComparisonClassification::Modified => "MODIFIED",
        ComparisonClassification::New => "NEW",
    }
}

pub(super) const fn comparison_class_color(
    classification: ComparisonClassification,
) -> egui::Color32 {
    match classification {
        ComparisonClassification::Unchanged => egui::Color32::from_rgb(91, 118, 130),
        ComparisonClassification::Moved => egui::Color32::from_rgb(74, 190, 168),
        ComparisonClassification::Modified => egui::Color32::from_rgb(235, 179, 66),
        ComparisonClassification::New => egui::Color32::from_rgb(74, 157, 220),
    }
}

pub(super) fn format_byte_range(range: &ByteRange) -> String {
    format!("0x{:x}..0x{:x}", range.start, range.end)
}

pub(super) fn comparison_range_rect(
    range: ByteRange,
    length: usize,
    track: egui::Rect,
) -> egui::Rect {
    let start = source_offset_x(range.start, length, track);
    let end = source_offset_x(range.end, length, track).max(start + 2.0);
    egui::Rect::from_min_max(
        egui::pos2(start, track.top()),
        egui::pos2(end, track.bottom()),
    )
}

pub(super) fn preview_range(bytes: &[u8], range: ByteRange) -> String {
    let (Ok(start), Ok(end)) = (usize::try_from(range.start), usize::try_from(range.end)) else {
        return "range overflow".to_owned();
    };
    let Some(selected) = bytes.get(start..end) else {
        return "range unavailable".to_owned();
    };
    let preview = selected.get(..selected.len().min(24)).unwrap_or(selected);
    format!("{}\n{}", hex_preview(preview), ascii_preview(preview))
}

pub(super) fn manual_branch_lead_id(range: ByteRange, key: u8, generation: u64) -> WorkbenchLeadId {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in range
        .start
        .to_le_bytes()
        .into_iter()
        .chain(range.end.to_le_bytes())
        .chain(generation.to_le_bytes())
        .chain([key])
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    WorkbenchLeadId(hash)
}

pub(super) fn materialize_analytical_cohort(
    selection: &ScreenCohortSelection,
    source_bytes: &[u8],
    generation: u64,
    projection: ProjectionKind,
) -> Result<CohortModel, String> {
    let source = SourceSnapshot {
        source_id: SourceId(1),
        generation: SourceGeneration(generation),
    };
    let mut model = CohortModel::new(source);
    let geometry_code = projection_code(projection);
    let mut offsets = BTreeSet::new();
    for member in &selection.members {
        for offset in member.source_offsets {
            offsets.insert(offset);
        }
    }
    let mut member_ids = Vec::with_capacity(offsets.len());
    for offset in offsets.iter().copied() {
        let offset_u64 = u64::try_from(offset).map_err(|_| "cohort offset overflow".to_owned())?;
        let end = offset_u64
            .checked_add(1)
            .ok_or_else(|| "cohort range overflow".to_owned())?;
        let byte = *source_bytes
            .get(offset)
            .ok_or_else(|| "cohort offset is outside the immutable source".to_owned())?;
        let id = SampledByteId {
            source_id: source.source_id,
            generation: source.generation,
            offset: offset_u64,
        };
        model
            .add_sample(CohortSample {
                id,
                byte,
                position: [
                    i32::from(byte) * 1_000,
                    i32::try_from(offset).map_err(|_| "cohort position overflow".to_owned())?,
                    geometry_code,
                ],
                factors: vec![
                    CohortFactor {
                        name: "byte-value-milli".to_owned(),
                        contribution: i64::from(byte) * 1_000,
                    },
                    CohortFactor {
                        name: "projection-space".to_owned(),
                        contribution: i64::from(geometry_code),
                    },
                ],
                provenance: ExactProvenance {
                    source_id: source.source_id,
                    generation: source.generation,
                    ranges: ByteRangeSet {
                        ranges: vec![
                            ByteRange::new(offset_u64, end).map_err(|error| error.to_string())?,
                        ],
                    },
                },
            })
            .map_err(|error| error.to_string())?;
        member_ids.push(id);
    }
    let mut factors = vec![
        CohortFactor {
            name: "selected-voxels".to_owned(),
            contribution: i64::try_from(selection.metrics.member_count)
                .map_err(|_| "cohort member count overflow".to_owned())?,
        },
        CohortFactor {
            name: "exact-source-bytes".to_owned(),
            contribution: i64::try_from(selection.metrics.unique_byte_count)
                .map_err(|_| "cohort byte count overflow".to_owned())?,
        },
    ];
    if let Some(concentration) = selection.metrics.source_byte_concentration {
        factors.push(CohortFactor {
            name: format!("dominant-byte-0x{:02x}", concentration.byte),
            contribution: i64::try_from(concentration.occurrences)
                .map_err(|_| "cohort concentration overflow".to_owned())?,
        });
    }
    model
        .select_lasso(
            member_ids,
            format!(
                "{} voxels remain linked because they occupied one screen-space cohort in {}.",
                selection.metrics.member_count,
                projection.label()
            ),
            factors,
        )
        .map_err(|error| error.to_string())?;
    Ok(model)
}

pub(super) const fn projection_code(projection: ProjectionKind) -> i32 {
    match projection {
        ProjectionKind::AddressRaster => 0,
        ProjectionKind::Hilbert => 1,
        ProjectionKind::Transitions => 2,
        ProjectionKind::Bitplanes => 3,
        ProjectionKind::Complexity => 4,
        ProjectionKind::Sections => 5,
        ProjectionKind::PolarAddressPath => 6,
        ProjectionKind::HelicalAddressPath => 7,
        ProjectionKind::AlignmentLattice => 8,
        ProjectionKind::RecurrencePlane => 9,
        ProjectionKind::RepetitionSkyline => 10,
        ProjectionKind::SpectralWaterfall => 11,
        ProjectionKind::HammingHypercube => 12,
        ProjectionKind::HierarchicalBlockVolume => 13,
    }
}

pub(super) const fn branch_status_label(status: BranchStatus) -> &'static str {
    match status {
        BranchStatus::Draft => "draft",
        BranchStatus::Active => "active",
        BranchStatus::Pinned => "pinned",
        BranchStatus::Discarded => "discarded",
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn source_offset_x(offset: u64, source_length: usize, rect: egui::Rect) -> f32 {
    let denominator = source_length.max(1) as f32;
    rect.width()
        .mul_add((offset as f32 / denominator).clamp(0.0, 1.0), rect.left())
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn discovery_arc(
    first_x: f32,
    second_x: f32,
    baseline: f32,
    height: f32,
) -> Vec<egui::Pos2> {
    const SEGMENTS: usize = 28;
    (0..=SEGMENTS)
        .map(|segment| {
            let t = segment as f32 / SEGMENTS as f32;
            let x = (second_x - first_x).mul_add(t, first_x);
            let y = (std::f32::consts::PI * t).sin().mul_add(-height, baseline);
            egui::pos2(x, y)
        })
        .collect()
}
