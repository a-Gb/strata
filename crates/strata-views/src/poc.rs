//! Render-ready, dependency-free mappings for the first Strata demonstrations.
//!
//! These functions deliberately accept byte slices and count slices instead of analysis
//! service artifacts. This keeps the POC runnable while its worker and renderer contracts
//! are still being implemented. Every atlas layout retains an exact source-offset mapping.

use strata_analysis::poc::{ByteClass, classify_byte};
use strata_core::ByteRange;

/// A compact RGBA8 image ready to upload to a texture or draw in a software canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Image width in pixels.
    pub width: usize,
    /// Image height in pixels.
    pub height: usize,
    /// Row-major RGBA8 pixels, four bytes per pixel.
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    fn new(width: usize, height: usize) -> Option<Self> {
        let byte_len = width.checked_mul(height)?.checked_mul(4)?;
        Some(Self {
            width,
            height,
            pixels: vec![0; byte_len],
        })
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        let Some(row_start) = y.checked_mul(self.width) else {
            return;
        };
        let Some(pixel_index) = row_start.checked_add(x) else {
            return;
        };
        let Some(byte_index) = pixel_index.checked_mul(4) else {
            return;
        };
        let Some(byte_end) = byte_index.checked_add(4) else {
            return;
        };
        let Some(pixel) = self.pixels.get_mut(byte_index..byte_end) else {
            return;
        };
        pixel.copy_from_slice(&color);
    }
}

/// Exact row-major layout metadata for one-source-byte-per-pixel atlases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterLayout {
    /// Number of source bytes represented by the layout.
    pub byte_len: usize,
    /// Image width in pixels.
    pub width: usize,
    /// Image height in pixels, including a transparent final-row tail when needed.
    pub height: usize,
}

impl RasterLayout {
    /// Creates a row-major layout with `width` source bytes per row.
    #[must_use]
    pub fn new(byte_len: usize, width: usize) -> Option<Self> {
        let (resolved_width, height) = raster_dimensions(byte_len, width)?;
        Some(Self {
            byte_len,
            width: resolved_width,
            height,
        })
    }

    /// Resolves an in-bounds atlas pixel to its exact source byte offset.
    #[must_use]
    pub fn pixel_to_offset(self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = y.checked_mul(self.width)?.checked_add(x)?;
        (offset < self.byte_len).then_some(offset)
    }

    /// Resolves an in-bounds atlas pixel to the exact half-open source range it represents.
    #[must_use]
    pub fn pixel_to_range(self, x: usize, y: usize) -> Option<ByteRange> {
        let start = u64::try_from(self.pixel_to_offset(x, y)?).ok()?;
        let end = start.checked_add(1)?;
        ByteRange::new(start, end).ok()
    }

    /// Resolves a source byte offset to the unique atlas pixel that represents it.
    #[must_use]
    pub const fn offset_to_pixel(self, offset: usize) -> Option<(usize, usize)> {
        if offset >= self.byte_len {
            return None;
        }
        Some((offset % self.width, offset / self.width))
    }
}

/// Computes exact raster dimensions, retaining one transparent row for an empty source.
#[must_use]
pub fn raster_dimensions(byte_len: usize, width: usize) -> Option<(usize, usize)> {
    if width == 0 {
        return None;
    }
    let rows = byte_len.div_ceil(width);
    Some((width, rows.max(1)))
}

/// Resolves a pixel through a raw row-major raster configuration.
#[must_use]
pub fn raster_pixel_to_byte_offset(
    byte_len: usize,
    width: usize,
    x: usize,
    y: usize,
) -> Option<usize> {
    RasterLayout::new(byte_len, width)?.pixel_to_offset(x, y)
}

/// Resolves a pixel through a raw row-major raster configuration to an exact byte range.
#[must_use]
pub fn raster_pixel_to_byte_range(
    byte_len: usize,
    width: usize,
    x: usize,
    y: usize,
) -> Option<ByteRange> {
    RasterLayout::new(byte_len, width)?.pixel_to_range(x, y)
}

/// Resolves a raw source byte offset to its row-major raster pixel.
#[must_use]
pub fn raster_byte_offset_to_pixel(
    byte_len: usize,
    width: usize,
    offset: usize,
) -> Option<(usize, usize)> {
    RasterLayout::new(byte_len, width)?.offset_to_pixel(offset)
}

const DIGRAM_EMPTY: [u8; 4] = [10, 16, 28, 255];

const fn byte_class_color(class: ByteClass) -> [u8; 4] {
    match class {
        ByteClass::Zero => [25, 41, 68, 255],
        ByteClass::AllOnes => [235, 151, 66, 255],
        ByteClass::Whitespace => [130, 112, 186, 255],
        ByteClass::PrintableAscii => [74, 190, 168, 255],
        ByteClass::Control => [113, 128, 145, 255],
        ByteClass::HighBit => [202, 82, 104, 255],
    }
}

/// Renders a one-byte-per-pixel structural atlas using [`ByteClass`] colors.
///
/// The returned [`RasterLayout`] preserves the exact pick mapping for the image.
#[must_use]
pub fn render_byte_class_atlas(bytes: &[u8], width: usize) -> Option<(RgbaImage, RasterLayout)> {
    let classes = bytes.iter().copied().map(classify_byte).collect::<Vec<_>>();
    render_classified_byte_atlas(&classes, width)
}

/// Renders canonical precomputed byte classes without re-reading source bytes.
#[must_use]
pub fn render_classified_byte_atlas(
    classes: &[ByteClass],
    width: usize,
) -> Option<(RgbaImage, RasterLayout)> {
    let layout = RasterLayout::new(classes.len(), width)?;
    let mut image = RgbaImage::new(layout.width, layout.height)?;
    for (offset, class) in classes.iter().copied().enumerate() {
        let (x, y) = layout.offset_to_pixel(offset)?;
        image.set_pixel(x, y, byte_class_color(class));
    }
    Some((image, layout))
}

/// Counts all ordered byte pairs separated by `stride` bytes.
///
/// Index `from * 256 + to` contains the exact count for the pair `from -> to`.
#[must_use]
pub fn count_digrams(bytes: &[u8], stride: usize) -> Option<Vec<u64>> {
    if stride == 0 {
        return None;
    }
    let mut counts = vec![0_u64; 256 * 256];
    let Some(pair_len) = bytes.len().checked_sub(stride) else {
        return Some(counts);
    };
    for start in 0..pair_len {
        let end = start.checked_add(stride)?;
        let index = (usize::from(bytes[start]) * 256) + usize::from(bytes[end]);
        let count = counts.get_mut(index)?;
        *count = count.checked_add(1)?;
    }
    Some(counts)
}

/// Renders exactly 256 by 256 pixels of log-scaled digram counts.
///
/// Matrix coordinate `(from, to)` maps to pixel `(from, to)`. The scale uses the integer
/// `floor(log2(count + 1))`, which is stable across platforms and preserves zero as black.
#[must_use]
pub fn render_log_digram_matrix(counts: &[u64]) -> Option<RgbaImage> {
    if counts.len() != 256 * 256 {
        return None;
    }
    let maximum = counts.iter().copied().max()?;
    let max_log = maximum.saturating_add(1).ilog2();
    let mut image = RgbaImage::new(256, 256)?;
    for (index, count) in counts.iter().copied().enumerate() {
        let x = index / 256;
        let y = index % 256;
        if count == 0 || max_log == 0 {
            image.set_pixel(x, y, DIGRAM_EMPTY);
            continue;
        }
        let count_log = count.saturating_add(1).ilog2();
        let intensity = (count_log * 255) / max_log;
        let channel = u8::try_from(intensity).map_or(u8::MAX, std::convert::identity);
        image.set_pixel(x, y, [channel, channel, 255, 255]);
    }
    Some(image)
}

/// Validated controls for a stride/lane bit-plane atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitPlaneConfig {
    /// Pixels per output row.
    pub width: usize,
    /// Number of interleaved lanes in the source.
    pub stride: usize,
    /// Zero-based lane selected from each stride group.
    pub lane: usize,
    /// Zero-based bit selected from each source byte.
    pub bit: u8,
}

/// Exact source mapping for a configured bit-plane/stride atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitPlaneLayout {
    /// Validated bit-plane controls.
    pub config: BitPlaneConfig,
    /// Raster over selected lane values rather than raw source bytes.
    pub raster: RasterLayout,
}

impl BitPlaneLayout {
    /// Validates controls and builds the exact output-to-source mapping.
    #[must_use]
    pub fn new(source_len: usize, config: BitPlaneConfig) -> Option<Self> {
        if config.width == 0
            || config.stride == 0
            || config.lane >= config.stride
            || config.bit >= 8
        {
            return None;
        }
        let selected_len = source_len
            .checked_sub(config.lane)
            .map_or(0, |remaining| remaining.div_ceil(config.stride));
        let raster = RasterLayout::new(selected_len, config.width)?;
        Some(Self { config, raster })
    }

    /// Resolves a rendered bit-plane pixel to its exact source byte offset.
    #[must_use]
    pub fn pixel_to_offset(self, x: usize, y: usize) -> Option<usize> {
        let selected_index = self.raster.pixel_to_offset(x, y)?;
        self.config
            .lane
            .checked_add(selected_index.checked_mul(self.config.stride)?)
    }

    /// Resolves a rendered bit-plane pixel to the exact source-byte range it represents.
    #[must_use]
    pub fn pixel_to_range(self, x: usize, y: usize) -> Option<ByteRange> {
        let start = u64::try_from(self.pixel_to_offset(x, y)?).ok()?;
        let end = start.checked_add(1)?;
        ByteRange::new(start, end).ok()
    }

    /// Resolves a source offset in the selected lane to its bit-plane pixel.
    #[must_use]
    pub fn offset_to_pixel(self, source_offset: usize) -> Option<(usize, usize)> {
        let relative = source_offset.checked_sub(self.config.lane)?;
        if relative % self.config.stride != 0 {
            return None;
        }
        self.raster.offset_to_pixel(relative / self.config.stride)
    }
}

/// Renders the selected bit as a bright/dark stride-lane atlas.
///
/// `None` indicates invalid controls or a dimension overflow. Transparent pixels are the
/// unused tail of the final raster row.
#[must_use]
pub fn render_bit_plane_stride_atlas(
    bytes: &[u8],
    config: BitPlaneConfig,
) -> Option<(RgbaImage, BitPlaneLayout)> {
    let layout = BitPlaneLayout::new(bytes.len(), config)?;
    let mut image = RgbaImage::new(layout.raster.width, layout.raster.height)?;
    for y in 0..image.height {
        for x in 0..image.width {
            let Some(offset) = layout.pixel_to_offset(x, y) else {
                continue;
            };
            let Some(byte) = bytes.get(offset).copied() else {
                continue;
            };
            let is_set = (byte & (1_u8 << config.bit)) != 0;
            let color = if is_set {
                [94, 220, 255, 255]
            } else {
                [13, 25, 44, 255]
            };
            image.set_pixel(x, y, color);
        }
    }
    Some((image, layout))
}

/// Classification of one aligned byte position between two revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionDiffClass {
    /// Both revisions contain the same byte at this offset.
    Equal,
    /// Both revisions contain different bytes at this offset.
    Changed,
    /// Only the right/new revision contains a byte at this offset.
    Added,
    /// Only the left/old revision contains a byte at this offset.
    Removed,
}

/// Classifies one exact aligned source offset between optional old and new bytes.
#[must_use]
pub const fn classify_revision_byte(
    left: Option<u8>,
    right: Option<u8>,
) -> Option<RevisionDiffClass> {
    match (left, right) {
        (Some(first), Some(second)) if first == second => Some(RevisionDiffClass::Equal),
        (Some(_), Some(_)) => Some(RevisionDiffClass::Changed),
        (None, Some(_)) => Some(RevisionDiffClass::Added),
        (Some(_), None) => Some(RevisionDiffClass::Removed),
        (None, None) => None,
    }
}

const fn revision_diff_color(class: RevisionDiffClass) -> [u8; 4] {
    match class {
        RevisionDiffClass::Equal => [29, 53, 82, 255],
        RevisionDiffClass::Changed => [242, 179, 63, 255],
        RevisionDiffClass::Added => [66, 190, 125, 255],
        RevisionDiffClass::Removed => [221, 91, 104, 255],
    }
}

/// Renders an exact aligned revision diff atlas at one source offset per pixel.
///
/// The returned layout maps every non-transparent output pixel to the shared aligned offset.
#[must_use]
pub fn render_revision_diff_atlas(
    left: &[u8],
    right: &[u8],
    width: usize,
) -> Option<(RgbaImage, RasterLayout)> {
    let byte_len = left.len().max(right.len());
    let layout = RasterLayout::new(byte_len, width)?;
    let mut image = RgbaImage::new(layout.width, layout.height)?;
    for offset in 0..byte_len {
        let class = classify_revision_byte(left.get(offset).copied(), right.get(offset).copied())?;
        let (x, y) = layout.offset_to_pixel(offset)?;
        image.set_pixel(x, y, revision_diff_color(class));
    }
    Some((image, layout))
}

#[cfg(test)]
mod tests {
    use super::{
        BitPlaneConfig, BitPlaneLayout, ByteClass, RasterLayout, RevisionDiffClass, classify_byte,
        classify_revision_byte, count_digrams, raster_byte_offset_to_pixel,
        raster_pixel_to_byte_offset, raster_pixel_to_byte_range, render_bit_plane_stride_atlas,
        render_byte_class_atlas, render_log_digram_matrix, render_revision_diff_atlas,
    };

    #[test]
    fn raster_layout_preserves_exact_offset_and_range_mappings() -> Result<(), String> {
        let layout = RasterLayout::new(10, 4).ok_or("layout should be valid")?;
        assert_eq!(layout.height, 3);
        assert_eq!(layout.pixel_to_offset(1, 2), Some(9));
        assert_eq!(layout.pixel_to_offset(2, 2), None);
        assert_eq!(layout.offset_to_pixel(9), Some((1, 2)));
        assert_eq!(raster_pixel_to_byte_offset(10, 4, 1, 2), Some(9));
        assert_eq!(raster_byte_offset_to_pixel(10, 4, 9), Some((1, 2)));
        let range = raster_pixel_to_byte_range(10, 4, 1, 2).ok_or("pixel should map")?;
        assert_eq!((range.start, range.end), (9, 10));
        Ok(())
    }

    #[test]
    fn structural_atlas_dimensions_and_classes_are_stable() -> Result<(), String> {
        let bytes = [0x00, 0xff, b' ', b'A', 0x01, 0x80];
        let (image, layout) = render_byte_class_atlas(&bytes, 4).ok_or("atlas should render")?;
        assert_eq!((image.width, image.height), (4, 2));
        assert_eq!(layout.pixel_to_offset(1, 1), Some(5));
        assert_eq!(classify_byte(bytes[0]), ByteClass::Zero);
        assert_eq!(classify_byte(bytes[1]), ByteClass::AllOnes);
        assert_eq!(classify_byte(bytes[3]), ByteClass::PrintableAscii);
        Ok(())
    }

    #[test]
    fn digram_counting_and_matrix_shape_are_exact() -> Result<(), String> {
        let counts = count_digrams(&[0x10, 0x20, 0x10], 1).ok_or("stride is valid")?;
        assert_eq!(counts[(0x10 * 256) + 0x20], 1);
        assert_eq!(counts[(0x20 * 256) + 0x10], 1);
        let image = render_log_digram_matrix(&counts).ok_or("matrix should render")?;
        assert_eq!(
            (image.width, image.height, image.pixels.len()),
            (256, 256, 262_144)
        );
        Ok(())
    }

    #[test]
    fn bit_plane_stride_layout_maps_back_to_selected_source_lane() -> Result<(), String> {
        let config = BitPlaneConfig {
            width: 2,
            stride: 3,
            lane: 1,
            bit: 0,
        };
        let layout = BitPlaneLayout::new(8, config).ok_or("layout should be valid")?;
        assert_eq!(layout.raster.byte_len, 3);
        assert_eq!(layout.pixel_to_offset(0, 0), Some(1));
        assert_eq!(layout.pixel_to_offset(0, 1), Some(7));
        assert_eq!(layout.offset_to_pixel(4), Some((1, 0)));
        let (image, _) = render_bit_plane_stride_atlas(&[0, 1, 2, 3, 4, 5, 6, 7], config)
            .ok_or("atlas should render")?;
        assert_eq!((image.width, image.height), (2, 2));
        Ok(())
    }

    #[test]
    fn revision_diff_distinguishes_all_exact_classes() -> Result<(), String> {
        assert_eq!(
            classify_revision_byte(Some(1), Some(1)),
            Some(RevisionDiffClass::Equal)
        );
        assert_eq!(
            classify_revision_byte(Some(1), Some(2)),
            Some(RevisionDiffClass::Changed)
        );
        assert_eq!(
            classify_revision_byte(None, Some(2)),
            Some(RevisionDiffClass::Added)
        );
        assert_eq!(
            classify_revision_byte(Some(1), None),
            Some(RevisionDiffClass::Removed)
        );
        let (image, layout) =
            render_revision_diff_atlas(&[1, 2, 3], &[1, 4, 3, 5], 2).ok_or("diff should render")?;
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(layout.pixel_to_offset(1, 1), Some(3));
        assert_eq!(&image.pixels[4..8], &[242, 179, 63, 255]);
        assert_eq!(&image.pixels[12..16], &[66, 190, 125, 255]);
        Ok(())
    }
}
