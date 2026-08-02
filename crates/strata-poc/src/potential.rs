//! Deterministic CPU reference renderer for shader-ready projection potential fields.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::redundant_pub_crate
)]

use eframe::egui;

const FIELD_LONG_SIDE: usize = 320;
const FIELD_SHORT_SIDE_MINIMUM: usize = 80;
const FINE_BLUR_PASSES: usize = 3;
const AMBIENT_BLUR_PASSES: usize = 3;
const AMBIENT_WEIGHT: f32 = 0.06;
const SUPPORT_FLOOR_SOURCES: f32 = 0.85;
const FULL_SCALE_SOURCES: f32 = 12.0;
const TONE_LOOKUP_STEPS: usize = 1_024;

type FieldCell = [f32; 4];

#[derive(Debug, Clone, Copy)]
struct ToneSample {
    color: [f32; 3],
    tint_amount: f32,
    contour: f32,
}

/// One projected sample contributing density and colour to a potential field.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PotentialPoint {
    position: egui::Pos2,
    color: egui::Color32,
    strength: f32,
}

impl PotentialPoint {
    /// Creates one screen-space potential source.
    pub(crate) const fn new(position: egui::Pos2, color: egui::Color32, strength: f32) -> Self {
        Self {
            position,
            color,
            strength,
        }
    }
}

/// Adjustable potential-field appearance parameters.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PotentialSettings {
    radius_points: f32,
    exposure: f32,
    contours: bool,
}

impl PotentialSettings {
    /// Creates bounded settings for the reference renderer.
    pub(crate) const fn new(radius_points: f32, exposure: f32, contours: bool) -> Self {
        Self {
            radius_points,
            exposure,
            contours,
        }
    }
}

/// Accumulates projected points into a multi-scale, tone-mapped heat field.
pub(crate) fn render_potential_field(
    rect: egui::Rect,
    points: &[PotentialPoint],
    settings: PotentialSettings,
) -> egui::ColorImage {
    let [width, height] = field_dimensions(rect);
    let cell_count = width.saturating_mul(height);
    let mut deposited = vec![[0.0; 4]; cell_count];
    for point in points {
        deposit_point(&mut deposited, width, height, rect, *point);
    }

    let field_scale = width as f32 / rect.width().max(1.0);
    let fine_radius = ((settings.radius_points.clamp(4.0, 96.0) * field_scale) / 3.0)
        .round()
        .clamp(1.0, 18.0) as usize;
    let ambient_radius = fine_radius.saturating_mul(2).clamp(2, 28);

    let mut fine = deposited.clone();
    let mut ambient = deposited;
    blur_field(&mut fine, width, height, fine_radius, FINE_BLUR_PASSES);
    blur_field(
        &mut ambient,
        width,
        height,
        ambient_radius,
        AMBIENT_BLUR_PASSES,
    );

    let single_source_density = isolated_source_density(fine_radius, ambient_radius);
    let tone_lookup = build_tone_lookup(settings.exposure, settings.contours);
    let pixels = fine
        .iter()
        .zip(&ambient)
        .map(|(fine_cell, ambient_cell)| {
            potential_pixel(
                *fine_cell,
                *ambient_cell,
                single_source_density,
                &tone_lookup,
            )
        })
        .collect();

    egui::ColorImage::new([width, height], pixels)
}

fn field_dimensions(rect: egui::Rect) -> [usize; 2] {
    let width = rect.width().max(1.0);
    let height = rect.height().max(1.0);
    if width >= height {
        let scaled_height = (FIELD_LONG_SIDE as f32 * height / width)
            .round()
            .clamp(FIELD_SHORT_SIDE_MINIMUM as f32, FIELD_LONG_SIDE as f32)
            as usize;
        [FIELD_LONG_SIDE, scaled_height]
    } else {
        let scaled_width = (FIELD_LONG_SIDE as f32 * width / height)
            .round()
            .clamp(FIELD_SHORT_SIDE_MINIMUM as f32, FIELD_LONG_SIDE as f32)
            as usize;
        [scaled_width, FIELD_LONG_SIDE]
    }
}

fn deposit_point(
    cells: &mut [FieldCell],
    width: usize,
    height: usize,
    rect: egui::Rect,
    point: PotentialPoint,
) {
    if !rect.contains(point.position) || width == 0 || height == 0 {
        return;
    }
    let normalized_x = ((point.position.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    let normalized_y = ((point.position.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
    let grid_x = normalized_x * width.saturating_sub(1) as f32;
    let grid_y = normalized_y * height.saturating_sub(1) as f32;
    let x0 = grid_x.floor() as usize;
    let y0 = grid_y.floor() as usize;
    let x1 = x0.saturating_add(1).min(width.saturating_sub(1));
    let y1 = y0.saturating_add(1).min(height.saturating_sub(1));
    let x_fraction = grid_x - x0 as f32;
    let y_fraction = grid_y - y0 as f32;
    let rgba = point.color.to_srgba_unmultiplied();
    let source_color = [
        f32::from(rgba[0]) / 255.0,
        f32::from(rgba[1]) / 255.0,
        f32::from(rgba[2]) / 255.0,
    ];

    for (x, x_weight) in [(x0, 1.0 - x_fraction), (x1, x_fraction)] {
        for (y, y_weight) in [(y0, 1.0 - y_fraction), (y1, y_fraction)] {
            let weight = point.strength.max(0.0) * x_weight * y_weight;
            let index = y.saturating_mul(width).saturating_add(x);
            let Some(cell) = cells.get_mut(index) else {
                continue;
            };
            cell[0] += weight;
            cell[1] = source_color[0].mul_add(weight, cell[1]);
            cell[2] = source_color[1].mul_add(weight, cell[2]);
            cell[3] = source_color[2].mul_add(weight, cell[3]);
        }
    }
}

fn blur_field(cells: &mut [FieldCell], width: usize, height: usize, radius: usize, passes: usize) {
    if cells.is_empty() || width == 0 || height == 0 || radius == 0 || passes == 0 {
        return;
    }
    let mut scratch = vec![[0.0; 4]; cells.len()];
    for _ in 0..passes {
        box_blur_horizontal(cells, &mut scratch, width, height, radius);
        box_blur_vertical(&scratch, cells, width, height, radius);
    }
}

fn box_blur_horizontal(
    source: &[FieldCell],
    target: &mut [FieldCell],
    width: usize,
    height: usize,
    radius: usize,
) {
    let initial_end = radius.min(width.saturating_sub(1));
    for y in 0..height {
        let row_start = y.saturating_mul(width);
        let mut sum = [0.0; 4];
        let mut count = 0_usize;
        for sample_x in 0..=initial_end {
            if let Some(cell) = source.get(row_start.saturating_add(sample_x)) {
                add_cell(&mut sum, *cell);
                count = count.saturating_add(1);
            }
        }
        for x in 0..width {
            if x > 0 {
                let add_x = x.saturating_add(radius);
                if add_x < width
                    && let Some(cell) = source.get(row_start.saturating_add(add_x))
                {
                    add_cell(&mut sum, *cell);
                    count = count.saturating_add(1);
                }
                if let Some(remove_x) = x.checked_sub(radius.saturating_add(1))
                    && let Some(cell) = source.get(row_start.saturating_add(remove_x))
                {
                    subtract_cell(&mut sum, *cell);
                    count = count.saturating_sub(1);
                }
            }
            if let Some(cell) = target.get_mut(row_start.saturating_add(x)) {
                *cell = scaled_cell(sum, count);
            }
        }
    }
}

fn box_blur_vertical(
    source: &[FieldCell],
    target: &mut [FieldCell],
    width: usize,
    height: usize,
    radius: usize,
) {
    let initial_end = radius.min(height.saturating_sub(1));
    for x in 0..width {
        let mut sum = [0.0; 4];
        let mut count = 0_usize;
        for sample_y in 0..=initial_end {
            if let Some(cell) = source.get(sample_y.saturating_mul(width).saturating_add(x)) {
                add_cell(&mut sum, *cell);
                count = count.saturating_add(1);
            }
        }
        for y in 0..height {
            if y > 0 {
                let add_y = y.saturating_add(radius);
                if add_y < height
                    && let Some(cell) = source.get(add_y.saturating_mul(width).saturating_add(x))
                {
                    add_cell(&mut sum, *cell);
                    count = count.saturating_add(1);
                }
                if let Some(remove_y) = y.checked_sub(radius.saturating_add(1))
                    && let Some(cell) = source.get(remove_y.saturating_mul(width).saturating_add(x))
                {
                    subtract_cell(&mut sum, *cell);
                    count = count.saturating_sub(1);
                }
            }
            if let Some(cell) = target.get_mut(y.saturating_mul(width).saturating_add(x)) {
                *cell = scaled_cell(sum, count);
            }
        }
    }
}

fn add_cell(sum: &mut FieldCell, cell: FieldCell) {
    for (sum_channel, cell_channel) in sum.iter_mut().zip(cell) {
        *sum_channel += cell_channel;
    }
}

fn subtract_cell(sum: &mut FieldCell, cell: FieldCell) {
    for (sum_channel, cell_channel) in sum.iter_mut().zip(cell) {
        *sum_channel -= cell_channel;
    }
}

fn scaled_cell(sum: FieldCell, count: usize) -> FieldCell {
    let reciprocal = 1.0 / count.max(1) as f32;
    sum.map(|channel| channel * reciprocal)
}

fn isolated_source_density(fine_radius: usize, ambient_radius: usize) -> f32 {
    let fine_response = central_box_response(fine_radius, FINE_BLUR_PASSES);
    let ambient_response = central_box_response(ambient_radius, AMBIENT_BLUR_PASSES);
    ambient_response
        .powi(2)
        .mul_add(AMBIENT_WEIGHT, fine_response.powi(2))
}

fn central_box_response(radius: usize, passes: usize) -> f32 {
    let kernel_width = radius.saturating_mul(2).saturating_add(1);
    let reciprocal = 1.0 / kernel_width.max(1) as f32;
    let mut response = vec![1.0_f32];
    for _ in 0..passes {
        let mut blurred = vec![0.0; response.len().saturating_add(radius.saturating_mul(2))];
        for (index, value) in response.iter().copied().enumerate() {
            for offset in 0..kernel_width {
                if let Some(sample) = blurred.get_mut(index.saturating_add(offset)) {
                    *sample = value.mul_add(reciprocal, *sample);
                }
            }
        }
        response = blurred;
    }
    response
        .get(response.len() / 2)
        .copied()
        .unwrap_or_default()
}

fn potential_pixel(
    fine: FieldCell,
    ambient: FieldCell,
    single_source_density: f32,
    tone_lookup: &[ToneSample],
) -> egui::Color32 {
    let density = ambient[0].mul_add(AMBIENT_WEIGHT, fine[0]);
    if single_source_density <= f32::EPSILON || density <= f32::EPSILON {
        return egui::Color32::from_rgb(1, 2, 7);
    }
    let equivalent_sources = density / single_source_density;
    let normalized = ((equivalent_sources - SUPPORT_FLOOR_SOURCES)
        / (FULL_SCALE_SOURCES - SUPPORT_FLOOR_SOURCES))
        .clamp(0.0, 1.0);
    if normalized <= f32::EPSILON {
        return egui::Color32::from_rgb(1, 2, 7);
    }
    let lookup_index = (normalized * TONE_LOOKUP_STEPS as f32)
        .round()
        .clamp(0.0, TONE_LOOKUP_STEPS as f32) as usize;
    let Some(tone) = tone_lookup.get(lookup_index).copied() else {
        return egui::Color32::from_rgb(1, 2, 7);
    };
    let source_tint = [
        ambient[1].mul_add(AMBIENT_WEIGHT, fine[1]) / density,
        ambient[2].mul_add(AMBIENT_WEIGHT, fine[2]) / density,
        ambient[3].mul_add(AMBIENT_WEIGHT, fine[3]) / density,
    ];
    let mut color = mix_rgb(tone.color, source_tint, tone.tint_amount);
    color = mix_rgb(color, [0.004, 0.012, 0.018], tone.contour);
    rgb_color(color)
}

fn build_tone_lookup(exposure: f32, contours: bool) -> Vec<ToneSample> {
    let exposure = exposure.clamp(0.25, 8.0);
    let maximum_response = 1.0 - (-exposure).exp();
    (0..=TONE_LOOKUP_STEPS)
        .map(|step| {
            let normalized = step as f32 / TONE_LOOKUP_STEPS as f32;
            let shaped_density = normalized.powf(1.18);
            let response = 1.0 - (-exposure * shaped_density).exp();
            let tone = (response / maximum_response.max(f32::EPSILON)).clamp(0.0, 1.0);
            ToneSample {
                color: potential_palette(tone),
                tint_amount: tone * tone * 0.025,
                contour: contour_strength(normalized, tone, contours),
            }
        })
        .collect()
}

fn potential_palette(tone: f32) -> [f32; 3] {
    let tone = tone.clamp(0.0, 1.0);
    if tone < 0.20 {
        return mix_rgb([0.004, 0.008, 0.020], [0.012, 0.030, 0.050], tone / 0.20);
    }
    if tone < 0.50 {
        return mix_rgb(
            [0.012, 0.030, 0.050],
            [0.025, 0.115, 0.150],
            (tone - 0.20) / 0.30,
        );
    }
    if tone < 0.78 {
        return mix_rgb(
            [0.025, 0.115, 0.150],
            [0.055, 0.300, 0.340],
            (tone - 0.50) / 0.28,
        );
    }
    if tone < 0.94 {
        return mix_rgb(
            [0.055, 0.300, 0.340],
            [0.150, 0.500, 0.500],
            (tone - 0.78) / 0.16,
        );
    }
    mix_rgb(
        [0.150, 0.500, 0.500],
        [0.410, 0.720, 0.670],
        (tone - 0.94) / 0.06,
    )
}

fn contour_strength(normalized: f32, tone: f32, enabled: bool) -> f32 {
    if !enabled || tone < 0.28 {
        return 0.0;
    }
    let band = normalized * 6.0;
    let distance = (band - band.round()).abs();
    (1.0 - (distance / 0.045)).clamp(0.0, 1.0) * 0.24 * tone
}

fn mix_rgb(first: [f32; 3], second: [f32; 3], amount: f32) -> [f32; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        (second[0] - first[0]).mul_add(amount, first[0]),
        (second[1] - first[1]).mul_add(amount, first[1]),
        (second[2] - first[2]).mul_add(amount, first[2]),
    ]
}

fn rgb_color(color: [f32; 3]) -> egui::Color32 {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgb(channel(color[0]), channel(color[1]), channel(color[2]))
}

#[cfg(test)]
mod tests {
    use super::{
        PotentialPoint, PotentialSettings, field_dimensions, potential_palette,
        render_potential_field,
    };
    use eframe::egui;

    #[test]
    fn field_dimensions_are_bounded_and_aspect_aware() {
        let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 600.0));
        let tall = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 900.0));
        assert_eq!(field_dimensions(wide), [320, 160]);
        assert_eq!(field_dimensions(tall), [107, 320]);
    }

    #[test]
    fn potential_field_is_deterministic_and_peaks_near_sources() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0));
        let color = egui::Color32::from_rgb(42, 210, 255);
        let points = [
            PotentialPoint::new(rect.center() + egui::vec2(-4.0, -3.0), color, 1.0),
            PotentialPoint::new(rect.center() + egui::vec2(3.0, -4.0), color, 1.0),
            PotentialPoint::new(rect.center() + egui::vec2(-2.0, 2.0), color, 1.0),
            PotentialPoint::new(rect.center() + egui::vec2(4.0, 3.0), color, 1.0),
            PotentialPoint::new(rect.center(), color, 1.0),
            PotentialPoint::new(rect.center() + egui::vec2(1.0, -1.0), color, 1.0),
        ];
        let settings = PotentialSettings::new(20.0, 1.6, false);
        let first = render_potential_field(rect, &points, settings);
        let second = render_potential_field(rect, &points, settings);
        assert_eq!(first.pixels, second.pixels);
        let center_index = (first.size[1] / 2)
            .saturating_mul(first.size[0])
            .saturating_add(first.size[0] / 2);
        let center = first.pixels[center_index];
        let corner = first.pixels[0];
        let center_light = u16::from(center.r()) + u16::from(center.g()) + u16::from(center.b());
        let corner_light = u16::from(corner.r()) + u16::from(corner.g()) + u16::from(corner.b());
        assert!(center_light > corner_light.saturating_mul(8));
    }

    #[test]
    fn neighbourhood_density_outranks_an_isolated_sample() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0));
        let isolated = egui::pos2(80.0, 100.0);
        let cluster = egui::pos2(300.0, 100.0);
        let color = egui::Color32::from_rgb(42, 210, 255);
        let points = [
            PotentialPoint::new(isolated, color, 1.0),
            PotentialPoint::new(cluster + egui::vec2(-6.0, -4.0), color, 1.0),
            PotentialPoint::new(cluster + egui::vec2(5.0, -5.0), color, 1.0),
            PotentialPoint::new(cluster + egui::vec2(-4.0, 3.0), color, 1.0),
            PotentialPoint::new(cluster + egui::vec2(4.0, 4.0), color, 1.0),
            PotentialPoint::new(cluster + egui::vec2(0.0, -1.0), color, 1.0),
            PotentialPoint::new(cluster + egui::vec2(2.0, 1.0), color, 1.0),
        ];
        let image = render_potential_field(rect, &points, PotentialSettings::new(20.0, 1.6, false));
        let sample_light = |position: egui::Pos2| {
            let x = (((position.x - rect.left()) / rect.width())
                * image.size[0].saturating_sub(1) as f32)
                .round() as usize;
            let y = (((position.y - rect.top()) / rect.height())
                * image.size[1].saturating_sub(1) as f32)
                .round() as usize;
            let pixel = image.pixels[y.saturating_mul(image.size[0]).saturating_add(x)];
            u16::from(pixel.r()) + u16::from(pixel.g()) + u16::from(pixel.b())
        };
        let isolated_light = sample_light(isolated);
        let cluster_light = sample_light(cluster);
        assert!(cluster_light > isolated_light.saturating_mul(4));
    }

    #[test]
    fn source_alpha_does_not_change_density() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0));
        let settings = PotentialSettings::new(20.0, 1.6, false);
        let opaque = [PotentialPoint::new(
            rect.center(),
            egui::Color32::from_rgba_unmultiplied(42, 210, 255, 255),
            2.0,
        )];
        let contextual = [PotentialPoint::new(
            rect.center(),
            egui::Color32::from_rgba_unmultiplied(42, 210, 255, 24),
            2.0,
        )];
        let opaque_field = render_potential_field(rect, &opaque, settings);
        let contextual_field = render_potential_field(rect, &contextual, settings);
        assert_eq!(opaque_field.pixels, contextual_field.pixels);
    }

    #[test]
    fn potential_palette_has_monotonic_perceived_lightness() {
        let perceived_lightness = |tone: f32| {
            let color = potential_palette(tone);
            color[0].mul_add(0.2126, color[1].mul_add(0.7152, color[2] * 0.0722))
        };
        let mut previous = perceived_lightness(0.0);
        for step in 1..=20 {
            let current = perceived_lightness(step as f32 / 20.0);
            assert!(current + 0.025 >= previous);
            previous = current;
        }
    }
}
