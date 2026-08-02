use super::export::{attach_p1_video_features, projection_source};
use super::presets::animation_presets;
use super::program::{
    AnimationEasing, AnimationKeyframe, AnimationPalette, AnimationPrimitive, AnimationProgram,
    AnimationSourceRange,
};
use super::render::{palette_color, relative_luminance, render_frame};
use crate::projection::{
    ProjectionComposition, ProjectionKind, ProjectionSamplingConfig, sample_projection_samples,
    sample_projection_samples_with_config,
};

#[test]
fn example_program_is_valid_and_has_exact_frame_count() {
    let program = AnimationProgram::example();
    assert_eq!(program.validate(), Ok(120));
}

#[test]
fn timeline_interpolation_hits_endpoints_and_midpoint() {
    let mut program = AnimationProgram::example();
    program.easing = AnimationEasing::Linear;
    for (keyframe, focus_offset) in program.keyframes.iter_mut().zip([100, 200, 300]) {
        keyframe.focus_offset = Some(focus_offset);
    }
    let start = program.state_at(0.0);
    let middle = program.state_at(0.5);
    let end = program.state_at(1.0);
    assert!(start.is_ok());
    assert!(middle.is_ok());
    assert!(end.is_ok());
    if let (Ok(start), Ok(middle), Ok(end)) = (start, middle, end) {
        assert!((start.morph - 0.0).abs() < f32::EPSILON);
        assert!((middle.morph - 3.0).abs() < f32::EPSILON);
        assert!((end.morph - 0.0).abs() < f32::EPSILON);
        assert_eq!(start.focus_offset, Some(100.0));
        assert_eq!(middle.focus_offset, Some(200.0));
        assert_eq!(end.focus_offset, Some(300.0));
    }
}

#[test]
fn validation_rejects_partial_focus_timeline() {
    let mut program = AnimationProgram::example();
    program.keyframes[0].focus_offset = Some(12);
    assert!(program.validate().is_err());
}

#[test]
fn source_range_selects_exact_bytes_and_base_offset() {
    let mut program = AnimationProgram::example();
    program.source_range = Some(AnimationSourceRange { start: 2, end: 6 });
    let source = projection_source(&program, &[0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(source, Ok((&[2, 3, 4, 5][..], 2)));
}

#[test]
fn validation_rejects_non_monotonic_keyframes() {
    let mut program = AnimationProgram::example();
    program.keyframes.insert(
        2,
        AnimationKeyframe {
            at: 0.25,
            morph: 1.0,
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
            focus_offset: None,
        },
    );
    assert!(program.validate().is_err());
}

#[test]
fn json_round_trip_preserves_program() {
    let program = AnimationProgram::example();
    let encoded = serde_json::to_vec(&program);
    assert!(encoded.is_ok());
    if let Ok(encoded) = encoded {
        let decoded = serde_json::from_slice::<AnimationProgram>(&encoded);
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert_eq!(decoded, program);
        }
    }
}

#[test]
fn old_programs_default_to_the_source_disc_look() {
    let program = AnimationProgram::example();
    let value = serde_json::to_value(program);
    assert!(value.is_ok());
    if let Ok(mut value) = value {
        if let Some(object) = value.as_object_mut() {
            object.remove("look");
        }
        let decoded = serde_json::from_value::<AnimationProgram>(value);
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert_eq!(decoded.look.palette, AnimationPalette::Source);
            assert_eq!(decoded.look.primitive, AnimationPrimitive::Disc);
        }
    }
}

#[test]
fn software_frame_is_deterministic_and_non_black() {
    let mut program = AnimationProgram::example();
    program.width = 96;
    program.height = 64;
    program.point_budget = 64;
    program.look.primitive = AnimationPrimitive::Voxel;
    program.look.palette = AnimationPalette::Cividis;
    let samples = sample_projection_samples(&(0_u8..=255).collect::<Vec<_>>(), 1, 64);
    let state = program.state_at(0.25);
    assert!(state.is_ok());
    if let Ok(state) = state {
        let first = render_frame(&program, &samples, state);
        let second = render_frame(&program, &samples, state);
        assert_eq!(first, second);
        if let Ok(frame) = first {
            assert!(frame.chunks_exact(4).any(|pixel| pixel[..3] != [0, 0, 0]));
        }
    }
}

#[test]
fn sequential_palettes_raise_linear_light_luminance() {
    for palette in [AnimationPalette::Cividis, AnimationPalette::Monochrome] {
        let mut previous = -f32::EPSILON;
        for step in 0_u8..=32 {
            let signal = f32::from(step) / 32.0;
            let color = palette_color(palette, signal, [0, 0, 0, 200]);
            let luminance = relative_luminance(color);
            assert!(luminance + 0.000_01 >= previous, "{palette:?} regressed");
            previous = luminance;
        }
    }
}

#[test]
fn curated_presets_are_distinct_valid_and_fixture_correlated() {
    let presets = animation_presets();
    assert_eq!(presets.len(), 4);
    for preset in &presets {
        assert!(
            preset.program.validate().is_ok(),
            "{} is invalid",
            preset.id
        );
        assert_eq!(preset.program.source, preset.fixture);
        assert!(!preset.title.is_empty());
        assert!(!preset.reveals.is_empty());
        assert!(preset.program.composition.is_some());
        assert_eq!(preset.program.look.primitive, AnimationPrimitive::Voxel);
    }
    for pair in presets.windows(2) {
        assert_ne!(pair[0].id, pair[1].id);
        assert_ne!(pair[0].program.composition, pair[1].program.composition);
    }
}

#[test]
fn p1_composition_attaches_analytical_coordinates_before_render() {
    let bytes = b"0123456789abcdef----0123456789abcdef";
    let composition = ProjectionComposition {
        projection_a: ProjectionKind::RecurrencePlane,
        projection_b: ProjectionKind::HammingHypercube,
        ..ProjectionComposition::default()
    };
    let mut samples = sample_projection_samples_with_config(
        bytes,
        0,
        ProjectionSamplingConfig::from(composition),
        64,
    );
    assert!(samples.iter().all(|sample| sample.p1_feature().is_none()));
    let attached = attach_p1_video_features(&mut samples, bytes, 0, composition);
    assert!(attached.is_ok());
    assert!(samples.iter().all(|sample| sample.p1_feature().is_some()));
}
