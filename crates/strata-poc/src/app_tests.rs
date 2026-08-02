use super::{
    LARGE_SOURCE_PREVIEW_BYTES, MAX_CONTIGUOUS_SOURCE_BYTES, ProjectionCompareMode,
    ProjectionComposition, ProjectionGeometry, ProjectionKind, ProjectionLabelState,
    ProjectionSlot, RAIL_SEGMENT_GAP, ScreenProjection, ascii_preview, build_investigation_model,
    closest_screen_point, hex_preview, image_pixel, open_local_source,
    open_local_source_with_focus, projection_context_color, projection_footer_labels,
    projection_phase_for_mix, projection_voxel_rect, rail_segment_width,
};
use eframe::egui;
use std::{
    error::Error,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
    time::{SystemTime, UNIX_EPOCH},
};
use strata_analysis::workbench::{
    ReversibleTransform, WorkbenchConfig, WorkbenchEvidence, WorkbenchLeadKind, analyze_workbench,
};
use strata_core::{ByteRange, SourceGeneration, SourceId};
use strata_test_support::poc_fixtures::investigation_binary;

#[test]
fn previews_are_bounded_and_readable() {
    assert_eq!(hex_preview(&[0, 16, 255]), "00 10 ff");
    assert_eq!(ascii_preview(b"A\nB"), "A.B");
}

#[test]
fn image_coordinates_resolve_inside_bounds() {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 50.0));
    assert_eq!(
        image_pixel(egui::pos2(50.0, 25.0), rect, [10, 5]),
        Some((5, 2))
    );
    assert_eq!(image_pixel(egui::pos2(101.0, 25.0), rect, [10, 5]), None);
}

#[test]
fn named_projection_destinations_are_distinct() {
    assert_eq!(ProjectionKind::BASIC.len(), 6);
    assert_eq!(ProjectionKind::BASIC[0].short_label(), "Raster");
    assert_eq!(ProjectionKind::BASIC[5].short_label(), "Sections");
    for mix in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let phase = projection_phase_for_mix(mix);
        let reconstructed = phase.sin().mul_add(0.5, 0.5);
        assert!((reconstructed - mix).abs() < 0.000_01);
    }
}

#[test]
fn projection_defaults_to_exact_voxels_without_a_field() {
    let geometry = ProjectionGeometry::default();
    assert_eq!(geometry, ProjectionGeometry::Voxels);
    assert_eq!(geometry.label(), "VOXELS");
    assert!(!geometry.uses_field());
    assert_eq!(geometry.field_alpha(), 0);
    assert!(ProjectionGeometry::Surface.field_alpha() > 0);
}

#[test]
fn rail_segments_fit_their_allocated_width() {
    let available = 304.0;
    for option_count in [2_u16, 3, 4] {
        let segment = rail_segment_width(available, usize::from(option_count));
        let occupied = segment * f32::from(option_count)
            + RAIL_SEGMENT_GAP * f32::from(option_count.saturating_sub(1));
        assert!(occupied <= available);
        assert!(segment > 0.0);
    }
    assert!(rail_segment_width(available, 0).abs() < f32::EPSILON);
}

#[test]
fn dense_projection_labels_remain_bounded() {
    let state = ProjectionLabelState {
        point_count: 1_022,
        composition: ProjectionComposition {
            projection_a: ProjectionKind::Hilbert,
            projection_b: ProjectionKind::Complexity,
            geometry: ProjectionGeometry::Surface,
            compare_mode: ProjectionCompareMode::Morph,
            mix: 0.35,
            ..ProjectionComposition::default()
        },
        relief: 1.0,
        context_light: 0.72,
        field_radius: 64.0,
        field_exposure: 6.0,
        field_contours: true,
    };
    let (full_left, full_right) = projection_footer_labels(state, false);
    assert_eq!(
        full_left,
        "A HILBERT > B COMPLEXITY / MORPH 35% / SURFACE R64 G6.0 +C"
    );
    assert_eq!(
        full_right,
        "WINDOW / C:ADDR H:ENTROPY S:UNIFORM O:SELECTION"
    );
    let (compact_left, compact_right) = projection_footer_labels(state, true);
    assert_eq!(compact_left, "HILBERT / MORPH");
    assert_eq!(compact_right, "ADDR / H100 / C72");
}

#[test]
fn projection_voxel_glyph_is_square_and_pixel_aligned_in_size() {
    let center = egui::pos2(12.25, 18.75);
    let voxel = projection_voxel_rect(center, 1.8);
    assert_eq!(voxel.center(), center);
    assert!((voxel.width() - voxel.height()).abs() < f32::EPSILON);
    assert!((voxel.width() - 4.0).abs() < f32::EPSILON);
}

#[test]
fn projection_pick_prefers_front_most_visible_voxel() {
    let back = ScreenProjection {
        position: egui::pos2(10.0, 10.0),
        depth: -0.5,
        radius: 2.0,
        color: egui::Color32::WHITE,
        point_id: 1,
        source_offsets: [1, 2, 3],
        analysis_range: [1, 4],
        slot: ProjectionSlot::A,
        bit_plane: None,
        region_slot: None,
        p1: None,
    };
    let front = ScreenProjection {
        position: egui::pos2(12.0, 10.0),
        depth: 0.5,
        radius: 2.0,
        color: egui::Color32::WHITE,
        point_id: 4,
        source_offsets: [4, 5, 6],
        analysis_range: [4, 7],
        slot: ProjectionSlot::A,
        bit_plane: None,
        region_slot: None,
        p1: None,
    };
    let picked = closest_screen_point(&[back, front], egui::pos2(10.0, 10.0));
    assert_eq!(picked.map(|point| point.source_offsets), Some([4, 5, 6]));
}

#[test]
fn large_sparse_source_uses_bounded_tiles_and_exact_focus() -> Result<(), Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!(
        "strata-large-source-{}-{nonce}.bin",
        std::process::id()
    ));
    let source_length = MAX_CONTIGUOUS_SOURCE_BYTES.saturating_add(1024 * 1024);
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)?;
    file.set_len(source_length)?;
    file.write_all(b"STRATA-TILE-START")?;
    let focus_start = source_length.saturating_sub(128 * 1024);
    file.seek(SeekFrom::Start(focus_start))?;
    file.write_all(b"STRATA-TILE-FOCUS")?;
    file.sync_all()?;
    drop(file);

    let overview = open_local_source(&path, SourceId(41), SourceGeneration(2))
        .map_err(std::io::Error::other)?;
    assert_eq!(overview.source_length, source_length);
    assert_eq!(
        overview.bytes.len(),
        usize::try_from(LARGE_SOURCE_PREVIEW_BYTES)?
    );
    assert!(overview.sampled_overview);
    assert!(overview.resident_tiles.len() <= 64);
    assert!(overview.resident_bytes <= 16 * 1024 * 1024);
    drop(overview);

    let focus = ByteRange::new(focus_start, focus_start.saturating_add(32))?;
    let refined =
        open_local_source_with_focus(&path, SourceId(41), SourceGeneration(2), Some(focus))
            .map_err(std::io::Error::other)?;
    assert!(refined.resident_tiles.iter().any(|tile| {
        tile.key.precision == strata_analysis::tiles::TilePrecision::Exact
            && tile.coverage.start <= focus.start
            && tile.coverage.end >= focus.end
    }));
    assert!(refined.resident_bytes <= 16 * 1024 * 1024);
    drop(refined);
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn lens_labels_and_context_light_are_bounded() {
    let source = egui::Color32::from_rgba_unmultiplied(200, 100, 50, 160);
    assert_eq!(
        projection_context_color(source, 0.5),
        egui::Color32::from_rgba_premultiplied(63, 32, 16, 80)
    );
    assert_eq!(projection_context_color(source, 2.0), source);
}

#[test]
fn discovery_fixture_becomes_an_exact_navigable_xor_hypothesis() -> Result<(), String> {
    let fixture = investigation_binary().map_err(|error| error.to_string())?;
    let report = analyze_workbench(&fixture.bytes, WorkbenchConfig::default())
        .map_err(|error| error.to_string())?;
    for kind in [
        WorkbenchLeadKind::EntropyBoundary,
        WorkbenchLeadKind::ExactRepeat,
        WorkbenchLeadKind::Periodicity,
        WorkbenchLeadKind::EmbeddedSignature,
        WorkbenchLeadKind::TransformCandidate,
    ] {
        assert!(report.leads.iter().any(|finding| finding.kind == kind));
    }
    let finding = report
        .leads
        .iter()
        .find(|finding| {
            matches!(
                finding.evidence,
                WorkbenchEvidence::XorCorrelatedTransform { .. }
            )
        })
        .ok_or_else(|| "fixture should expose an XOR-linked region".to_owned())?;
    assert_eq!(
        finding.source_ranges,
        vec![
            fixture.xor_copy.source_range,
            fixture.xor_copy.encoded_range
        ]
    );
    assert!(matches!(
        finding.evidence,
        WorkbenchEvidence::XorCorrelatedTransform {
            transform: ReversibleTransform::XorByte(key),
            ..
        } if key == fixture.xor_copy.xor_key
    ));

    let model = build_investigation_model(&report.leads, 0).map_err(|error| error.to_string())?;
    assert!(!model.correlations().is_empty());
    assert!(!model.hypotheses().is_empty());
    Ok(())
}
