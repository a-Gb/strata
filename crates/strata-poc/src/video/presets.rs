//! Curated, source-correlated animation programs with restrained display transforms.

use crate::projection::{
    ProjectionChannels, ProjectionColorFeature, ProjectionCompareMode, ProjectionComposition,
    ProjectionDimensions, ProjectionDomain, ProjectionGeometry, ProjectionHeightFeature,
    ProjectionKind, ProjectionOpacityFeature, ProjectionOverlays, ProjectionParameters,
    ProjectionSizeFeature,
};

use super::program::{
    AnimationEasing, AnimationKeyframe, AnimationLook, AnimationPalette, AnimationPrimitive,
    AnimationProgram, PROGRAM_VERSION,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AnimationPreset {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    pub(crate) reveals: &'static str,
    pub(crate) fixture: &'static str,
    pub(crate) program: AnimationProgram,
}

pub(crate) fn animation_presets() -> Vec<AnimationPreset> {
    vec![
        AnimationPreset {
            id: "firmware-stratigraphy",
            title: "Firmware Stratigraphy",
            reveals: "padding, text, table, and high-complexity region boundaries",
            fixture: "fixtures/video/composite-firmware-v1.bin",
            program: firmware_stratigraphy(),
        },
        AnimationPreset {
            id: "xor-correlation-reveal",
            title: "XOR Correlation Reveal",
            reveals: "address-separated source/XOR regions converging in fixed complexity space",
            fixture: "fixtures/video/investigation-xor-v1.bin",
            program: xor_correlation_reveal(),
        },
        AnimationPreset {
            id: "interleave-lattice",
            title: "Interleave Lattice",
            reveals: "six-byte RGB records, row periodicity, and alignment candidates",
            fixture: "fixtures/video/interleaved-sensor-v1.bin",
            program: interleave_lattice(),
        },
        AnimationPreset {
            id: "bitplane-blueprint",
            title: "Bitplane Blueprint",
            reveals: "planar image gradients as eight address-stable bit layers",
            fixture: "fixtures/video/bitplane-image-v1.bin",
            program: bitplane_blueprint(),
        },
    ]
}

pub(crate) fn animation_preset(id: &str) -> Option<AnimationPreset> {
    animation_presets()
        .into_iter()
        .find(|preset| preset.id == id)
}

pub(super) fn default_program() -> AnimationProgram {
    AnimationProgram {
        version: PROGRAM_VERSION,
        source: "demo://composite-firmware".to_owned(),
        source_range: None,
        output: "output/strata-morph.mp4".to_owned(),
        width: 1_280,
        height: 720,
        fps: 30,
        duration_seconds: 4.0,
        composition: None,
        stride: 1,
        point_budget: 12_000,
        point_size: 1.8,
        brightness: 1.25,
        perspective: 0.72,
        background: [0, 0, 0],
        show_guides: true,
        look: AnimationLook::default(),
        easing: AnimationEasing::SmoothStep,
        overwrite: false,
        keyframes: vec![
            keyframe(0.0, 0.0, -0.72, 0.38, 0.92, None),
            keyframe(0.5, 3.0, std::f32::consts::PI - 0.72, -0.18, 1.08, None),
            keyframe(1.0, 0.0, std::f32::consts::TAU - 0.72, 0.38, 0.92, None),
        ],
    }
}

fn firmware_stratigraphy() -> AnimationProgram {
    let parameters = ProjectionParameters {
        dimensions: ProjectionDimensions::Three,
        row_width: 64,
        curve_order: 4,
        aggregation_bytes: 1,
        window_bytes: 32,
        hop_bytes: 1,
        ..ProjectionParameters::default()
    };
    curated_program(
        "fixtures/video/composite-firmware-v1.bin",
        "output/video/firmware-stratigraphy.mp4",
        ProjectionComposition {
            domain: ProjectionDomain::Window,
            projection_a: ProjectionKind::Hilbert,
            projection_b: ProjectionKind::Complexity,
            geometry: ProjectionGeometry::Voxels,
            compare_mode: ProjectionCompareMode::Morph,
            mix: 0.0,
            parameters,
            channels: ProjectionChannels {
                color: ProjectionColorFeature::Address,
                height: ProjectionHeightFeature::Entropy,
                size: ProjectionSizeFeature::ChangeRate,
                opacity: ProjectionOpacityFeature::Uniform,
            },
            overlays: ProjectionOverlays::default(),
        },
        AnimationLook {
            palette: AnimationPalette::Cividis,
            primitive: AnimationPrimitive::Voxel,
            contrast: 0.96,
            saturation: 0.92,
            vignette: 0.12,
            guide_opacity: 0.08,
        },
        [3, 6, 10],
        2.4,
        1.32,
        0.55,
        vec![
            keyframe(0.0, 0.0, -0.48, 0.32, 0.90, None),
            keyframe(0.22, 0.35, 0.62, 0.16, 1.04, None),
            keyframe(0.5, 1.6, 1.72, -0.08, 1.16, None),
            keyframe(0.76, 3.0, 3.02, 0.24, 1.04, None),
            keyframe(1.0, 0.0, 5.80, 0.32, 0.90, None),
        ],
    )
}

fn xor_correlation_reveal() -> AnimationProgram {
    let parameters = ProjectionParameters {
        dimensions: ProjectionDimensions::Three,
        row_width: 64,
        curve_order: 5,
        aggregation_bytes: 2,
        window_bytes: 32,
        hop_bytes: 4,
        ..ProjectionParameters::default()
    };
    curated_program(
        "fixtures/video/investigation-xor-v1.bin",
        "output/video/xor-correlation-reveal.mp4",
        ProjectionComposition {
            domain: ProjectionDomain::Window,
            projection_a: ProjectionKind::Hilbert,
            projection_b: ProjectionKind::Complexity,
            geometry: ProjectionGeometry::Voxels,
            compare_mode: ProjectionCompareMode::Morph,
            mix: 0.0,
            parameters,
            channels: ProjectionChannels {
                color: ProjectionColorFeature::Address,
                height: ProjectionHeightFeature::None,
                size: ProjectionSizeFeature::Entropy,
                opacity: ProjectionOpacityFeature::Uniform,
            },
            overlays: ProjectionOverlays::default(),
        },
        AnimationLook {
            palette: AnimationPalette::CyanAmber,
            primitive: AnimationPrimitive::Voxel,
            contrast: 0.98,
            saturation: 0.94,
            vignette: 0.12,
            guide_opacity: 0.08,
        },
        [3, 7, 13],
        2.25,
        1.26,
        0.60,
        vec![
            keyframe(0.0, 0.0, -0.42, 0.34, 0.86, None),
            keyframe(0.22, 0.55, 0.66, 0.12, 1.02, None),
            keyframe(0.48, 1.65, 1.76, -0.10, 1.16, None),
            keyframe(0.74, 3.0, 3.26, 0.20, 1.08, None),
            keyframe(1.0, 3.0, 5.90, 0.28, 0.90, None),
        ],
    )
}

fn interleave_lattice() -> AnimationProgram {
    let parameters = ProjectionParameters {
        dimensions: ProjectionDimensions::Three,
        row_width: 144,
        curve_order: 5,
        aggregation_bytes: 6,
        window_bytes: 24,
        hop_bytes: 2,
        word_bits: 16,
        little_endian: true,
        alignment_stride: 6,
        alignment_max_stride: 48,
        ..ProjectionParameters::default()
    };
    curated_program(
        "fixtures/video/interleaved-sensor-v1.bin",
        "output/video/interleave-lattice.mp4",
        ProjectionComposition {
            domain: ProjectionDomain::Byte,
            projection_a: ProjectionKind::AddressRaster,
            projection_b: ProjectionKind::AlignmentLattice,
            geometry: ProjectionGeometry::Voxels,
            compare_mode: ProjectionCompareMode::Morph,
            mix: 0.0,
            parameters,
            channels: ProjectionChannels {
                color: ProjectionColorFeature::Value,
                height: ProjectionHeightFeature::None,
                size: ProjectionSizeFeature::Uniform,
                opacity: ProjectionOpacityFeature::Uniform,
            },
            overlays: ProjectionOverlays::default(),
        },
        AnimationLook {
            palette: AnimationPalette::Cividis,
            primitive: AnimationPrimitive::Voxel,
            contrast: 0.88,
            saturation: 0.95,
            vignette: 0.10,
            guide_opacity: 0.0,
        },
        [3, 7, 12],
        2.2,
        1.30,
        0.55,
        vec![
            keyframe(0.0, 0.0, 0.0, 0.0, 0.86, None),
            keyframe(0.28, 0.45, 0.72, 0.18, 1.02, None),
            keyframe(0.55, 1.8, 1.58, -0.14, 1.10, None),
            keyframe(0.8, 3.0, 2.42, 0.22, 1.02, None),
            keyframe(1.0, 3.0, 3.18, 0.08, 0.88, None),
        ],
    )
}

fn bitplane_blueprint() -> AnimationProgram {
    let parameters = ProjectionParameters {
        dimensions: ProjectionDimensions::Three,
        row_width: 32,
        curve_order: 4,
        aggregation_bytes: 1,
        lag: 1,
        window_bytes: 16,
        hop_bytes: 1,
        ..ProjectionParameters::default()
    };
    curated_program(
        "fixtures/video/bitplane-image-v1.bin",
        "output/video/bitplane-blueprint.mp4",
        ProjectionComposition {
            domain: ProjectionDomain::Byte,
            projection_a: ProjectionKind::AddressRaster,
            projection_b: ProjectionKind::Bitplanes,
            geometry: ProjectionGeometry::Voxels,
            compare_mode: ProjectionCompareMode::Morph,
            mix: 0.0,
            parameters,
            channels: ProjectionChannels {
                color: ProjectionColorFeature::Value,
                height: ProjectionHeightFeature::None,
                size: ProjectionSizeFeature::Uniform,
                opacity: ProjectionOpacityFeature::Uniform,
            },
            overlays: ProjectionOverlays::default(),
        },
        AnimationLook {
            palette: AnimationPalette::Monochrome,
            primitive: AnimationPrimitive::Voxel,
            contrast: 1.18,
            saturation: 0.32,
            vignette: 0.18,
            guide_opacity: 0.0,
        },
        [2, 5, 9],
        1.35,
        1.10,
        0.48,
        vec![
            keyframe(0.0, 0.0, -0.36, 0.04, 0.78, None),
            keyframe(0.30, 0.45, 0.64, 0.16, 0.96, None),
            keyframe(0.58, 2.1, 1.84, -0.10, 1.10, None),
            keyframe(0.82, 3.0, 3.78, 0.20, 0.94, None),
            keyframe(1.0, 3.0, 5.92, 0.32, 0.78, None),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn curated_program(
    source: &str,
    output: &str,
    composition: ProjectionComposition,
    look: AnimationLook,
    background: [u8; 3],
    point_size: f32,
    brightness: f32,
    perspective: f32,
    keyframes: Vec<AnimationKeyframe>,
) -> AnimationProgram {
    AnimationProgram {
        version: PROGRAM_VERSION,
        source: source.to_owned(),
        source_range: None,
        output: output.to_owned(),
        width: 1_280,
        height: 720,
        fps: 30,
        duration_seconds: 5.0,
        composition: Some(composition),
        stride: 1,
        point_budget: 20_000,
        point_size,
        brightness,
        perspective,
        background,
        show_guides: look.guide_opacity > 0.0,
        look,
        easing: AnimationEasing::SmoothStep,
        overwrite: true,
        keyframes,
    }
}

const fn keyframe(
    at: f32,
    morph: f32,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    focus_offset: Option<u64>,
) -> AnimationKeyframe {
    AnimationKeyframe {
        at,
        morph,
        yaw,
        pitch,
        zoom,
        focus_offset,
    }
}
