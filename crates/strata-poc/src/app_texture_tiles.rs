//! Bounded GPU texture tiling for exact raster views.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]

use super::*;

const MAX_RASTER_TEXTURE_TILES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RasterTextureTilePlan {
    pub(super) row_start: usize,
    pub(super) row_count: usize,
}

pub(super) struct RasterTextureTile {
    pub(super) handle: egui::TextureHandle,
    pub(super) plan: RasterTextureTilePlan,
}

pub(super) fn plan_raster_texture_tiles(
    dimensions: [usize; 2],
    max_texture_side: usize,
) -> Result<Vec<RasterTextureTilePlan>, String> {
    let [width, height] = dimensions;
    if width == 0 || height == 0 {
        return Err("Raster texture dimensions must be non-zero".to_owned());
    }
    if max_texture_side == 0 {
        return Err("GPU reported a zero-sized texture limit".to_owned());
    }
    if width > max_texture_side {
        return Err(format!(
            "Raster width {width} exceeds the GPU texture-side limit {max_texture_side}"
        ));
    }
    let tile_count = height.div_ceil(max_texture_side);
    if tile_count > MAX_RASTER_TEXTURE_TILES {
        return Err(format!(
            "Raster requires {tile_count} GPU tiles; bounded maximum is {MAX_RASTER_TEXTURE_TILES}. Increase bytes per row or focus a smaller range"
        ));
    }
    let mut plans = Vec::with_capacity(tile_count);
    for tile_index in 0..tile_count {
        let row_start = tile_index
            .checked_mul(max_texture_side)
            .ok_or_else(|| "Raster tile row offset overflowed".to_owned())?;
        let row_count = height.saturating_sub(row_start).min(max_texture_side);
        plans.push(RasterTextureTilePlan {
            row_start,
            row_count,
        });
    }
    Ok(plans)
}

pub(super) fn upload_raster_texture_tiles(
    context: &egui::Context,
    name: &str,
    image: &RgbaImage,
    max_texture_side: usize,
) -> Result<Vec<RasterTextureTile>, String> {
    let plans = plan_raster_texture_tiles([image.width, image.height], max_texture_side)?;
    let bytes_per_row = image
        .width
        .checked_mul(4)
        .ok_or_else(|| "Raster byte width overflowed".to_owned())?;
    let mut tiles = Vec::with_capacity(plans.len());
    for (tile_index, plan) in plans.into_iter().enumerate() {
        let byte_start = plan
            .row_start
            .checked_mul(bytes_per_row)
            .ok_or_else(|| "Raster tile byte offset overflowed".to_owned())?;
        let byte_len = plan
            .row_count
            .checked_mul(bytes_per_row)
            .ok_or_else(|| "Raster tile byte length overflowed".to_owned())?;
        let byte_end = byte_start
            .checked_add(byte_len)
            .ok_or_else(|| "Raster tile byte range overflowed".to_owned())?;
        let pixels = image
            .pixels
            .get(byte_start..byte_end)
            .ok_or_else(|| "Raster tile byte range exceeded the rendered image".to_owned())?;
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([image.width, plan.row_count], pixels);
        let handle = context.load_texture(
            format!("{name}-tile-{tile_index}"),
            color_image,
            egui::TextureOptions::NEAREST,
        );
        tiles.push(RasterTextureTile { handle, plan });
    }
    Ok(tiles)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn paint_raster_texture_tiles(
    painter: &egui::Painter,
    target: egui::Rect,
    full_height: usize,
    tiles: &[RasterTextureTile],
) {
    if full_height == 0 || target.is_negative() {
        return;
    }
    let full_height = full_height as f32;
    let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    for tile in tiles {
        let row_end = tile.plan.row_start.saturating_add(tile.plan.row_count);
        let top = target
            .height()
            .mul_add(tile.plan.row_start as f32 / full_height, target.top());
        let bottom = target
            .height()
            .mul_add(row_end as f32 / full_height, target.top());
        let tile_rect = egui::Rect::from_min_max(
            egui::pos2(target.left(), top),
            egui::pos2(target.right(), bottom),
        );
        painter.image(tile.handle.id(), tile_rect, uv, egui::Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::{RasterTextureTilePlan, plan_raster_texture_tiles};

    #[test]
    fn oversized_height_is_partitioned_without_changing_width() -> Result<(), String> {
        let plans = plan_raster_texture_tiles([32, 13_298], 8_192)?;
        assert_eq!(
            plans,
            vec![
                RasterTextureTilePlan {
                    row_start: 0,
                    row_count: 8_192,
                },
                RasterTextureTilePlan {
                    row_start: 8_192,
                    row_count: 5_106,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn width_over_device_limit_fails_closed() {
        let result = plan_raster_texture_tiles([8_193, 2], 8_192);
        assert!(matches!(result, Err(error) if error.contains("exceeds")));
    }
}
