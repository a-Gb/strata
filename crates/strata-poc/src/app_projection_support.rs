//! Stateless projection mapping, picking, labels, and GPU request helpers.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

pub(super) fn projection_phase_for_mix(mix: f32) -> f32 {
    (mix.clamp(0.0, 1.0).mul_add(2.0, -1.0)).asin()
}

pub(super) const fn resonance_metric_label(metric: ResonanceMetric) -> &'static str {
    match metric {
        ResonanceMetric::ExactBytes => "exact bytes",
        ResonanceMetric::ByteShape => "byte shape",
        ResonanceMetric::Texture => "texture",
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn resonance_color(score: f32, opacity: f32) -> egui::Color32 {
    let score = score.clamp(0.0, 1.0);
    let (from, to, mix) = if score < 0.72 {
        (
            [18_u8, 54_u8, 105_u8],
            [58_u8, 203_u8, 218_u8],
            score / 0.72,
        )
    } else {
        (
            [58_u8, 203_u8, 218_u8],
            [255_u8, 190_u8, 58_u8],
            (score - 0.72) / 0.28,
        )
    };
    let channel = |index: usize| {
        f32::from(from[index])
            .mul_add(1.0 - mix, f32::from(to[index]) * mix)
            .round() as u8
    };
    let alpha = score
        .mul_add(205.0, 28.0)
        .mul_add(opacity.clamp(0.0, 1.0), 0.0)
        .clamp(0.0, 255.0)
        .round() as u8;
    egui::Color32::from_rgba_unmultiplied(channel(0), channel(1), channel(2), alpha)
}

pub(super) fn closest_resonance_point(
    points: &[ResonanceScreenPoint],
    position: egui::Pos2,
) -> Option<ResonanceScreenPoint> {
    points
        .iter()
        .copied()
        .filter_map(|point| {
            let delta = point.position - position;
            (delta.x.abs() <= 9.0 && delta.y.abs() <= 18.0).then_some((point, delta.length_sq()))
        })
        .min_by(|first, second| first.1.total_cmp(&second.1))
        .map(|(point, _)| point)
}

#[allow(clippy::too_many_lines)]
pub(super) fn project_points(
    samples: &[ProjectionSample],
    composition: ProjectionComposition,
    regions: &RegionModel,
    rect: egui::Rect,
    settings: ProjectionRenderSettings,
) -> Vec<ScreenProjection> {
    let mut projected = Vec::with_capacity(samples.len().saturating_mul(8));
    let split_gap = 8.0;
    let split_width = ((rect.width() - split_gap) * 0.5).max(1.0);
    let left = egui::Rect::from_min_size(rect.min, egui::vec2(split_width, rect.height()));
    let right = egui::Rect::from_min_size(
        egui::pos2(left.right() + split_gap, rect.top()),
        egui::vec2(split_width, rect.height()),
    );

    for sample in samples {
        let region = projection_region_placement(sample, regions);
        match composition.compare_mode {
            ProjectionCompareMode::Single => {
                for plane in projection_instance_planes(composition.projection_a) {
                    let bit_plane = projection_plane(composition.projection_a, plane);
                    push_screen_projection(
                        &mut projected,
                        sample,
                        sample.position_for_instance(
                            composition.projection_a,
                            composition.parameters,
                            region,
                            composition.channels.height,
                            settings.relief,
                            bit_plane,
                        ),
                        rect,
                        settings,
                        ProjectionSlot::A,
                        bit_plane,
                        composition.channels,
                        region.map(|placement| placement.slot),
                        1.0,
                    );
                }
            }
            ProjectionCompareMode::Split => {
                for plane in projection_instance_planes(composition.projection_a) {
                    let bit_plane = projection_plane(composition.projection_a, plane);
                    push_screen_projection(
                        &mut projected,
                        sample,
                        sample.position_for_instance(
                            composition.projection_a,
                            composition.parameters,
                            region,
                            composition.channels.height,
                            settings.relief,
                            bit_plane,
                        ),
                        left,
                        settings,
                        ProjectionSlot::A,
                        bit_plane,
                        composition.channels,
                        region.map(|placement| placement.slot),
                        1.0,
                    );
                }
                for plane in projection_instance_planes(composition.projection_b) {
                    let bit_plane = projection_plane(composition.projection_b, plane);
                    push_screen_projection(
                        &mut projected,
                        sample,
                        sample.position_for_instance(
                            composition.projection_b,
                            composition.parameters,
                            region,
                            composition.channels.height,
                            settings.relief,
                            bit_plane,
                        ),
                        right,
                        settings,
                        ProjectionSlot::B,
                        bit_plane,
                        composition.channels,
                        region.map(|placement| placement.slot),
                        1.0,
                    );
                }
            }
            ProjectionCompareMode::Overlay => {
                for (projection, slot, opacity) in [
                    (composition.projection_a, ProjectionSlot::A, 0.82),
                    (
                        composition.projection_b,
                        ProjectionSlot::B,
                        composition.mix.mul_add(0.55, 0.2),
                    ),
                ] {
                    for plane in projection_instance_planes(projection) {
                        let bit_plane = projection_plane(projection, plane);
                        push_screen_projection(
                            &mut projected,
                            sample,
                            sample.position_for_instance(
                                projection,
                                composition.parameters,
                                region,
                                composition.channels.height,
                                settings.relief,
                                bit_plane,
                            ),
                            rect,
                            settings,
                            slot,
                            bit_plane,
                            composition.channels,
                            region.map(|placement| placement.slot),
                            opacity,
                        );
                    }
                }
            }
            ProjectionCompareMode::Morph => {
                let has_bitplanes = composition.projection_a == ProjectionKind::Bitplanes
                    || composition.projection_b == ProjectionKind::Bitplanes;
                for plane in 0..=if has_bitplanes { 7 } else { 0 } {
                    let bit_plane = has_bitplanes.then_some(plane);
                    push_screen_projection(
                        &mut projected,
                        sample,
                        sample.morphed_position_instance(
                            composition.projection_a,
                            composition.projection_b,
                            composition.parameters,
                            region,
                            composition.mix,
                            composition.channels.height,
                            settings.relief,
                            bit_plane,
                        ),
                        rect,
                        settings,
                        ProjectionSlot::A,
                        bit_plane,
                        composition.channels,
                        region.map(|placement| placement.slot),
                        1.0,
                    );
                }
            }
        }
    }
    projected
}

pub(super) const fn is_p1_projection(projection: ProjectionKind) -> bool {
    matches!(
        projection,
        ProjectionKind::AlignmentLattice
            | ProjectionKind::RecurrencePlane
            | ProjectionKind::RepetitionSkyline
            | ProjectionKind::SpectralWaterfall
            | ProjectionKind::HammingHypercube
            | ProjectionKind::HierarchicalBlockVolume
    )
}

pub(super) fn projection_ranges_inside(
    samples: &[ProjectionSample],
    resident: ByteRange,
) -> Vec<ByteRange> {
    samples
        .iter()
        .filter_map(|sample| {
            let [start, end] = sample.exact_analysis_range();
            let range =
                ByteRange::new(u64::try_from(start).ok()?, u64::try_from(end).ok()?).ok()?;
            (range.start >= resident.start && range.end <= resident.end).then_some(range)
        })
        .collect()
}

pub(super) fn merge_alignment_candidates(
    candidates: &[AlignmentCandidate],
    limit: usize,
) -> Vec<AlignmentCandidate> {
    let mut by_stride = BTreeMap::<usize, (f32, usize)>::new();
    for candidate in candidates {
        let entry = by_stride.entry(candidate.stride).or_insert((0.0, 0));
        entry.0 += candidate.score;
        entry.1 = entry.1.saturating_add(1);
    }
    let mut merged = by_stride
        .into_iter()
        .filter_map(|(stride, (score, count))| {
            let count = u16::try_from(count).ok()?;
            (count > 0).then_some(AlignmentCandidate {
                stride,
                score: score / f32::from(count),
            })
        })
        .collect::<Vec<_>>();
    merged.sort_by(|first, second| {
        second
            .score
            .total_cmp(&first.score)
            .then_with(|| first.stride.cmp(&second.stride))
    });
    merged.truncate(limit);
    merged
}

pub(super) fn composition_uses_p1(composition: ProjectionComposition) -> bool {
    is_p1_projection(composition.projection_a)
        || (composition.compare_mode != ProjectionCompareMode::Single
            && is_p1_projection(composition.projection_b))
}

pub(super) fn composition_uses_gpu_coordinates(composition: ProjectionComposition) -> bool {
    matches!(
        composition.projection_a,
        ProjectionKind::AlignmentLattice | ProjectionKind::HammingHypercube
    ) || (composition.compare_mode != ProjectionCompareMode::Single
        && matches!(
            composition.projection_b,
            ProjectionKind::AlignmentLattice | ProjectionKind::HammingHypercube
        ))
}

pub(super) fn p1_feature_request(composition: ProjectionComposition) -> P1FeatureRequest {
    let recurrence = matches!(
        composition.projection_a,
        ProjectionKind::RecurrencePlane | ProjectionKind::RepetitionSkyline
    ) || (composition.compare_mode != ProjectionCompareMode::Single
        && matches!(
            composition.projection_b,
            ProjectionKind::RecurrencePlane | ProjectionKind::RepetitionSkyline
        ));
    let spectrum = composition.projection_a == ProjectionKind::SpectralWaterfall
        || (composition.compare_mode != ProjectionCompareMode::Single
            && composition.projection_b == ProjectionKind::SpectralWaterfall);
    let hierarchy = composition.projection_a == ProjectionKind::HierarchicalBlockVolume
        || (composition.compare_mode != ProjectionCompareMode::Single
            && composition.projection_b == ProjectionKind::HierarchicalBlockVolume);
    P1FeatureRequest {
        recurrence,
        spectrum,
        hierarchy,
    }
}

pub(super) const fn p1_analysis_config(parameters: ProjectionParameters) -> P1AnalysisConfig {
    P1AnalysisConfig {
        alignment_stride: parameters.alignment_stride,
        alignment_max_stride: parameters.alignment_max_stride,
        recurrence_window: parameters.recurrence_window,
        recurrence_search_bytes: parameters.recurrence_search_bytes,
        recurrence_candidate_budget: parameters.recurrence_candidate_budget,
        recurrence_threshold_percent: parameters.recurrence_threshold_percent,
        spectrum_window: parameters.spectrum_window,
        spectrum_bins: parameters.spectrum_bins,
        hierarchy_max_depth: parameters.hierarchy_max_depth,
        hierarchy_min_block: parameters.hierarchy_min_block,
        hierarchy_threshold_percent: parameters.hierarchy_threshold_percent,
    }
}

pub(super) fn p1_point_budget(composition: ProjectionComposition, requested: usize) -> usize {
    let request = p1_feature_request(composition);
    if request.spectrum {
        requested.min(2_048)
    } else if request.recurrence || request.hierarchy {
        requested.min(4_096)
    } else {
        requested
    }
}

pub(super) const fn projection_instance_multiplier(composition: ProjectionComposition) -> usize {
    let first = if matches!(composition.projection_a, ProjectionKind::Bitplanes) {
        8
    } else {
        1
    };
    let second = if matches!(composition.projection_b, ProjectionKind::Bitplanes) {
        8
    } else {
        1
    };
    match composition.compare_mode {
        ProjectionCompareMode::Single => first,
        ProjectionCompareMode::Split | ProjectionCompareMode::Overlay => first + second,
        ProjectionCompareMode::Morph => {
            if first > second {
                first
            } else {
                second
            }
        }
    }
}

// Screen instances deliberately carry all visual channels and exact source identity together.
#[allow(clippy::too_many_arguments)]
pub(super) fn push_screen_projection(
    projected: &mut Vec<ScreenProjection>,
    sample: &ProjectionSample,
    point: [f32; 3],
    rect: egui::Rect,
    settings: ProjectionRenderSettings,
    slot: ProjectionSlot,
    bit_plane: Option<u8>,
    channels: ProjectionChannels,
    region_slot: Option<usize>,
    opacity: f32,
) {
    let Some((position, depth, depth_scale)) = project_position(
        point,
        rect,
        settings.yaw,
        settings.pitch,
        settings.zoom,
        settings.perspective,
    ) else {
        return;
    };
    let depth_radius = (depth + 1.0).mul_add(0.12, 0.9);
    let radius =
        (settings.point_size * sample.size_scale(channels.size) * depth_scale * depth_radius)
            .clamp(0.35, 8.0);
    let color = projection_color(sample.color_for(channels.color), settings.brightness, depth);
    projected.push(ScreenProjection {
        position,
        depth,
        radius,
        color: projection_opacity(color, opacity),
        point_id: sample.point_id,
        source_offsets: sample.source_offsets,
        analysis_range: sample.exact_analysis_range(),
        slot,
        bit_plane,
        region_slot,
        p1: sample.p1_feature(),
    });
}

pub(super) fn projection_instance_planes(
    projection: ProjectionKind,
) -> std::ops::RangeInclusive<u8> {
    0..=if projection == ProjectionKind::Bitplanes {
        7
    } else {
        0
    }
}

pub(super) fn projection_plane(projection: ProjectionKind, plane: u8) -> Option<u8> {
    (projection == ProjectionKind::Bitplanes).then_some(plane)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn projection_region_placement(
    sample: &ProjectionSample,
    regions: &RegionModel,
) -> Option<ProjectionRegionPlacement> {
    let offset = u64::try_from(sample.source_offsets[0]).ok()?;
    let count = regions.regions().len();
    regions
        .regions()
        .iter()
        .enumerate()
        .find_map(|(slot, region)| {
            region
                .provenance
                .ranges
                .ranges
                .iter()
                .copied()
                .find(|range| range.contains(offset))
                .map(|range| ProjectionRegionPlacement {
                    slot,
                    count,
                    local_progress: if range.len() <= 1 {
                        0.5
                    } else {
                        (offset.saturating_sub(range.start) as f32) / (range.len() - 1) as f32
                    },
                })
        })
}

#[allow(clippy::suboptimal_flops)]
pub(super) fn project_position(
    point: [f32; 3],
    rect: egui::Rect,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    perspective: f32,
) -> Option<(egui::Pos2, f32, f32)> {
    let yaw_cosine = yaw.cos();
    let yaw_sine = yaw.sin();
    let pitch_cosine = pitch.cos();
    let pitch_sine = pitch.sin();
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
    let screen_scale = rect.width().min(rect.height()) * 0.39 * zoom * depth_scale;
    let position = egui::pos2(
        rect.center().x + (yaw_x * screen_scale),
        rect.center().y - (rotated_y * screen_scale),
    );
    position
        .is_finite()
        .then_some((position, depth, depth_scale))
}

pub(super) fn paint_projection_labels(
    painter: &egui::Painter,
    rect: egui::Rect,
    state: ProjectionLabelState,
) {
    let compact = rect.width() < 500.0;
    painter.text(
        rect.left_top() + egui::vec2(14.0, 12.0),
        egui::Align2::LEFT_TOP,
        if compact {
            "DRAG / ZOOM / PICK / RESET"
        } else {
            "DRAG ROTATE / WHEEL ZOOM / CLICK PICK / DOUBLE-CLICK RESET"
        },
        egui::FontId::monospace(10.5),
        egui::Color32::from_gray(126),
    );
    painter.text(
        rect.right_top() + egui::vec2(-14.0, 12.0),
        egui::Align2::RIGHT_TOP,
        format!(
            "{} samples / {}",
            state.point_count,
            state.composition.geometry.label()
        ),
        egui::FontId::monospace(10.5),
        egui::Color32::from_gray(132),
    );
    let (footer_left, footer_right) = projection_footer_labels(state, compact);
    painter.text(
        rect.left_bottom() + egui::vec2(14.0, -12.0),
        egui::Align2::LEFT_BOTTOM,
        footer_left,
        egui::FontId::monospace(10.5),
        egui::Color32::from_gray(124),
    );
    painter.text(
        rect.right_bottom() + egui::vec2(-14.0, -12.0),
        egui::Align2::RIGHT_BOTTOM,
        footer_right,
        egui::FontId::monospace(10.5),
        egui::Color32::from_gray(124),
    );
    if state.composition.compare_mode == ProjectionCompareMode::Split {
        painter.line_segment(
            [
                egui::pos2(rect.center().x, rect.top() + 34.0),
                egui::pos2(rect.center().x, rect.bottom() - 34.0),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(28)),
        );
        painter.text(
            egui::pos2(rect.left() + 14.0, rect.top() + 31.0),
            egui::Align2::LEFT_TOP,
            format!("A  {}", state.composition.projection_a.short_label()),
            egui::FontId::monospace(10.0),
            UI_CYAN,
        );
        painter.text(
            egui::pos2(rect.center().x + 14.0, rect.top() + 31.0),
            egui::Align2::LEFT_TOP,
            format!("B  {}", state.composition.projection_b.short_label()),
            egui::FontId::monospace(10.0),
            UI_AMBER,
        );
    }
}

pub(super) fn projection_voxel_rect(position: egui::Pos2, radius: f32) -> egui::Rect {
    let side = (radius.max(0.5) * 2.0).round().max(1.0);
    egui::Rect::from_center_size(position, egui::vec2(side, side))
}

pub(super) fn projection_footer_labels(
    state: ProjectionLabelState,
    compact: bool,
) -> (String, String) {
    let composition = state.composition;
    let field = if composition.geometry.uses_field() {
        format!(
            "{} R{:.0} G{:.1}{}",
            composition.geometry.label(),
            state.field_radius,
            state.field_exposure,
            if state.field_contours { " +C" } else { "" }
        )
    } else {
        composition.geometry.label().to_owned()
    };
    let colour = composition.channels.color.label();
    let left = if compact {
        format!(
            "{} / {}",
            composition.projection_a.short_label().to_uppercase(),
            composition.compare_mode.label()
        )
    } else {
        match composition.compare_mode {
            ProjectionCompareMode::Single => format!(
                "A {} / {}",
                composition.projection_a.short_label().to_uppercase(),
                field
            ),
            ProjectionCompareMode::Split | ProjectionCompareMode::Overlay => format!(
                "A {} / B {} / {} / {}",
                composition.projection_a.short_label().to_uppercase(),
                composition.projection_b.short_label().to_uppercase(),
                composition.compare_mode.label(),
                field
            ),
            ProjectionCompareMode::Morph => format!(
                "A {} > B {} / MORPH {:.0}% / {}",
                composition.projection_a.short_label().to_uppercase(),
                composition.projection_b.short_label().to_uppercase(),
                composition.mix * 100.0,
                field
            ),
        }
    };
    let right = if compact {
        format!(
            "{colour} / H{:.0} / C{:.0}",
            state.relief * 100.0,
            state.context_light * 100.0
        )
    } else {
        format!(
            "{} / C:{colour} H:{} S:{} O:{}",
            composition.domain.label().to_uppercase(),
            composition.channels.height.label(),
            composition.channels.size.label(),
            composition.channels.opacity.label()
        )
    };
    (left, right)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn projection_color(color: [u8; 4], brightness: f32, depth: f32) -> egui::Color32 {
    let depth_light = (depth + 1.0).mul_add(0.14, 0.82).clamp(0.55, 1.2);
    let scale = brightness * depth_light;
    let channel = |value: u8| (f32::from(value) * scale).clamp(0.0, 255.0).round() as u8;
    let alpha = (f32::from(color[3]) * brightness.clamp(0.35, 1.6))
        .clamp(35.0, 220.0)
        .round() as u8;
    egui::Color32::from_rgba_unmultiplied(
        channel(color[0]),
        channel(color[1]),
        channel(color[2]),
        alpha,
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn projection_context_color(color: egui::Color32, context_light: f32) -> egui::Color32 {
    let light = context_light.clamp(0.05, 1.0);
    let channel = |value: u8| (f32::from(value) * light).round() as u8;
    egui::Color32::from_rgba_premultiplied(
        channel(color.r()),
        channel(color.g()),
        channel(color.b()),
        channel(color.a()),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn projection_opacity(color: egui::Color32, opacity: f32) -> egui::Color32 {
    let alpha = (f32::from(color.a()) * opacity.clamp(0.0, 1.0)).round() as u8;
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub(super) const fn projection_region_color(slot: usize) -> egui::Color32 {
    const PALETTE: [egui::Color32; 6] = [
        egui::Color32::from_rgb(58, 203, 218),
        egui::Color32::from_rgb(238, 184, 72),
        egui::Color32::from_rgb(141, 116, 232),
        egui::Color32::from_rgb(95, 190, 124),
        egui::Color32::from_rgb(224, 105, 122),
        egui::Color32::from_rgb(132, 164, 194),
    ];
    PALETTE[slot % PALETTE.len()]
}

pub(super) const fn projection_point_is_selected(
    point: &ScreenProjection,
    selection: &Range<usize>,
) -> bool {
    selection.start < selection.end
        && point.analysis_range[0] < selection.end
        && selection.start < point.analysis_range[1]
}

pub(super) fn closest_screen_point(
    points: &[ScreenProjection],
    position: egui::Pos2,
) -> Option<ScreenProjection> {
    points
        .iter()
        .copied()
        .filter_map(|point| {
            let distance_squared = (point.position - position).length_sq();
            let hit_radius = point.radius + 8.0;
            (distance_squared <= hit_radius * hit_radius).then_some((point, distance_squared))
        })
        .max_by(|first, second| {
            first
                .0
                .depth
                .total_cmp(&second.0.depth)
                .then_with(|| second.1.total_cmp(&first.1))
        })
        .map(|(point, _)| point)
}
