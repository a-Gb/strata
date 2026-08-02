//! Stateless atlas sizing, preview, and entropy-strip helpers.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

#[allow(clippy::cast_precision_loss)]
pub(super) fn fitted_size(dimensions: [usize; 2], available: egui::Vec2) -> egui::Vec2 {
    let width = dimensions[0].max(1) as f32;
    let height = dimensions[1].max(1) as f32;
    let maximum = egui::vec2(available.x.max(1.0), available.y.max(1.0));
    let scale = (maximum.x / width).min(maximum.y / height).max(0.01);
    egui::vec2(width * scale, height * scale)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn image_pixel(
    position: egui::Pos2,
    rect: egui::Rect,
    dimensions: [usize; 2],
) -> Option<(usize, usize)> {
    if !rect.contains(position) || dimensions[0] == 0 || dimensions[1] == 0 {
        return None;
    }
    let normalized_x = ((position.x - rect.left()) / rect.width()).clamp(0.0, 0.999_999);
    let normalized_y = ((position.y - rect.top()) / rect.height()).clamp(0.0, 0.999_999);
    let x = (normalized_x * dimensions[0] as f32) as usize;
    let y = (normalized_y * dimensions[1] as f32) as usize;
    Some((x, y))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops
)]
pub(super) fn show_entropy_strip(ui: &mut egui::Ui, blocks: &[EntropyBlock]) {
    ui.horizontal(|ui| {
        ui.strong("Entropy");
        ui.weak("0");
        let available = (ui.available_width() - 80.0).max(64.0);
        let width = available / blocks.len().max(1) as f32;
        for block in blocks {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width.max(2.0), 42.0), egui::Sense::hover());
            let intensity = (block.shannon_entropy_bits / 8.0).clamp(0.0, 1.0) as f32;
            let filled = egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.bottom() - (rect.height() * intensity)),
                rect.right_bottom(),
            );
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 29, 47));
            ui.painter()
                .rect_filled(filled, 0.0, egui::Color32::from_rgb(202, 82, 104));
            response.on_hover_text(format!(
                "0x{:x} +{} bytes: {:.2} bits/byte",
                block.offset, block.length, block.shannon_entropy_bits
            ));
        }
        ui.weak("8 bits");
    });
}

pub(super) fn hex_preview(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn ascii_preview(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            }
        })
        .collect()
}
