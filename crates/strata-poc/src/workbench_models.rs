//! Deterministic adapters from POC fixtures and transform evaluation into workbench state.
//!
//! This module has no GUI dependencies. It deliberately gives each fixture source a fixed
//! identity so selections created from the bundled demonstrations retain stable provenance.
#![allow(clippy::redundant_pub_crate)] // Parent-only adapters live in a separate binary module.

use strata_analysis::workbench::{ReversibleTransform, TransformEvaluation, WorkbenchLeadId};
use strata_core::{
    ByteRange, ByteRangeSet, DataDomain, Determinism, SourceGeneration, SourceId,
    TransformGraphSpec, TransformNodeId, TransformNodeSpec,
};
use strata_test_support::poc_fixtures::{
    ComparisonTruthKind, InvestigationFixture, InvestigationRegionKind, RevisionPairFixture,
};
use strata_views::{
    investigation::ExactProvenance,
    workbench::{
        BranchId, BranchReversibility, BranchStatus, ComparisonArchaeology,
        ComparisonClassification, ComparisonId, ComparisonPair, ComparisonRegion,
        ComparisonRegionId, HypothesisBranch, LivingRegion, MetricValue, RegionId, RegionKind,
        RegionModel, RegionRelationship, RegionRelationshipId, RegionRelationshipKind,
        SourceSnapshot,
    },
};

/// Stable source identity used for the bundled investigation fixture.
pub(crate) const INVESTIGATION_SOURCE_ID: SourceId = SourceId(1);
/// Stable source identity used for the earlier revision-diff fixture.
pub(crate) const REVISION_BEFORE_SOURCE_ID: SourceId = SourceId(0x5354_5241_5441_0002);
/// Stable source identity used for the later revision-diff fixture.
pub(crate) const REVISION_AFTER_SOURCE_ID: SourceId = SourceId(0x5354_5241_5441_0003);
const MAX_GENERIC_COMPARISON_REGIONS: usize = 2_048;

/// Builds a living-region model from all investigation fixture ground-truth regions.
///
/// The result contains adjacency links between neighboring fixture regions, the exact XOR link,
/// and an internal repeat relationship between the repeated-block region and its motif exemplar.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_region_model(
    fixture: &InvestigationFixture,
    generation: SourceGeneration,
) -> Result<RegionModel, String> {
    let mut model = RegionModel::new();
    for (index, region) in fixture.regions.iter().enumerate() {
        let id = region_id(index)?;
        model
            .add_region(LivingRegion {
                id,
                label: region.name.to_owned(),
                kind: fixture_region_kind(region.kind),
                provenance: fixture_provenance(region.range, generation),
                parent_id: None,
            })
            .map_err(|error| workbench_error(&error))?;
    }

    for index in 1..fixture.regions.len() {
        let previous = fixture
            .regions
            .get(index - 1)
            .ok_or_else(|| "missing previous fixture region".to_owned())?;
        let current = fixture
            .regions
            .get(index)
            .ok_or_else(|| "missing current fixture region".to_owned())?;
        let provenance = fixture_provenance_set(&[previous.range, current.range], generation);
        model
            .add_relationship(RegionRelationship {
                id: RegionRelationshipId(
                    1_000_u128 + u128::try_from(index).map_err(|_| "region index overflow")?,
                ),
                from: region_id(index - 1)?,
                to: region_id(index)?,
                kind: RegionRelationshipKind::Adjacent,
                provenance,
                rationale: "contiguous fixture regions".to_owned(),
            })
            .map_err(|error| workbench_error(&error))?;
    }

    let source_index = fixture
        .regions
        .iter()
        .position(|region| region.kind == InvestigationRegionKind::CorrelatedSource)
        .ok_or_else(|| "fixture lacks XOR source region".to_owned())?;
    let encoded_index = fixture
        .regions
        .iter()
        .position(|region| region.kind == InvestigationRegionKind::XorEncodedCopy)
        .ok_or_else(|| "fixture lacks XOR encoded region".to_owned())?;
    model
        .add_relationship(RegionRelationship {
            id: RegionRelationshipId(2_000),
            from: region_id(source_index)?,
            to: region_id(encoded_index)?,
            kind: RegionRelationshipKind::XorEncoded,
            provenance: fixture_provenance_set(
                &[
                    fixture.xor_copy.source_range,
                    fixture.xor_copy.encoded_range,
                ],
                generation,
            ),
            rationale: format!(
                "exact XOR encoding with key 0x{:02x}",
                fixture.xor_copy.xor_key
            ),
        })
        .map_err(|error| workbench_error(&error))?;

    let repeat_index = fixture
        .regions
        .iter()
        .position(|region| region.kind == InvestigationRegionKind::RepeatedBlock)
        .ok_or_else(|| "fixture lacks repeated block region".to_owned())?;
    let repeated = fixture
        .regions
        .get(repeat_index)
        .ok_or_else(|| "missing repeated block region".to_owned())?;
    let motif_end = repeated
        .range
        .start
        .checked_add(16)
        .ok_or_else(|| "repeat motif range overflow".to_owned())?;
    let motif_range =
        ByteRange::new(repeated.range.start, motif_end).map_err(|error| error.to_string())?;
    let motif_id = RegionId(3_000);
    model
        .add_region(LivingRegion {
            id: motif_id,
            label: "repeated motif exemplar".to_owned(),
            kind: RegionKind::Custom("motif-exemplar".to_owned()),
            provenance: fixture_provenance(motif_range, generation),
            parent_id: Some(region_id(repeat_index)?),
        })
        .map_err(|error| workbench_error(&error))?;
    model
        .add_relationship(RegionRelationship {
            id: RegionRelationshipId(3_001),
            from: region_id(repeat_index)?,
            to: motif_id,
            kind: RegionRelationshipKind::Repeats,
            provenance: fixture_provenance(repeated.range, generation),
            rationale: "sixteen exact motif occurrences in the fixture region".to_owned(),
        })
        .map_err(|error| workbench_error(&error))?;
    Ok(model)
}

/// Builds comparison archaeology from all exact semantic truth records in the revision fixture.
pub(crate) fn build_comparison_archaeology(
    fixture: &RevisionPairFixture,
) -> Result<ComparisonArchaeology, String> {
    let pair = ComparisonPair {
        id: ComparisonId(1),
        left: SourceSnapshot {
            source_id: REVISION_BEFORE_SOURCE_ID,
            generation: SourceGeneration(0),
        },
        right: SourceSnapshot {
            source_id: REVISION_AFTER_SOURCE_ID,
            generation: SourceGeneration(0),
        },
    };
    let mut archaeology = ComparisonArchaeology::new(pair);
    for (index, truth) in fixture.exact_truth.iter().enumerate() {
        let classification = match truth.kind {
            ComparisonTruthKind::Unchanged => ComparisonClassification::Unchanged,
            ComparisonTruthKind::Modified => ComparisonClassification::Modified,
            ComparisonTruthKind::NewlyIntroduced => ComparisonClassification::New,
            ComparisonTruthKind::Moved => ComparisonClassification::Moved,
        };
        let right_range = truth
            .after_range
            .ok_or_else(|| format!("comparison truth {} has no after range", truth.name))?;
        let left = truth.before_range.map(|range| ExactProvenance {
            source_id: REVISION_BEFORE_SOURCE_ID,
            generation: SourceGeneration(0),
            ranges: ByteRangeSet {
                ranges: vec![range],
            },
        });
        archaeology
            .add_region(ComparisonRegion {
                id: ComparisonRegionId(
                    u128::try_from(index + 1).map_err(|_| "comparison index overflow")?,
                ),
                classification,
                left,
                right: ExactProvenance {
                    source_id: REVISION_AFTER_SOURCE_ID,
                    generation: SourceGeneration(0),
                    ranges: ByteRangeSet {
                        ranges: vec![right_range],
                    },
                },
                explanation: truth.name.to_owned(),
            })
            .map_err(|error| workbench_error(&error))?;
    }
    Ok(archaeology)
}

/// Builds a bounded exact-offset comparison for two arbitrary retained sources.
///
/// Each region is an exact range. A `Modified` region means at least one byte in
/// that bounded range differs; the explanation records the exact changed count.
/// A longer right-hand tail is `New`. A left-only tail remains visible in pair
/// length metrics because this POC contract has no removed classification.
pub(crate) fn build_bytewise_comparison(
    left: &[u8],
    right: &[u8],
    left_snapshot: SourceSnapshot,
    right_snapshot: SourceSnapshot,
) -> Result<ComparisonArchaeology, String> {
    let pair = ComparisonPair {
        id: ComparisonId(2),
        left: left_snapshot,
        right: right_snapshot,
    };
    let mut archaeology = ComparisonArchaeology::new(pair);
    let aligned_length = left.len().min(right.len());
    let block_size = aligned_length
        .div_ceil(MAX_GENERIC_COMPARISON_REGIONS.saturating_sub(1))
        .max(1);
    let mut region_id = 1_u128;
    for start in (0..aligned_length).step_by(block_size) {
        let end = start.saturating_add(block_size).min(aligned_length);
        let left_bytes = left
            .get(start..end)
            .ok_or_else(|| "left comparison range is unavailable".to_owned())?;
        let right_bytes = right
            .get(start..end)
            .ok_or_else(|| "right comparison range is unavailable".to_owned())?;
        let changed = left_bytes
            .iter()
            .zip(right_bytes)
            .filter(|(before, after)| before != after)
            .count();
        let classification = if changed == 0 {
            ComparisonClassification::Unchanged
        } else {
            ComparisonClassification::Modified
        };
        let range = ByteRange::new(
            u64::try_from(start).map_err(|_| "comparison start overflow")?,
            u64::try_from(end).map_err(|_| "comparison end overflow")?,
        )
        .map_err(|error| error.to_string())?;
        archaeology
            .add_region(ComparisonRegion {
                id: ComparisonRegionId(region_id),
                classification,
                left: Some(ExactProvenance {
                    source_id: left_snapshot.source_id,
                    generation: left_snapshot.generation,
                    ranges: ByteRangeSet {
                        ranges: vec![range],
                    },
                }),
                right: ExactProvenance {
                    source_id: right_snapshot.source_id,
                    generation: right_snapshot.generation,
                    ranges: ByteRangeSet {
                        ranges: vec![range],
                    },
                },
                explanation: if changed == 0 {
                    format!(
                        "{} exact aligned bytes unchanged",
                        end.saturating_sub(start)
                    )
                } else {
                    format!(
                        "{changed} of {} exact aligned bytes differ",
                        end.saturating_sub(start)
                    )
                },
            })
            .map_err(|error| workbench_error(&error))?;
        region_id = region_id.saturating_add(1);
    }

    if right.len() > aligned_length {
        let range = ByteRange::new(
            u64::try_from(aligned_length).map_err(|_| "comparison tail start overflow")?,
            u64::try_from(right.len()).map_err(|_| "comparison tail end overflow")?,
        )
        .map_err(|error| error.to_string())?;
        archaeology
            .add_region(ComparisonRegion {
                id: ComparisonRegionId(region_id),
                classification: ComparisonClassification::New,
                left: None,
                right: ExactProvenance {
                    source_id: right_snapshot.source_id,
                    generation: right_snapshot.generation,
                    ranges: ByteRangeSet {
                        ranges: vec![range],
                    },
                },
                explanation: format!(
                    "{} exact bytes exist only in comparison source B",
                    right.len().saturating_sub(aligned_length)
                ),
            })
            .map_err(|error| workbench_error(&error))?;
    }
    Ok(archaeology)
}

/// Converts an immutable XOR transform evaluation into a reproducible branch contract.
///
/// `provenance` must contain exactly the range measured by `evaluation`; the caller supplies the
/// source identity and generation because the analysis evaluation intentionally operates on bytes.
pub(crate) fn build_branch_from_evaluation(
    lead_id: WorkbenchLeadId,
    label: String,
    provenance: ExactProvenance,
    evaluation: &TransformEvaluation,
) -> Result<HypothesisBranch, String> {
    require_evaluation_range(&provenance, evaluation.source_range)?;
    let ReversibleTransform::XorByte(key) = evaluation.transform;
    let parameter_json = format!("{{\"key\":{key}}}");
    let transform = TransformGraphSpec {
        nodes: vec![TransformNodeSpec {
            id: TransformNodeId(1),
            kind: "xor-byte".to_owned(),
            input_domain: DataDomain::Bytes,
            output_domain: DataDomain::Bytes,
            parameter_json: parameter_json.clone(),
            determinism: Determinism::Deterministic,
            reversible: true,
            inverse_spec_json: Some(parameter_json),
            loss_model: Some("none".to_owned()),
            implementation_id: "strata-poc.xor-byte.v1".to_owned(),
        }],
        edges: Vec::new(),
    };
    Ok(HypothesisBranch {
        id: BranchId(u128::from(lead_id.0)),
        label,
        parent_id: None,
        provenance,
        transform,
        reversibility: BranchReversibility::Reversible {
            loss_model: "none".to_owned(),
        },
        before_metrics: compact_metric_values(evaluation.before)?,
        after_metrics: compact_metric_values(evaluation.after)?,
        status: BranchStatus::Active,
    })
}

fn fixture_region_kind(kind: InvestigationRegionKind) -> RegionKind {
    match kind {
        InvestigationRegionKind::SignatureHeader => RegionKind::Header,
        InvestigationRegionKind::PreambleText => RegionKind::Text,
        InvestigationRegionKind::FixedWidthTable => RegionKind::Table,
        InvestigationRegionKind::CorrelatedSource | InvestigationRegionKind::XorEncodedCopy => {
            RegionKind::Structural
        }
        InvestigationRegionKind::HighVariationPayload => {
            RegionKind::Custom("high-variation".to_owned())
        }
        InvestigationRegionKind::EmbeddedObject => RegionKind::Custom("embedded-object".to_owned()),
        InvestigationRegionKind::RepeatedBlock => RegionKind::Custom("repeated-block".to_owned()),
        InvestigationRegionKind::ReservedPadding => RegionKind::Padding,
    }
}

fn fixture_provenance(range: ByteRange, generation: SourceGeneration) -> ExactProvenance {
    fixture_provenance_set(&[range], generation)
}

fn fixture_provenance_set(ranges: &[ByteRange], generation: SourceGeneration) -> ExactProvenance {
    ExactProvenance {
        source_id: INVESTIGATION_SOURCE_ID,
        generation,
        ranges: ByteRangeSet {
            ranges: ranges.to_vec(),
        },
    }
}

fn region_id(index: usize) -> Result<RegionId, String> {
    Ok(RegionId(
        u128::try_from(index + 1).map_err(|_| "region index overflow")?,
    ))
}

fn require_evaluation_range(
    provenance: &ExactProvenance,
    source_range: ByteRange,
) -> Result<(), String> {
    if provenance.ranges.ranges.len() != 1
        || provenance.ranges.ranges.first() != Some(&source_range)
    {
        return Err("branch provenance must exactly match the evaluated source range".to_owned());
    }
    Ok(())
}

fn compact_metric_values(
    metrics: strata_analysis::workbench::CompactMetrics,
) -> Result<Vec<MetricValue>, String> {
    Ok(vec![
        MetricValue {
            name: "byte_count".to_owned(),
            value: i64::try_from(metrics.byte_count)
                .map_err(|_| "byte count cannot fit fixed-point metric".to_owned())?,
        },
        MetricValue {
            name: "distinct_byte_count".to_owned(),
            value: i64::from(metrics.distinct_byte_count),
        },
        MetricValue {
            name: "entropy_microbits".to_owned(),
            value: fixed_point(metrics.entropy_bits, 1_000_000)?,
        },
        MetricValue {
            name: "zero_fraction_ppm".to_owned(),
            value: fixed_point(metrics.zero_fraction, 1_000_000)?,
        },
        MetricValue {
            name: "text_likelihood_ppm".to_owned(),
            value: fixed_point(metrics.text_likelihood, 1_000_000)?,
        },
    ])
}

fn fixed_point(value: f64, scale: u64) -> Result<i64, String> {
    if !value.is_finite() {
        return Err("metric is not finite".to_owned());
    }
    let scaled = value
        * f64::from(u32::try_from(scale).map_err(|_| "metric scale exceeds f64 adapter limit")?);
    format!("{scaled:.0}")
        .parse::<i64>()
        .map_err(|_| "fixed-point metric conversion failed".to_owned())
}

fn workbench_error(error: &strata_views::workbench::WorkbenchError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use strata_analysis::workbench::{ReversibleTransform, evaluate_transform_candidate};
    use strata_core::{ByteRange, SourceGeneration, SourceId};
    use strata_test_support::poc_fixtures::{aligned_revision_pair, investigation_binary};
    use strata_views::{
        investigation::ExactProvenance,
        workbench::{ComparisonClassification, RegionRelationshipKind},
    };

    use super::{
        INVESTIGATION_SOURCE_ID, build_branch_from_evaluation, build_bytewise_comparison,
        build_comparison_archaeology, build_region_model,
    };

    #[test]
    fn fixture_regions_include_adjacency_xor_and_repeat_relationships() -> Result<(), String> {
        let fixture = investigation_binary().map_err(|error| error.to_string())?;
        let model = build_region_model(&fixture, SourceGeneration(5))?;
        assert_eq!(model.regions().len(), fixture.regions.len() + 1);
        assert!(
            model
                .relationships()
                .iter()
                .any(|relationship| relationship.kind == RegionRelationshipKind::Adjacent)
        );
        assert!(
            model
                .relationships()
                .iter()
                .any(|relationship| relationship.kind == RegionRelationshipKind::XorEncoded)
        );
        assert!(
            model
                .relationships()
                .iter()
                .any(|relationship| relationship.kind == RegionRelationshipKind::Repeats)
        );
        Ok(())
    }

    #[test]
    fn revision_truth_maps_all_four_archaeology_classes() -> Result<(), String> {
        let fixture = aligned_revision_pair().map_err(|error| error.to_string())?;
        let archaeology = build_comparison_archaeology(&fixture)?;
        assert_eq!(archaeology.regions().len(), 4);
        for classification in [
            ComparisonClassification::Unchanged,
            ComparisonClassification::Modified,
            ComparisonClassification::New,
            ComparisonClassification::Moved,
        ] {
            assert!(
                archaeology
                    .regions()
                    .iter()
                    .any(|region| region.classification == classification)
            );
        }
        Ok(())
    }

    #[test]
    fn arbitrary_pair_is_bounded_and_retains_exact_ranges() -> Result<(), String> {
        let mut right = vec![0_u8; 10_000];
        right[4_096] = 7;
        right.extend_from_slice(&[8, 9, 10]);
        let archaeology = build_bytewise_comparison(
            &vec![0_u8; 10_000],
            &right,
            strata_views::workbench::SourceSnapshot {
                source_id: SourceId(1),
                generation: SourceGeneration(2),
            },
            strata_views::workbench::SourceSnapshot {
                source_id: SourceId(2),
                generation: SourceGeneration(3),
            },
        )?;
        assert!(archaeology.regions().len() <= 2_048);
        assert!(archaeology.regions().iter().any(|region| {
            region.classification == ComparisonClassification::Modified && region.left.is_some()
        }));
        assert!(archaeology.regions().iter().any(|region| {
            region.classification == ComparisonClassification::New && region.left.is_none()
        }));
        Ok(())
    }

    #[test]
    fn transform_evaluation_becomes_reproducible_xor_branch() -> Result<(), String> {
        let fixture = investigation_binary().map_err(|error| error.to_string())?;
        let range = fixture.xor_copy.encoded_range;
        let evaluation = evaluate_transform_candidate(
            &fixture.bytes,
            range,
            ReversibleTransform::XorByte(fixture.xor_copy.xor_key),
        )
        .map_err(|error| error.to_string())?;
        let provenance = ExactProvenance {
            source_id: INVESTIGATION_SOURCE_ID,
            generation: SourceGeneration(7),
            ranges: strata_core::ByteRangeSet {
                ranges: vec![
                    ByteRange::new(range.start, range.end).map_err(|error| error.to_string())?,
                ],
            },
        };
        let branch = build_branch_from_evaluation(
            strata_analysis::workbench::WorkbenchLeadId(42),
            "decode XOR copy".to_owned(),
            provenance,
            &evaluation,
        )?;
        assert_eq!(branch.transform.nodes.len(), 1);
        assert!(branch.transform.nodes[0].reversible);
        assert_eq!(branch.before_metrics.len(), 5);
        assert_eq!(branch.after_metrics.len(), 5);
        Ok(())
    }
}
