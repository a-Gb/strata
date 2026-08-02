//! Deterministic coordinate mappings for stable projection samples.

use std::f32::consts::{PI, TAU};

use super::color::{mix_color, value_color};
use super::model::{
    ProjectionColorFeature, ProjectionDimensions, ProjectionHeightFeature, ProjectionKind,
    ProjectionParameters, ProjectionRegionPlacement, ProjectionSample, ProjectionSizeFeature,
};

impl ProjectionSample {
    pub(crate) fn position_at(self, morph: f32) -> [f32; 3] {
        self.position_with_relief(morph, 1.0)
    }

    pub(crate) fn position_with_relief(self, morph: f32, relief: f32) -> [f32; 3] {
        let terrain = mix_position(self.terrain_flat, self.positions[3], relief.clamp(0.0, 1.0));
        morph_position(
            self.positions[0],
            self.positions[1],
            self.positions[2],
            terrain,
            morph,
        )
    }

    pub(crate) fn color_at(self, morph: f32) -> [u8; 4] {
        if morph <= 2.0 {
            return self.colors[0];
        }
        mix_color(
            self.colors[0],
            self.colors[1],
            smooth_step((morph - 2.0).clamp(0.0, 1.0)),
        )
    }

    #[cfg(test)]
    pub(crate) fn color_with_mix(self, entropy_mix: f32) -> [u8; 4] {
        mix_color(
            self.colors[0],
            self.colors[1],
            smooth_step(entropy_mix.clamp(0.0, 1.0)),
        )
    }

    #[cfg(test)]
    pub(crate) fn position_for(
        self,
        kind: ProjectionKind,
        parameters: ProjectionParameters,
        region: Option<ProjectionRegionPlacement>,
        relief: f32,
    ) -> [f32; 3] {
        self.position_for_instance(
            kind,
            parameters,
            region,
            ProjectionHeightFeature::Entropy,
            relief,
            None,
        )
    }

    pub(crate) fn position_for_instance(
        self,
        kind: ProjectionKind,
        parameters: ProjectionParameters,
        region: Option<ProjectionRegionPlacement>,
        height_feature: ProjectionHeightFeature,
        relief: f32,
        bit_plane: Option<u8>,
    ) -> [f32; 3] {
        let mut position = match kind {
            ProjectionKind::AddressRaster => self.address_raster_position(parameters.row_width),
            ProjectionKind::Hilbert => self.hilbert_position(parameters),
            ProjectionKind::Transitions => self.transition_position(parameters.ngram_order),
            ProjectionKind::Bitplanes => {
                self.bitplane_position(parameters, bit_plane.unwrap_or(parameters.bit_plane))
            }
            ProjectionKind::Complexity => self.complexity_position(),
            ProjectionKind::Sections => self.section_position(region),
            ProjectionKind::AlignmentLattice => self.p1.map_or_else(
                || self.address_raster_position(parameters.alignment_stride),
                |p1| p1.alignment,
            ),
            ProjectionKind::RecurrencePlane => self
                .p1
                .map_or_else(|| self.complexity_position(), |p1| p1.recurrence),
            ProjectionKind::RepetitionSkyline => self
                .p1
                .map_or_else(|| self.complexity_position(), |p1| p1.repetition),
            ProjectionKind::SpectralWaterfall => self
                .p1
                .map_or_else(|| self.complexity_position(), |p1| p1.spectrum),
            ProjectionKind::HammingHypercube => self
                .p1
                .map_or_else(|| self.transition_position(3), |p1| p1.hypercube),
            ProjectionKind::HierarchicalBlockVolume => self
                .p1
                .map_or_else(|| self.section_position(region), |p1| p1.hierarchy),
            ProjectionKind::PolarAddressPath => self.polar_address_position(),
            ProjectionKind::HelicalAddressPath => self.helical_address_position(),
        };
        if relief > 0.0 && kind != ProjectionKind::Complexity {
            position[1] += self.height_offset(height_feature, relief);
        }
        position
    }

    #[cfg(test)]
    pub(crate) fn morphed_position(
        self,
        first: ProjectionKind,
        second: ProjectionKind,
        parameters: ProjectionParameters,
        region: Option<ProjectionRegionPlacement>,
        mix: f32,
        relief: f32,
    ) -> [f32; 3] {
        mix_position(
            self.position_for(first, parameters, region, relief),
            self.position_for(second, parameters, region, relief),
            smooth_step(mix.clamp(0.0, 1.0)),
        )
    }

    // These orthogonal values stay explicit so projection code never depends on hidden UI state.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn morphed_position_instance(
        self,
        first: ProjectionKind,
        second: ProjectionKind,
        parameters: ProjectionParameters,
        region: Option<ProjectionRegionPlacement>,
        mix: f32,
        height_feature: ProjectionHeightFeature,
        relief: f32,
        bit_plane: Option<u8>,
    ) -> [f32; 3] {
        mix_position(
            self.position_for_instance(
                first,
                parameters,
                region,
                height_feature,
                relief,
                bit_plane,
            ),
            self.position_for_instance(
                second,
                parameters,
                region,
                height_feature,
                relief,
                bit_plane,
            ),
            smooth_step(mix.clamp(0.0, 1.0)),
        )
    }

    pub(crate) const fn exact_analysis_range(self) -> [usize; 2] {
        self.analysis_range
    }

    pub(crate) const fn attach_p1(
        &mut self,
        feature: strata_analysis::projection_p1::P1FeatureRecord,
    ) {
        if feature.point_id == self.point_id {
            self.p1 = Some(feature);
        }
    }

    pub(crate) const fn primary_byte(self) -> u8 {
        self.bytes[0]
    }

    pub(crate) const fn p1_feature(
        self,
    ) -> Option<strata_analysis::projection_p1::P1FeatureRecord> {
        self.p1
    }

    pub(crate) fn color_for(self, feature: ProjectionColorFeature) -> [u8; 4] {
        match feature {
            ProjectionColorFeature::Address => self.colors[0],
            ProjectionColorFeature::Entropy => self.colors[1],
            ProjectionColorFeature::Value => value_color(self.bytes[0]),
        }
    }

    pub(crate) fn color_signal(self, feature: ProjectionColorFeature) -> f32 {
        match feature {
            ProjectionColorFeature::Address => ratio(
                self.relative_offset,
                self.source_length.saturating_sub(1).max(1),
            ),
            ProjectionColorFeature::Entropy => self.entropy,
            ProjectionColorFeature::Value => unit_byte(self.bytes[0]),
        }
        .clamp(0.0, 1.0)
    }

    pub(crate) fn size_scale(self, feature: ProjectionSizeFeature) -> f32 {
        match feature {
            ProjectionSizeFeature::Uniform => 1.0,
            ProjectionSizeFeature::Entropy => self.entropy.mul_add(0.7, 0.55),
            ProjectionSizeFeature::ChangeRate => self.change_rate.mul_add(0.7, 0.55),
        }
    }

    fn height_offset(self, feature: ProjectionHeightFeature, relief: f32) -> f32 {
        let signal = match feature {
            ProjectionHeightFeature::None => return 0.0,
            ProjectionHeightFeature::Entropy => self.entropy,
            ProjectionHeightFeature::ChangeRate => self.change_rate,
        };
        (signal - 0.5) * relief.clamp(0.0, 1.0) * 0.34
    }

    fn address_raster_position(self, row_width: usize) -> [f32; 3] {
        let width = row_width.max(2);
        let x = self.relative_offset % width;
        let row = self.relative_offset / width;
        let row_count = self.source_length.div_ceil(width).max(2);
        [
            ratio(x, width.saturating_sub(1)).mul_add(2.0, -1.0),
            1.0 - (ratio(row, row_count.saturating_sub(1)) * 2.0),
            0.0,
        ]
    }

    fn hilbert_position(self, parameters: ProjectionParameters) -> [f32; 3] {
        let order = parameters.curve_order.clamp(2, 8);
        let side = 1_u32 << order;
        let plane_cells = side.saturating_mul(side).max(1);
        let aggregate = parameters.aggregation_bytes.max(1);
        let logical_index = self.relative_offset / aggregate;
        let logical_index = u32::try_from(logical_index).unwrap_or(u32::MAX);
        let plane = logical_index / plane_cells;
        let index = logical_index % plane_cells;
        let [x, y] = hilbert_2d(index, side);
        let denominator = side.saturating_sub(1).max(1);
        let plane_count = u32::try_from(self.source_length.div_ceil(aggregate))
            .unwrap_or(u32::MAX)
            .div_ceil(plane_cells)
            .max(1);
        let z = if parameters.dimensions == ProjectionDimensions::Three && plane_count > 1 {
            ratio_u32(plane, plane_count.saturating_sub(1)).mul_add(2.0, -1.0)
        } else {
            0.0
        };
        [
            ratio_u32(x, denominator).mul_add(2.0, -1.0),
            1.0 - (ratio_u32(y, denominator) * 2.0),
            z,
        ]
    }

    fn transition_position(self, ngram_order: u8) -> [f32; 3] {
        [
            normalize_byte(self.bytes[0]),
            normalize_byte(self.bytes[1]),
            if ngram_order >= 3 {
                normalize_byte(self.bytes[2])
            } else {
                0.0
            },
        ]
    }

    fn bitplane_position(self, parameters: ProjectionParameters, plane: u8) -> [f32; 3] {
        let mut position = self.address_raster_position(parameters.row_width);
        let plane = plane.min(7);
        let bit_set = self.bytes[0] & (1 << plane) != 0;
        position[2] = (f32::from(plane) / 7.0).mul_add(1.6, -0.8);
        position[1] += if bit_set { 0.055 } else { -0.055 };
        position
    }

    fn complexity_position(self) -> [f32; 3] {
        [
            self.entropy.mul_add(2.0, -1.0),
            self.change_rate.mul_add(2.0, -1.0),
            self.unique_fraction.mul_add(2.0, -1.0),
        ]
    }

    fn section_position(self, region: Option<ProjectionRegionPlacement>) -> [f32; 3] {
        let fallback_count = 8_usize;
        let fallback_slot = ((self.relative_offset.saturating_mul(fallback_count))
            / self.source_length.max(1))
        .min(fallback_count.saturating_sub(1));
        let fallback_start = (fallback_slot.saturating_mul(self.source_length)) / fallback_count;
        let fallback_end = ((fallback_slot.saturating_add(1)).saturating_mul(self.source_length))
            .div_ceil(fallback_count)
            .max(fallback_start.saturating_add(1));
        let fallback_progress = ratio(
            self.relative_offset.saturating_sub(fallback_start),
            fallback_end.saturating_sub(fallback_start).max(1),
        );
        let placement = region.unwrap_or(ProjectionRegionPlacement {
            slot: fallback_slot,
            count: fallback_count,
            local_progress: fallback_progress,
        });
        let columns = 4_usize;
        let column = placement.slot % columns;
        let row = placement.slot / columns;
        let rows = placement.count.div_ceil(columns).max(1);
        [
            ratio(column, columns.saturating_sub(1)).mul_add(1.6, -0.8),
            placement
                .local_progress
                .clamp(0.0, 1.0)
                .mul_add(1.65, -0.825),
            if rows > 1 {
                ratio(row, rows.saturating_sub(1)).mul_add(1.4, -0.7)
            } else {
                0.0
            },
        ]
    }

    fn polar_address_position(self) -> [f32; 3] {
        let progress = ratio(
            self.relative_offset,
            self.source_length.saturating_sub(1).max(1),
        );
        let angle = progress * TAU * 5.0;
        let radius = unit_byte(self.bytes[0]).mul_add(0.68, 0.2);
        [
            radius * angle.cos(),
            self.entropy.mul_add(1.4, -0.7),
            radius * angle.sin(),
        ]
    }

    fn helical_address_position(self) -> [f32; 3] {
        let progress = ratio(
            self.relative_offset,
            self.source_length.saturating_sub(1).max(1),
        );
        sequence_position(self.bytes[0], self.bytes[2], progress)
    }
}

pub(super) fn trigram_position(first: u8, second: u8, third: u8) -> [f32; 3] {
    [
        normalize_byte(first),
        normalize_byte(second),
        normalize_byte(third),
    ]
}

#[allow(clippy::suboptimal_flops)]
pub(super) fn orbit_position(first: u8, second: u8, third: u8) -> [f32; 3] {
    let longitude = unit_byte(first) * TAU;
    let latitude = (unit_byte(second) - 0.5) * PI;
    let radius = 0.22 + (unit_byte(third) * 0.78);
    let latitude_radius = radius * latitude.cos();
    [
        latitude_radius * longitude.cos(),
        radius * latitude.sin(),
        latitude_radius * longitude.sin(),
    ]
}

#[allow(clippy::suboptimal_flops)]
pub(super) fn sequence_position(first: u8, third: u8, progress: f32) -> [f32; 3] {
    let angle = (progress * TAU * 10.0) + (normalize_byte(first) * 0.32);
    let radius = 0.18 + (unit_byte(third) * 0.72);
    [
        radius * angle.cos(),
        (progress * 2.0) - 1.0,
        radius * angle.sin(),
    ]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn entropy_terrain_position(
    first: u8,
    third: u8,
    progress: f32,
    entropy: f32,
) -> [f32; 3] {
    let morton = (progress.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u16;
    let terrain_x = f32::from(compact_even_bits(morton)) / 255.0;
    let terrain_z = f32::from(compact_even_bits(morton >> 1)) / 255.0;
    let byte_relief = (f32::from(first.abs_diff(third)) / 255.0) * 0.16;
    [
        terrain_x.mul_add(2.0, -1.0),
        entropy.mul_add(1.7, byte_relief - 0.86),
        terrain_z.mul_add(2.0, -1.0),
    ]
}

const fn compact_even_bits(mut value: u16) -> u8 {
    let mut compacted = 0_u8;
    let mut bit = 0_u8;
    while bit < 8 {
        if value & 1 != 0 {
            compacted |= 1 << bit;
        }
        value >>= 2;
        bit += 1;
    }
    compacted
}

pub(super) fn morph_position(
    triplet: [f32; 3],
    orbit: [f32; 3],
    sequence: [f32; 3],
    terrain: [f32; 3],
    morph: f32,
) -> [f32; 3] {
    let morph = morph.clamp(0.0, 3.0);
    if morph <= 0.0 {
        return triplet;
    }
    if morph < 1.0 {
        return mix_position(triplet, orbit, smooth_step(morph));
    }
    if morph <= 1.0 {
        return orbit;
    }
    if morph < 2.0 {
        return mix_position(orbit, sequence, smooth_step(morph - 1.0));
    }
    if morph <= 2.0 {
        return sequence;
    }
    if morph < 3.0 {
        return mix_position(sequence, terrain, smooth_step(morph - 2.0));
    }
    terrain
}

#[allow(clippy::suboptimal_flops)]
pub(super) fn smooth_step(value: f32) -> f32 {
    value * value * (3.0 - (2.0 * value))
}

pub(super) fn mix_position(first: [f32; 3], second: [f32; 3], amount: f32) -> [f32; 3] {
    [
        (second[0] - first[0]).mul_add(amount, first[0]),
        (second[1] - first[1]).mul_add(amount, first[1]),
        (second[2] - first[2]).mul_add(amount, first[2]),
    ]
}

fn normalize_byte(byte: u8) -> f32 {
    unit_byte(byte).mul_add(2.0, -1.0)
}

pub(super) fn unit_byte(byte: u8) -> f32 {
    f32::from(byte) / 255.0
}

const fn hilbert_2d(mut index: u32, side: u32) -> [u32; 2] {
    let mut x = 0_u32;
    let mut y = 0_u32;
    let mut scale = 1_u32;
    while scale < side {
        let rotate_x = (index / 2) & 1;
        let rotate_y = (index ^ rotate_x) & 1;
        if rotate_y == 0 {
            if rotate_x == 1 {
                x = scale.saturating_sub(1).saturating_sub(x);
                y = scale.saturating_sub(1).saturating_sub(y);
            }
            std::mem::swap(&mut x, &mut y);
        }
        x = x.saturating_add(scale.saturating_mul(rotate_x));
        y = y.saturating_add(scale.saturating_mul(rotate_y));
        index /= 4;
        scale = scale.saturating_mul(2);
    }
    [x, y]
}

#[allow(clippy::cast_precision_loss)]
fn ratio(numerator: usize, denominator: usize) -> f32 {
    numerator as f32 / denominator as f32
}

#[allow(clippy::cast_precision_loss)]
fn ratio_u32(numerator: u32, denominator: u32) -> f32 {
    numerator as f32 / denominator as f32
}
