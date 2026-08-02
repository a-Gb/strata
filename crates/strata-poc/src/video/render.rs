//! Deterministic software rasterization with restrained, data-preserving display transforms.

use crate::projection::{ProjectionColorFeature, ProjectionKind, ProjectionSample};

use super::program::{
    AnimationLook, AnimationPalette, AnimationPrimitive, AnimationProgram, AnimationState,
};

#[derive(Debug, Clone, Copy)]
struct RenderedPoint {
    x: f32,
    y: f32,
    depth: f32,
    radius: f32,
    color: [u8; 4],
}

pub(super) fn render_frame(
    program: &AnimationProgram,
    samples: &[ProjectionSample],
    state: AnimationState,
) -> Result<Vec<u8>, String> {
    let pixel_count = u64::from(program.width)
        .checked_mul(u64::from(program.height))
        .and_then(|count| count.checked_mul(4))
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| "video frame allocation overflowed".to_owned())?;
    let mut pixels = vec![0_u8; pixel_count];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[0] = program.background[0];
        pixel[1] = program.background[1];
        pixel[2] = program.background[2];
        pixel[3] = 255;
    }

    if program.show_guides && program.look.guide_opacity > 0.0 {
        draw_guides(
            &mut pixels,
            program.width,
            program.height,
            state,
            program.look.guide_opacity,
        );
    }

    let focus = focus_position(program, samples, state);
    let bitplane_instances = program.composition.is_some_and(|composition| {
        composition.projection_a == ProjectionKind::Bitplanes
            || composition.projection_b == ProjectionKind::Bitplanes
    });
    let mut rendered =
        Vec::with_capacity(
            samples
                .len()
                .saturating_mul(if bitplane_instances { 8 } else { 1 }),
        );
    for sample in samples {
        for plane in 0..=if bitplane_instances { 7 } else { 0 } {
            let bit_plane = bitplane_instances.then_some(plane);
            let Some((x, y, depth, depth_scale)) = project_position(
                animation_position(program, sample, state, bit_plane),
                focus,
                program.width,
                program.height,
                state,
                program.perspective,
            ) else {
                continue;
            };
            let depth_radius = (depth + 1.0).mul_add(0.12, 0.9);
            let size_scale = program.composition.map_or(1.0, |composition| {
                sample.size_scale(composition.channels.size)
            });
            rendered.push(RenderedPoint {
                x,
                y,
                depth,
                radius: (program.point_size * size_scale * depth_scale * depth_radius)
                    .clamp(0.35, 12.0),
                color: grade_at_depth(
                    animation_color(program, sample, state),
                    program.look,
                    program.brightness,
                    depth,
                ),
            });
        }
    }
    rendered.sort_by(|first, second| first.depth.total_cmp(&second.depth));
    for point in rendered {
        draw_primitive(
            &mut pixels,
            program.width,
            program.height,
            point,
            program.look.primitive,
        );
    }
    if program.look.vignette > 0.0 {
        apply_vignette(
            &mut pixels,
            program.width,
            program.height,
            program.look.vignette,
        );
    }
    Ok(pixels)
}

#[allow(clippy::cast_precision_loss)]
fn focus_position(
    program: &AnimationProgram,
    samples: &[ProjectionSample],
    state: AnimationState,
) -> [f32; 3] {
    let Some(offset) = state.focus_offset else {
        return [0.0; 3];
    };
    samples
        .iter()
        .min_by(|first, second| {
            let first_distance = (first.source_offsets[0] as f64 - offset).abs();
            let second_distance = (second.source_offsets[0] as f64 - offset).abs();
            first_distance.total_cmp(&second_distance)
        })
        .map_or([0.0; 3], |sample| {
            animation_position(program, sample, state, None)
        })
}

fn animation_position(
    program: &AnimationProgram,
    sample: &ProjectionSample,
    state: AnimationState,
    bit_plane: Option<u8>,
) -> [f32; 3] {
    program.composition.map_or_else(
        || sample.position_at(state.morph),
        |composition| {
            sample.morphed_position_instance(
                composition.projection_a,
                composition.projection_b,
                composition.parameters,
                None,
                state.morph / 3.0,
                composition.channels.height,
                1.0,
                bit_plane,
            )
        },
    )
}

fn animation_color(
    program: &AnimationProgram,
    sample: &ProjectionSample,
    state: AnimationState,
) -> [u8; 4] {
    let (source, feature) = program.composition.map_or_else(
        || (sample.color_at(state.morph), ProjectionColorFeature::Value),
        |composition| {
            (
                sample.color_for(composition.channels.color),
                composition.channels.color,
            )
        },
    );
    let signal = sample.color_signal(feature);
    palette_color(program.look.palette, signal, source)
}

#[allow(clippy::suboptimal_flops, clippy::cast_precision_loss)]
fn project_position(
    point: [f32; 3],
    focus: [f32; 3],
    width: u32,
    height: u32,
    state: AnimationState,
    perspective: f32,
) -> Option<(f32, f32, f32, f32)> {
    let point = [
        point[0] - focus[0],
        point[1] - focus[1],
        point[2] - focus[2],
    ];
    let yaw_cosine = state.yaw.cos();
    let yaw_sine = state.yaw.sin();
    let pitch_cosine = state.pitch.cos();
    let pitch_sine = state.pitch.sin();
    let yaw_x = (point[0] * yaw_cosine) + (point[2] * yaw_sine);
    let yaw_z = (-point[0] * yaw_sine) + (point[2] * yaw_cosine);
    let rotated_y = (point[1] * pitch_cosine) - (yaw_z * pitch_sine);
    let depth = (point[1] * pitch_sine) + (yaw_z * pitch_cosine);
    let camera_distance = 3.2;
    let denominator = camera_distance - depth;
    if denominator <= 0.01 {
        return None;
    }
    let raw_perspective = camera_distance / denominator;
    let depth_scale = 1.0 + ((raw_perspective - 1.0) * perspective.clamp(0.0, 1.0));
    let screen_scale = width.min(height) as f32 * 0.39 * state.zoom * depth_scale;
    let x = (width as f32 * 0.5) + (yaw_x * screen_scale);
    let y = (height as f32 * 0.5) - (rotated_y * screen_scale);
    (x.is_finite() && y.is_finite()).then_some((x, y, depth, depth_scale))
}

pub(super) fn palette_color(palette: AnimationPalette, signal: f32, source: [u8; 4]) -> [u8; 4] {
    let stops = match palette {
        AnimationPalette::Source => return source,
        AnimationPalette::Cividis => &CIVIDIS[..],
        AnimationPalette::CyanAmber => &CYAN_AMBER[..],
        AnimationPalette::Monochrome => &MONOCHROME[..],
    };
    let rgb = gradient_color(stops, signal);
    [rgb[0], rgb[1], rgb[2], source[3]]
}

const CIVIDIS: [[u8; 3]; 6] = [
    [0, 32, 76],
    [40, 74, 108],
    [78, 110, 115],
    [124, 143, 109],
    [181, 177, 92],
    [253, 234, 69],
];
const CYAN_AMBER: [[u8; 3]; 5] = [
    [4, 15, 28],
    [18, 82, 105],
    [31, 166, 174],
    [227, 225, 192],
    [244, 162, 43],
];
const MONOCHROME: [[u8; 3]; 4] = [[2, 7, 15], [37, 51, 65], [113, 130, 139], [235, 239, 233]];

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn gradient_color(stops: &[[u8; 3]], signal: f32) -> [u8; 3] {
    let maximum = stops.len().saturating_sub(1);
    let position = signal.clamp(0.0, 1.0) * maximum as f32;
    let first_index = (position.floor() as usize).min(maximum);
    let second_index = first_index.saturating_add(1).min(maximum);
    let amount = position - first_index as f32;
    let first = stops[first_index];
    let second = stops[second_index];
    std::array::from_fn(|channel| {
        let first = srgb_decode(first[channel]);
        let second = srgb_decode(second[channel]);
        srgb_encode((second - first).mul_add(amount, first))
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn grade_at_depth(color: [u8; 4], look: AnimationLook, brightness: f32, depth: f32) -> [u8; 4] {
    let depth_light = (depth + 1.0).mul_add(0.10, 0.88).clamp(0.62, 1.12);
    let mut linear = [
        srgb_decode(color[0]),
        srgb_decode(color[1]),
        srgb_decode(color[2]),
    ];
    let luminance = linear[0].mul_add(0.2126, linear[1].mul_add(0.7152, linear[2] * 0.0722));
    for channel in &mut linear {
        *channel = (*channel - luminance).mul_add(look.saturation, luminance);
        *channel = (*channel - 0.18).mul_add(look.contrast, 0.18) * brightness * depth_light;
    }
    let alpha = (f32::from(color[3]) * brightness.clamp(0.35, 1.35))
        .clamp(30.0, 235.0)
        .round() as u8;
    [
        srgb_encode(linear[0]),
        srgb_encode(linear[1]),
        srgb_encode(linear[2]),
        alpha,
    ]
}

fn srgb_decode(value: u8) -> f32 {
    let signal = f32::from(value) / 255.0;
    if signal <= 0.04045 {
        signal / 12.92
    } else {
        ((signal + 0.055) / 1.055).powf(2.4)
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn srgb_encode(linear: f32) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let signal = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        linear.powf(1.0 / 2.4).mul_add(1.055, -0.055)
    };
    (signal * 255.0).round() as u8
}

#[cfg(test)]
pub(super) fn relative_luminance(color: [u8; 4]) -> f32 {
    srgb_decode(color[0]).mul_add(
        0.2126,
        srgb_decode(color[1]).mul_add(0.7152, srgb_decode(color[2]) * 0.0722),
    )
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn draw_guides(pixels: &mut [u8], width: u32, height: u32, state: AnimationState, opacity: f32) {
    let center_x = width as f32 * 0.5;
    let center_y = height as f32 * 0.5;
    let minimum = width.min(height) as f32;
    let alpha = (35.0 * opacity.clamp(0.0, 1.0)).round() as u8;
    for radius_scale in [0.18, 0.31, 0.44] {
        draw_ring(
            pixels,
            width,
            height,
            center_x,
            center_y,
            minimum * radius_scale * state.zoom,
            [42, 88, 128, alpha],
        );
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_ring(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: [u8; 4],
) {
    const SEGMENTS: u32 = 360;
    for segment in 0..SEGMENTS {
        let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        draw_glyph(
            pixels,
            width,
            height,
            angle.cos().mul_add(radius, center_x),
            angle.sin().mul_add(radius, center_y),
            0.75,
            color,
            AnimationPrimitive::Disc,
        );
    }
}

fn draw_primitive(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    point: RenderedPoint,
    primitive: AnimationPrimitive,
) {
    draw_glyph(
        pixels,
        width,
        height,
        point.x,
        point.y,
        point.radius,
        point.color,
        primitive,
    );
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    clippy::too_many_arguments
)]
fn draw_glyph(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: [u8; 4],
    primitive: AnimationPrimitive,
) {
    if !center_x.is_finite() || !center_y.is_finite() || radius <= 0.0 {
        return;
    }
    let minimum_x = (center_x - radius - 1.0).floor().max(0.0) as u32;
    let maximum_x = (center_x + radius + 1.0)
        .ceil()
        .min(width.saturating_sub(1) as f32) as u32;
    let minimum_y = (center_y - radius - 1.0).floor().max(0.0) as u32;
    let maximum_y = (center_y + radius + 1.0)
        .ceil()
        .min(height.saturating_sub(1) as f32) as u32;
    for y in minimum_y..=maximum_y {
        for x in minimum_x..=maximum_x {
            let delta_x = x as f32 + 0.5 - center_x;
            let delta_y = y as f32 + 0.5 - center_y;
            let distance = match primitive {
                AnimationPrimitive::Disc => (delta_x.mul_add(delta_x, delta_y * delta_y)).sqrt(),
                AnimationPrimitive::Voxel => delta_x.abs().max(delta_y.abs()),
            };
            let coverage = (radius + 0.75 - distance).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let Some(index) = (y as usize)
                .checked_mul(width as usize)
                .and_then(|row| row.checked_add(x as usize))
                .and_then(|pixel| pixel.checked_mul(4))
            else {
                continue;
            };
            let Some(target) = pixels.get_mut(index..index.saturating_add(4)) else {
                continue;
            };
            blend_pixel(target, color, coverage);
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn blend_pixel(target: &mut [u8], source: [u8; 4], coverage: f32) {
    let alpha = (f32::from(source[3]) / 255.0) * coverage;
    let inverse = 1.0 - alpha;
    for channel in 0..3 {
        target[channel] = f32::from(source[channel])
            .mul_add(alpha, f32::from(target[channel]) * inverse)
            .clamp(0.0, 255.0)
            .round() as u8;
    }
    target[3] = 255;
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn apply_vignette(pixels: &mut [u8], width: u32, height: u32, strength: f32) {
    let center_x = width as f32 * 0.5;
    let center_y = height as f32 * 0.5;
    let inverse_x = 1.0 / center_x.max(1.0);
    let inverse_y = 1.0 / center_y.max(1.0);
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let x = (index % width as usize) as f32 + 0.5;
        let y = (index / width as usize) as f32 + 0.5;
        let normalized_x = (x - center_x) * inverse_x;
        let normalized_y = (y - center_y) * inverse_y;
        let radius = normalized_x
            .mul_add(normalized_x, normalized_y * normalized_y)
            .sqrt();
        let edge = smooth_step(((radius - 0.42) / 0.72).clamp(0.0, 1.0));
        let scale = (edge * strength.clamp(0.0, 1.0)).mul_add(-0.42, 1.0);
        for channel in &mut pixel[..3] {
            *channel = (f32::from(*channel) * scale).round() as u8;
        }
    }
}

fn smooth_step(value: f32) -> f32 {
    value * value * 2.0f32.mul_add(-value, 3.0)
}
