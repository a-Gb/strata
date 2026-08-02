use strata_analysis::projection_p1::P1FeatureRecord;
use strata_core::ByteRange;
use strata_test_support::projection_fixtures::projection_golden_fixtures;

use super::{
    ProjectionComposition, ProjectionDomain, ProjectionGeometry, ProjectionKind,
    ProjectionParameters, ProjectionSample, ProjectionSamplingConfig, local_entropy,
    morph_position, sample_projection_sample_at_source_offset, sample_projection_samples,
    sample_projection_samples_at_offset, sample_projection_samples_with_config,
};

fn synthetic_sample() -> ProjectionSample {
    ProjectionSample {
        positions: [
            [-1.0, -0.5, 0.0],
            [0.25, 0.5, -0.25],
            [0.5, -0.75, 0.25],
            [0.75, 0.9, -0.5],
        ],
        terrain_flat: [0.75, 0.0, -0.5],
        colors: [[10, 20, 30, 100], [210, 120, 60, 200]],
        bytes: [10, 20, 30],
        relative_offset: 0,
        source_length: 64,
        entropy: 0.5,
        change_rate: 0.25,
        unique_fraction: 0.75,
        analysis_range: [0, 3],
        point_id: 0,
        source_offsets: [0, 1, 2],
        p1: None,
    }
}

fn assert_position_close(actual: [f32; 3], expected: [f32; 3]) {
    for (actual_component, expected_component) in actual.into_iter().zip(expected) {
        assert!((actual_component - expected_component).abs() < f32::EPSILON);
    }
}

#[test]
fn projection_samples_retain_exact_contributor_offsets() {
    let samples = sample_projection_samples(&[0, 32, 64, 96, 128, 255], 1, 16);
    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].source_offsets, [0, 1, 2]);
    assert_eq!(samples[3].source_offsets, [3, 4, 5]);
    assert!((samples[0].position_at(0.0)[0] + 1.0).abs() < f32::EPSILON);
}

#[test]
fn ranged_samples_retain_absolute_source_offsets() {
    let samples = sample_projection_samples_at_offset(&[4, 8, 15, 16, 23, 42], 20_849_808, 1, 16);
    assert_eq!(samples.len(), 4);
    assert_eq!(
        samples[0].source_offsets,
        [20_849_808, 20_849_809, 20_849_810]
    );
    assert_eq!(
        samples[3].source_offsets,
        [20_849_811, 20_849_812, 20_849_813]
    );
}

#[test]
fn projection_budget_is_deterministic_and_bounded() {
    let bytes: Vec<u8> = (0_u8..=255).cycle().take(10_000).collect();
    let first = sample_projection_samples(&bytes, 4, 257);
    let second = sample_projection_samples(&bytes, 4, 257);
    assert!(first.len() <= 257);
    assert_eq!(first, second);
}

#[test]
fn sparse_evidence_offset_can_be_pinned_outside_uniform_sampling() -> Result<(), String> {
    let bytes = (0_u8..=255).cycle().take(1_024).collect::<Vec<_>>();
    let uniform = sample_projection_samples(&bytes, 1, 8);
    assert!(!uniform.iter().any(|sample| sample.point_id == 511));
    let pinned = sample_projection_sample_at_source_offset(
        &bytes,
        0,
        bytes.len(),
        ProjectionSamplingConfig::legacy(1),
        511,
    )
    .ok_or("evidence offset should produce a sample")?;
    assert_eq!(pinned.point_id, 511);
    assert_eq!(pinned.source_offsets, [511, 512, 513]);
    assert_eq!(pinned.analysis_range, [511, 514]);
    let end_pinned = sample_projection_sample_at_source_offset(
        &bytes,
        0,
        bytes.len(),
        ProjectionSamplingConfig::legacy(1),
        1_023,
    )
    .ok_or("end evidence offset should clamp into the last exact sample")?;
    assert_eq!(end_pinned.source_offsets, [1_021, 1_022, 1_023]);
    Ok(())
}

#[test]
fn morph_hits_each_named_projection_exactly() {
    let triplet = [-1.0, 0.0, 1.0];
    let orbit = [0.2, 0.4, 0.6];
    let sequence = [1.0, -1.0, 0.5];
    let terrain = [-0.75, 0.9, 0.25];
    assert_position_close(
        morph_position(triplet, orbit, sequence, terrain, 0.0),
        triplet,
    );
    assert_position_close(
        morph_position(triplet, orbit, sequence, terrain, 1.0),
        orbit,
    );
    assert_position_close(
        morph_position(triplet, orbit, sequence, terrain, 2.0),
        sequence,
    );
    assert_position_close(
        morph_position(triplet, orbit, sequence, terrain, 3.0),
        terrain,
    );
}

#[test]
fn entropy_terrain_distinguishes_uniform_and_varied_windows() {
    let uniform = [0_u8; 64];
    let varied: Vec<u8> = (0_u8..64).collect();
    assert!(local_entropy(&uniform, 0) < f32::EPSILON);
    assert!(local_entropy(&varied, 0) > 0.7);
}

#[test]
fn entropy_relief_is_independent_from_geometry_morph() {
    let sample = synthetic_sample();

    assert_position_close(
        sample.position_with_relief(0.0, 0.0),
        sample.position_with_relief(0.0, 1.0),
    );
    assert_position_close(sample.position_with_relief(3.0, 0.0), [0.75, 0.0, -0.5]);
    assert_position_close(sample.position_with_relief(3.0, 1.0), [0.75, 0.9, -0.5]);
    assert_position_close(sample.position_at(3.0), [0.75, 0.9, -0.5]);
}

#[test]
fn colour_lens_hits_both_independent_endpoints() {
    let sample = synthetic_sample();

    assert_eq!(sample.color_with_mix(0.0), sample.colors[0]);
    assert_eq!(sample.color_with_mix(1.0), sample.colors[1]);
    assert_eq!(sample.color_with_mix(-1.0), sample.colors[0]);
    assert_eq!(sample.color_with_mix(2.0), sample.colors[1]);
}

#[test]
fn six_basic_projections_are_finite_and_deterministic() {
    let sample = synthetic_sample();
    let parameters = ProjectionParameters::default();
    for projection in ProjectionKind::BASIC {
        let first = sample.position_for(projection, parameters, None, 1.0);
        let second = sample.position_for(projection, parameters, None, 1.0);
        assert_position_close(first, second);
        assert!(first.into_iter().all(f32::is_finite));
    }
}

#[test]
fn p1_projection_catalog_uses_attached_evidence_coordinates() {
    let mut sample = synthetic_sample();
    let feature = P1FeatureRecord {
        point_id: sample.point_id,
        alignment: [-0.9, -0.8, -0.7],
        recurrence: [-0.6, -0.5, -0.4],
        repetition: [-0.3, -0.2, -0.1],
        spectrum: [0.1, 0.2, 0.3],
        hypercube: [0.4, 0.5, 0.6],
        hierarchy: [0.7, 0.8, 0.9],
        partner_range: ByteRange::new(8, 16).ok(),
        recurrence_score: 0.75,
        match_length: 16,
        dominant_frequency_bin: 4,
        spectral_magnitude: 0.5,
        hierarchy_depth: 3,
    };
    sample.attach_p1(feature);
    for (projection, expected) in [
        (ProjectionKind::AlignmentLattice, feature.alignment),
        (ProjectionKind::RecurrencePlane, feature.recurrence),
        (ProjectionKind::RepetitionSkyline, feature.repetition),
        (ProjectionKind::SpectralWaterfall, feature.spectrum),
        (ProjectionKind::HammingHypercube, feature.hypercube),
        (ProjectionKind::HierarchicalBlockVolume, feature.hierarchy),
    ] {
        assert_position_close(
            sample.position_for(projection, ProjectionParameters::default(), None, 0.0),
            expected,
        );
    }
}

#[test]
fn every_p1_projection_round_trips_in_composition_json() {
    for projection in ProjectionKind::P1 {
        let composition = ProjectionComposition {
            projection_a: projection,
            projection_b: ProjectionKind::Hilbert,
            ..ProjectionComposition::default()
        };
        let encoded = serde_json::to_vec(&composition);
        assert!(encoded.is_ok());
        if let Ok(encoded) = encoded {
            let decoded = serde_json::from_slice::<ProjectionComposition>(&encoded);
            assert!(decoded.is_ok());
            if let Ok(decoded) = decoded {
                assert_eq!(decoded, composition);
            }
        }
    }
}

#[test]
fn morph_keeps_stable_identity_and_exact_range() {
    let sample = synthetic_sample();
    let identity = (
        sample.point_id,
        sample.source_offsets,
        sample.exact_analysis_range(),
    );
    for mix in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let position = sample.morphed_position(
            ProjectionKind::Hilbert,
            ProjectionKind::Complexity,
            ProjectionParameters::default(),
            None,
            mix,
            1.0,
        );
        assert!(position.into_iter().all(f32::is_finite));
        assert_eq!(
            (
                sample.point_id,
                sample.source_offsets,
                sample.exact_analysis_range()
            ),
            identity
        );
    }
}

#[test]
fn window_domain_reports_the_full_analyzed_range() {
    let bytes: Vec<u8> = (0_u8..=255).collect();
    let config = ProjectionSamplingConfig {
        domain: ProjectionDomain::Window,
        lag: 1,
        window_bytes: 32,
        hop_bytes: 16,
        aggregation_bytes: 64,
        word_bits: 32,
    };
    let samples = sample_projection_samples_with_config(&bytes, 1_000, config, 32);
    assert_eq!(
        samples.first().map(|sample| sample.exact_analysis_range()),
        Some([1_000, 1_032])
    );
    assert_eq!(
        samples.get(1).map(|sample| sample.exact_analysis_range()),
        Some([1_016, 1_048])
    );
}

#[test]
fn default_composition_is_valid_and_orthogonal() {
    let composition = ProjectionComposition::default();
    assert_eq!(composition.projection_a, ProjectionKind::Hilbert);
    assert_eq!(composition.geometry, ProjectionGeometry::Voxels);
    assert_eq!(composition.validate(), Ok(()));
}

#[test]
fn golden_binary_classes_map_through_every_basic_projection() {
    let composition = ProjectionComposition::default();
    let config = ProjectionSamplingConfig::from(composition);
    for fixture in projection_golden_fixtures() {
        let first = sample_projection_samples_with_config(&fixture.bytes, 0, config, 128);
        let second = sample_projection_samples_with_config(&fixture.bytes, 0, config, 128);
        assert!(!first.is_empty(), "{} produced no samples", fixture.id);
        assert_eq!(first, second, "{} sampled nondeterministically", fixture.id);
        for sample in first {
            let range = sample.exact_analysis_range();
            assert!(range[0] < range[1]);
            for projection in ProjectionKind::BASIC {
                let position = sample.position_for(projection, composition.parameters, None, 1.0);
                assert!(
                    position.into_iter().all(f32::is_finite),
                    "{} generated a non-finite {} position",
                    fixture.id,
                    projection.label()
                );
            }
        }
    }
}
