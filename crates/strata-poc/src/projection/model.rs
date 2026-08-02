//! Projection composition contracts and stable source-addressed sample records.

use serde::{Deserialize, Serialize};
use strata_analysis::projection_p1::P1FeatureRecord;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionDomain {
    #[default]
    Byte,
    Word,
    Window,
    Region,
}

impl ProjectionDomain {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Byte => "Byte",
            Self::Word => "Word",
            Self::Window => "Window",
            Self::Region => "Region",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionKind {
    AddressRaster,
    #[default]
    Hilbert,
    Transitions,
    Bitplanes,
    Complexity,
    Sections,
    AlignmentLattice,
    RecurrencePlane,
    RepetitionSkyline,
    SpectralWaterfall,
    HammingHypercube,
    HierarchicalBlockVolume,
    PolarAddressPath,
    HelicalAddressPath,
}

impl ProjectionKind {
    pub(crate) const BASIC: [Self; 6] = [
        Self::AddressRaster,
        Self::Hilbert,
        Self::Transitions,
        Self::Bitplanes,
        Self::Complexity,
        Self::Sections,
    ];

    pub(crate) const P1: [Self; 6] = [
        Self::AlignmentLattice,
        Self::RecurrencePlane,
        Self::RepetitionSkyline,
        Self::SpectralWaterfall,
        Self::HammingHypercube,
        Self::HierarchicalBlockVolume,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::AddressRaster => "Address raster",
            Self::Hilbert => "Hilbert plane / cube",
            Self::Transitions => "Transition field",
            Self::Bitplanes => "Bit-plane stack",
            Self::Complexity => "Complexity phase",
            Self::Sections => "Section prism",
            Self::AlignmentLattice => "Alignment lattice",
            Self::RecurrencePlane => "Recurrence plane",
            Self::RepetitionSkyline => "Repetition skyline",
            Self::SpectralWaterfall => "Spectral waterfall",
            Self::HammingHypercube => "Hamming hypercube",
            Self::HierarchicalBlockVolume => "Hierarchical block volume",
            Self::PolarAddressPath => "Polar address path",
            Self::HelicalAddressPath => "Helical address path",
        }
    }

    pub(crate) const fn short_label(self) -> &'static str {
        match self {
            Self::AddressRaster => "Raster",
            Self::Hilbert => "Hilbert",
            Self::Transitions => "Transitions",
            Self::Bitplanes => "Bitplanes",
            Self::Complexity => "Complexity",
            Self::Sections => "Sections",
            Self::AlignmentLattice => "Alignment",
            Self::RecurrencePlane => "Recurrence",
            Self::RepetitionSkyline => "Repetition",
            Self::SpectralWaterfall => "Spectrum",
            Self::HammingHypercube => "Hypercube",
            Self::HierarchicalBlockVolume => "Hierarchy",
            Self::PolarAddressPath => "Polar path",
            Self::HelicalAddressPath => "Helical path",
        }
    }

    pub(crate) const fn family_label(self) -> &'static str {
        match self {
            Self::AddressRaster
            | Self::Hilbert
            | Self::PolarAddressPath
            | Self::HelicalAddressPath => "ADDRESS",
            Self::Transitions
            | Self::Bitplanes
            | Self::RecurrencePlane
            | Self::RepetitionSkyline
            | Self::HammingHypercube => "RELATION",
            Self::AlignmentLattice => "ADDRESS / ALIGNMENT",
            Self::Complexity | Self::SpectralWaterfall | Self::HierarchicalBlockVolume => {
                "STATISTICS"
            }
            Self::Sections => "PARSED / REGIONS",
        }
    }

    pub(crate) const fn evidence_label(self) -> &'static str {
        match self {
            Self::Sections => "HEURISTIC OR PARSED",
            Self::Complexity | Self::HierarchicalBlockVolume => "HEURISTIC FEATURES",
            Self::RecurrencePlane | Self::RepetitionSkyline => "BOUNDED EXACT MATCHES",
            Self::SpectralWaterfall => "BOUNDED DETERMINISTIC DFT",
            _ => "RAW / DETERMINISTIC",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionGeometry {
    Points,
    Path,
    #[default]
    Voxels,
    Surface,
}

impl ProjectionGeometry {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Points => "POINTS",
            Self::Path => "PATH",
            Self::Voxels => "VOXELS",
            Self::Surface => "SURFACE",
        }
    }

    pub(crate) const fn uses_field(self) -> bool {
        matches!(self, Self::Surface)
    }

    pub(crate) const fn field_alpha(self) -> u8 {
        if self.uses_field() { 156 } else { 0 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionCompareMode {
    Single,
    #[default]
    Split,
    Overlay,
    Morph,
}

impl ProjectionCompareMode {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Single => "SINGLE",
            Self::Split => "SPLIT",
            Self::Overlay => "OVERLAY",
            Self::Morph => "MORPH",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionDimensions {
    Two,
    #[default]
    Three,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionColorFeature {
    #[default]
    Address,
    Entropy,
    Value,
}

impl ProjectionColorFeature {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Address => "ADDR",
            Self::Entropy => "ENTROPY",
            Self::Value => "VALUE",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionHeightFeature {
    None,
    #[default]
    Entropy,
    ChangeRate,
}

impl ProjectionHeightFeature {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Entropy => "ENTROPY",
            Self::ChangeRate => "CHANGE",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionSizeFeature {
    #[default]
    Uniform,
    Entropy,
    ChangeRate,
}

impl ProjectionSizeFeature {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Uniform => "UNIFORM",
            Self::Entropy => "ENTROPY",
            Self::ChangeRate => "CHANGE",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionOpacityFeature {
    Uniform,
    #[default]
    SelectionContext,
}

impl ProjectionOpacityFeature {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Uniform => "UNIFORM",
            Self::SelectionContext => "SELECTION",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectionChannels {
    pub(crate) color: ProjectionColorFeature,
    pub(crate) height: ProjectionHeightFeature,
    pub(crate) size: ProjectionSizeFeature,
    pub(crate) opacity: ProjectionOpacityFeature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ProjectionOverlays {
    pub(crate) selection: bool,
    pub(crate) regions: bool,
    pub(crate) signatures: bool,
}

impl Default for ProjectionOverlays {
    fn default() -> Self {
        Self {
            selection: true,
            regions: false,
            signatures: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ProjectionParameters {
    pub(crate) dimensions: ProjectionDimensions,
    pub(crate) row_width: usize,
    pub(crate) curve_order: u8,
    pub(crate) aggregation_bytes: usize,
    pub(crate) lag: usize,
    pub(crate) ngram_order: u8,
    pub(crate) window_bytes: usize,
    pub(crate) hop_bytes: usize,
    pub(crate) bit_plane: u8,
    pub(crate) word_bits: u16,
    pub(crate) little_endian: bool,
    pub(crate) alignment_stride: usize,
    pub(crate) alignment_max_stride: usize,
    pub(crate) recurrence_window: usize,
    pub(crate) recurrence_search_bytes: usize,
    pub(crate) recurrence_candidate_budget: usize,
    pub(crate) recurrence_threshold_percent: u8,
    pub(crate) spectrum_window: usize,
    pub(crate) spectrum_bins: usize,
    pub(crate) hierarchy_max_depth: u8,
    pub(crate) hierarchy_min_block: usize,
    pub(crate) hierarchy_threshold_percent: u8,
}

impl Default for ProjectionParameters {
    fn default() -> Self {
        Self {
            dimensions: ProjectionDimensions::Three,
            row_width: 64,
            curve_order: 5,
            aggregation_bytes: 2,
            lag: 1,
            ngram_order: 3,
            window_bytes: 32,
            hop_bytes: 2,
            bit_plane: 0,
            word_bits: 32,
            little_endian: true,
            alignment_stride: 16,
            alignment_max_stride: 128,
            recurrence_window: 16,
            recurrence_search_bytes: 4096,
            recurrence_candidate_budget: 64,
            recurrence_threshold_percent: 75,
            spectrum_window: 64,
            spectrum_bins: 32,
            hierarchy_max_depth: 6,
            hierarchy_min_block: 64,
            hierarchy_threshold_percent: 18,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectionComposition {
    pub(crate) domain: ProjectionDomain,
    pub(crate) projection_a: ProjectionKind,
    pub(crate) projection_b: ProjectionKind,
    pub(crate) geometry: ProjectionGeometry,
    pub(crate) compare_mode: ProjectionCompareMode,
    pub(crate) mix: f32,
    pub(crate) parameters: ProjectionParameters,
    #[serde(default)]
    pub(crate) channels: ProjectionChannels,
    #[serde(default)]
    pub(crate) overlays: ProjectionOverlays,
}

impl Default for ProjectionComposition {
    fn default() -> Self {
        Self {
            domain: ProjectionDomain::Window,
            projection_a: ProjectionKind::Hilbert,
            projection_b: ProjectionKind::Complexity,
            geometry: ProjectionGeometry::Voxels,
            compare_mode: ProjectionCompareMode::Split,
            mix: 0.35,
            parameters: ProjectionParameters::default(),
            channels: ProjectionChannels::default(),
            overlays: ProjectionOverlays::default(),
        }
    }
}

impl ProjectionComposition {
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        let parameters = self.parameters;
        if !self.mix.is_finite()
            || !(0.0..=1.0).contains(&self.mix)
            || !(4..=4096).contains(&parameters.row_width)
            || !(2..=8).contains(&parameters.curve_order)
            || !(1..=1_048_576).contains(&parameters.aggregation_bytes)
            || !(1..=1024).contains(&parameters.lag)
            || !(2..=3).contains(&parameters.ngram_order)
            || !(4..=1_048_576).contains(&parameters.window_bytes)
            || !(1..=1_048_576).contains(&parameters.hop_bytes)
            || parameters.bit_plane > 7
            || !matches!(parameters.word_bits, 8 | 16 | 32 | 64)
            || !(1..=4096).contains(&parameters.alignment_stride)
            || !(2..=4096).contains(&parameters.alignment_max_stride)
            || !(4..=4096).contains(&parameters.recurrence_window)
            || !(4..=16 * 1024 * 1024).contains(&parameters.recurrence_search_bytes)
            || !(1..=4096).contains(&parameters.recurrence_candidate_budget)
            || parameters.recurrence_threshold_percent > 100
            || !(8..=4096).contains(&parameters.spectrum_window)
            || !(1..=256).contains(&parameters.spectrum_bins)
            || parameters.hierarchy_max_depth > 16
            || !(8..=16 * 1024 * 1024).contains(&parameters.hierarchy_min_block)
            || parameters.hierarchy_threshold_percent > 100
        {
            return Err("projection composition parameters are outside their bounded domains");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ProjectionSamplingConfig {
    pub(crate) domain: ProjectionDomain,
    pub(crate) lag: usize,
    pub(crate) window_bytes: usize,
    pub(crate) hop_bytes: usize,
    pub(crate) aggregation_bytes: usize,
    pub(crate) word_bits: u16,
}

impl ProjectionSamplingConfig {
    pub(super) const fn legacy(stride: usize) -> Self {
        Self {
            domain: ProjectionDomain::Byte,
            lag: stride,
            window_bytes: 64,
            hop_bytes: 1,
            aggregation_bytes: 64,
            word_bits: 8,
        }
    }
}

impl From<ProjectionComposition> for ProjectionSamplingConfig {
    fn from(composition: ProjectionComposition) -> Self {
        let parameters = composition.parameters;
        Self {
            domain: composition.domain,
            lag: parameters.lag,
            window_bytes: parameters.window_bytes,
            hop_bytes: parameters.hop_bytes,
            aggregation_bytes: parameters.aggregation_bytes,
            word_bits: parameters.word_bits,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ProjectionRegionPlacement {
    pub(crate) slot: usize,
    pub(crate) count: usize,
    pub(crate) local_progress: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ProjectionSample {
    pub(super) positions: [[f32; 3]; 4],
    pub(super) terrain_flat: [f32; 3],
    pub(super) colors: [[u8; 4]; 2],
    pub(super) bytes: [u8; 3],
    pub(super) relative_offset: usize,
    pub(super) source_length: usize,
    pub(super) entropy: f32,
    pub(super) change_rate: f32,
    pub(super) unique_fraction: f32,
    pub(super) analysis_range: [usize; 2],
    pub(crate) point_id: u64,
    pub(crate) source_offsets: [usize; 3],
    pub(super) p1: Option<P1FeatureRecord>,
}
