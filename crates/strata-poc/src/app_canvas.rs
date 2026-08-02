//! Primary, region, and comparison canvas presentation.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    #[allow(clippy::too_many_lines)]
    pub(super) fn show_central(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(13, 18, 22))
            .stroke(egui::Stroke::new(1.0, UI_BORDER))
            .inner_margin(egui::Margin::symmetric(11, 8))
            .show(ui, |ui| {
                let summary = match self.active_view {
                    ViewKind::Discover => match self.workbench_mode {
                        WorkbenchMode::Leads => {
                            format!("{} LEADS / EVIDENCE", self.discovery_findings.len())
                        }
                        WorkbenchMode::Regions => format!(
                            "{} REGIONS / {} LINKS",
                            self.regions.regions().len(),
                            self.regions.relationships().len()
                        ),
                        WorkbenchMode::Compare => format!(
                            "{} CLASSIFIED CHANGES",
                            self.comparison
                                .as_ref()
                                .map_or(0, |comparison| comparison.regions().len())
                        ),
                    },
                    ViewKind::Projection3d => format!(
                        "{} SAMPLES / {}",
                        self.projection_samples.len(),
                        self.projection_composition.geometry.label()
                    ),
                    _ => format!("{} BYTES / EXACT PICKS", self.active_bytes().len()),
                };
                let available = ui.available_width();
                let title_width = (available * 0.22).clamp(86.0, 150.0);
                let summary_width = (available * 0.28).clamp(120.0, 190.0);
                let note_width = (available - title_width - summary_width - 26.0).max(24.0);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [title_width, 20.0],
                        egui::Label::new(
                            egui::RichText::new(self.active_view.title())
                                .strong()
                                .size(14.0),
                        )
                        .truncate(),
                    );
                    ui.separator();
                    ui.add_sized(
                        [note_width, 20.0],
                        egui::Label::new(
                            egui::RichText::new(self.active_view.note())
                                .size(11.0)
                                .color(UI_MUTED),
                        )
                        .truncate(),
                    )
                    .on_hover_text(self.active_view.note());
                    ui.allocate_ui_with_layout(
                        egui::vec2(summary_width, 20.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(summary)
                                        .monospace()
                                        .size(10.0)
                                        .color(UI_TEAL),
                                )
                                .truncate(),
                            );
                        },
                    );
                });
            });

        if let Some(error) = &self.initialization_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            return;
        }

        if self.session_bundle.is_some() && !self.session_attached {
            let (digest, source_length) = self.session_bundle.as_ref().map_or_else(
                || (String::new(), 0),
                |bundle| {
                    (
                        bundle.manifest().source().sha256().to_owned(),
                        bundle.manifest().source().byte_length(),
                    )
                },
            );
            ui.add_space((ui.available_height() * 0.24).max(24.0));
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("SOURCE REQUIRED")
                        .strong()
                        .size(18.0)
                        .color(UI_AMBER),
                );
                ui.add_space(7.0);
                ui.label("The source-free investigation workspace is open.");
                ui.label(
                    egui::RichText::new(format!(
                        "Provide the matching {source_length}-byte source to rebuild analysis and pixels."
                    ))
                    .color(UI_MUTED),
                );
                ui.monospace(format!("SHA-256 {}…", digest_prefix(&digest)));
                ui.add_space(9.0);
                ui.label(
                    egui::RichText::new("Reattach from the Inspector or Session menu")
                        .color(UI_TEAL),
                );
            });
            return;
        }

        if self.active_view != ViewKind::Interleave {
            self.show_dossier_strip(ui);
        }

        if self.active_view == ViewKind::Projection3d {
            self.show_projection_canvas(ui);
            return;
        }
        if self.active_view == ViewKind::Discover {
            egui::ScrollArea::vertical()
                .id_salt(("discovery-canvas", self.workbench_mode))
                .horizontal_scroll_offset(0.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.show_discovery_canvas(ui);
                });
            return;
        }
        if self.active_view == ViewKind::Resonance {
            self.show_resonance_canvas(ui);
            return;
        }

        if self.active_view == ViewKind::RevisionDiff
            && self.loaded_source.is_some()
            && self.comparison_source.is_none()
        {
            self.show_comparison_empty_state(ui);
            return;
        }

        if self.active_view == ViewKind::Structure && self.structure_artifact.is_none() {
            ui.add_space((ui.available_height() * 0.28).max(32.0));
            ui.vertical_centered(|ui| {
                if self.structure_request.is_some() {
                    ui.spinner();
                    ui.strong("Building exact structure artifact");
                } else {
                    ui.colored_label(UI_AMBER, "Structure artifact unavailable");
                }
                ui.weak(&self.structure_status);
            });
            return;
        }

        self.ensure_texture(ui.ctx());
        if let Some(error) = &self.render_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            return;
        }

        if self.active_view == ViewKind::Structure {
            show_entropy_strip(ui, &self.entropy);
            ui.add_space(6.0);
        }

        if self.texture_tiles.is_empty() {
            ui.label("No render texture available");
            return;
        }
        let available = ui.available_size();
        let desired = fitted_size(self.texture_dimensions, available);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
        paint_raster_texture_tiles(
            ui.painter(),
            rect,
            self.texture_dimensions[1],
            &self.texture_tiles,
        );
        self.paint_active_selection(ui, &response);
        self.handle_image_interaction(&response);
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn show_discovery_canvas(&mut self, ui: &mut egui::Ui) {
        if self.discovery_generation != Some(self.source_generation) {
            self.recompute_discovery();
        }
        if let Some(error) = &self.discovery_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            return;
        }

        match self.workbench_mode {
            WorkbenchMode::Regions => {
                self.show_region_canvas(ui);
                return;
            }
            WorkbenchMode::Compare => {
                self.show_comparison_canvas(ui);
                return;
            }
            WorkbenchMode::Leads => {}
        }

        ui.add_space(8.0);
        self.show_discovery_map(ui);
        ui.add_space(10.0);

        if self.discovery_findings.is_empty() {
            ui.heading("No bounded correlation lead was found");
            ui.label(
                "Select a suspicious range in Structure or Resonance, then test a transform branch without modifying the source.",
            );
            return;
        }

        let selected = self.selected_discovery().cloned();
        egui::Frame::new()
            .fill(UI_RAIL_ALT)
            .stroke(egui::Stroke::new(1.0, UI_BORDER))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("EVIDENCE CHAIN").strong().size(11.0));
                    ui.label(
                        egui::RichText::new("OBSERVED  /  RELATED  /  TEST  /  EVIDENCE")
                            .monospace()
                            .size(10.0)
                            .color(UI_TEAL),
                    );
                    ui.label(
                        egui::RichText::new("source bytes remain immutable")
                            .size(10.0)
                            .color(UI_MUTED),
                    );
                });
                ui.separator();
                ui.add_space(5.0);
                if let Some(finding) = selected.as_ref() {
                    self.show_discovery_detail(ui, finding);
                }
            });
    }

    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    pub(super) fn show_region_canvas(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        let desired = egui::vec2(ui.available_width().max(1.0), 330.0);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, UI_CANVAS_BG);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, UI_BORDER),
            egui::StrokeKind::Inside,
        );
        let horizontal_inset = (rect.width() * 0.06).clamp(12.0, 24.0);
        let map = rect.shrink2(egui::vec2(horizontal_inset, 34.0));
        let source_length = self.source_bytes().len().max(1);
        let title = if rect.width() < 520.0 {
            "LIVING REGION MAP"
        } else {
            "LIVING REGION MAP  /  OBJECTS + RELATIONSHIPS"
        };
        painter.text(
            rect.left_top() + egui::vec2(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::monospace(11.0),
            UI_TEXT,
        );
        if rect.width() >= 520.0 {
            painter.text(
                rect.right_top() + egui::vec2(-14.0, 12.0),
                egui::Align2::RIGHT_TOP,
                format!(
                    "{} REGIONS  /  {} LINKS",
                    self.regions.regions().len(),
                    self.regions.relationships().len()
                ),
                egui::FontId::monospace(10.0),
                UI_TEAL,
            );
        }

        let track = egui::Rect::from_min_max(
            egui::pos2(map.left(), map.center().y - 22.0),
            egui::pos2(map.right(), map.center().y + 22.0),
        );
        painter.rect_filled(track, 2.0, egui::Color32::from_rgb(15, 22, 27));
        let mut hits = Vec::new();
        for (index, region) in self.regions.regions().iter().enumerate() {
            let Some(range) = region.provenance.ranges.ranges.first() else {
                continue;
            };
            let start = source_offset_x(range.start, source_length, track);
            let end = source_offset_x(range.end, source_length, track).max(start + 2.0);
            let child_offset = if region.parent_id.is_some() {
                28.0
            } else {
                0.0
            };
            let region_rect = egui::Rect::from_min_max(
                egui::pos2(start, track.top() + child_offset),
                egui::pos2(end, track.bottom() + child_offset),
            );
            let selected = self.selected_region == Some(region.id);
            painter.rect_filled(
                region_rect,
                1.0,
                if selected {
                    UI_AMBER
                } else {
                    region_kind_color(&region.kind)
                },
            );
            painter.rect_stroke(
                region_rect,
                1.0,
                egui::Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        egui::Color32::WHITE
                    } else {
                        UI_BORDER
                    },
                ),
                egui::StrokeKind::Outside,
            );
            if selected || region_rect.width() > 78.0 {
                painter.text(
                    region_rect.center_top() - egui::vec2(0.0, 7.0),
                    egui::Align2::CENTER_BOTTOM,
                    if selected && region_rect.width() > 90.0 {
                        region.label.clone()
                    } else {
                        format!("R{}", index.saturating_add(1))
                    },
                    egui::FontId::monospace(10.0),
                    if selected { UI_AMBER } else { UI_MUTED },
                );
            }
            hits.push((region_rect, region.id));
        }

        for relationship in self.regions.relationships() {
            if !matches!(
                relationship.kind,
                RegionRelationshipKind::XorEncoded
                    | RegionRelationshipKind::Repeats
                    | RegionRelationshipKind::Similar
            ) {
                continue;
            }
            let Some(from) = self.regions.region(relationship.from) else {
                continue;
            };
            let Some(to) = self.regions.region(relationship.to) else {
                continue;
            };
            let (Some(from_range), Some(to_range)) = (
                from.provenance.ranges.ranges.first(),
                to.provenance.ranges.ranges.first(),
            ) else {
                continue;
            };
            let first_x = source_offset_x(
                from_range.start.saturating_add(from_range.len() / 2),
                source_length,
                track,
            );
            let second_x = source_offset_x(
                to_range.start.saturating_add(to_range.len() / 2),
                source_length,
                track,
            );
            let active =
                self.selected_region == Some(from.id) || self.selected_region == Some(to.id);
            painter.add(egui::Shape::line(
                discovery_arc(first_x, second_x, track.top(), 86.0),
                egui::Stroke::new(
                    if active { 2.0 } else { 1.0 },
                    if active { UI_AMBER } else { UI_TEAL },
                ),
            ));
            if active {
                painter.text(
                    egui::pos2((first_x + second_x) * 0.5, track.top() - 94.0),
                    egui::Align2::CENTER_BOTTOM,
                    region_relationship_label(relationship.kind),
                    egui::FontId::monospace(10.0),
                    UI_AMBER,
                );
            }
        }

        painter.text(
            track.left_bottom() + egui::vec2(0.0, 48.0),
            egui::Align2::LEFT_TOP,
            "0x00000000",
            egui::FontId::monospace(10.0),
            UI_MUTED,
        );
        painter.text(
            track.right_bottom() + egui::vec2(0.0, 48.0),
            egui::Align2::RIGHT_TOP,
            format!("0x{source_length:08x}"),
            egui::FontId::monospace(10.0),
            UI_MUTED,
        );
        if response.clicked()
            && let Some(position) = response.interact_pointer_pos()
            && let Some((_, id)) = hits.iter().rev().find(|(hit, _)| hit.contains(position))
        {
            self.select_region(*id);
        }

        ui.add_space(10.0);
        let selected = self
            .selected_region
            .and_then(|id| self.regions.region(id))
            .cloned();
        egui::Frame::new()
            .fill(UI_RAIL_ALT)
            .stroke(egui::Stroke::new(1.0, UI_BORDER))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let Some(region) = selected.as_ref() else {
                    ui.weak("Select a region to inspect its exact bytes and relationships.");
                    return;
                };
                ui.horizontal_wrapped(|ui| {
                    ui.heading(&region.label);
                    ui.colored_label(
                        region_kind_color(&region.kind),
                        region_kind_label(&region.kind),
                    );
                });
                ui.monospace(discovery_range_summary(&region.provenance.ranges.ranges));
                if let Some(range) = region.provenance.ranges.ranges.first()
                    && let (Ok(start), Ok(end)) =
                        (usize::try_from(range.start), usize::try_from(range.end))
                    && let Some(bytes) = self.source_bytes().get(start..end)
                {
                    let preview = bytes.get(..bytes.len().min(48)).unwrap_or(bytes);
                    ui.monospace(hex_preview(preview));
                    ui.monospace(ascii_preview(preview));
                }
            });
    }

    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    pub(super) fn show_comparison_canvas(&mut self, ui: &mut egui::Ui) {
        let Some(comparison) = self.comparison.clone() else {
            ui.weak("Comparison archaeology is unavailable.");
            return;
        };
        ui.add_space(8.0);
        let desired = egui::vec2(ui.available_width().max(1.0), 330.0);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let tiled_diff = self.comparison_artifact.clone();
        painter.rect_filled(rect, 4.0, UI_CANVAS_BG);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, UI_BORDER),
            egui::StrokeKind::Inside,
        );
        let horizontal_inset = (rect.width() * 0.12).clamp(28.0, 72.0);
        let map = rect.shrink2(egui::vec2(horizontal_inset, 44.0));
        let length = if self.comparison_source.is_some() {
            self.source_bytes()
                .len()
                .max(self.comparison_target_bytes().len())
                .max(1)
        } else {
            self.data
                .as_ref()
                .map_or(1, |data| data.revisions.after.len().max(1))
        };
        let left_track = egui::Rect::from_min_max(
            egui::pos2(map.left(), map.top() + 48.0),
            egui::pos2(map.right(), map.top() + 78.0),
        );
        let right_track = egui::Rect::from_min_max(
            egui::pos2(map.left(), map.bottom() - 78.0),
            egui::pos2(map.right(), map.bottom() - 48.0),
        );
        for track in [left_track, right_track] {
            painter.rect_filled(track, 2.0, egui::Color32::from_rgb(15, 22, 27));
            painter.rect_stroke(
                track,
                2.0,
                egui::Stroke::new(1.0, UI_BORDER),
                egui::StrokeKind::Inside,
            );
        }
        let mut tiled_hits = Vec::new();
        if let Some(artifact) = &tiled_diff {
            let overview_track =
                egui::Rect::from_center_size(map.center(), egui::vec2(map.width(), 16.0));
            painter.rect_filled(overview_track, 2.0, egui::Color32::from_rgb(15, 22, 27));
            painter.text(
                overview_track.left_center() - egui::vec2(12.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                if artifact.is_sampled() {
                    "TILED SAMPLE"
                } else {
                    "TILED EXACT"
                },
                egui::FontId::monospace(9.0),
                UI_TEAL,
            );
            let aligned_length = usize::try_from(artifact.aligned_length)
                .unwrap_or(usize::MAX)
                .max(1);
            for tile in &artifact.tiles {
                let tile_rect =
                    comparison_range_rect(tile.coverage, aligned_length, overview_track);
                let changed = tile.changed_bytes();
                let ratio = changed as f32 / tile.change_mask.len().max(1) as f32;
                let color = if changed == 0 {
                    UI_TEAL.gamma_multiply(0.38)
                } else {
                    UI_AMBER.gamma_multiply(ratio.sqrt().mul_add(0.58_f32, 0.42_f32).min(1.0_f32))
                };
                painter.rect_filled(tile_rect, 1.0, color);
                painter.rect_stroke(
                    tile_rect,
                    1.0,
                    egui::Stroke::new(0.5, UI_BORDER),
                    egui::StrokeKind::Inside,
                );
                tiled_hits.push((tile_rect, tile.read_range, tile.coverage, changed));
            }
        }
        painter.text(
            left_track.left_center() - egui::vec2(12.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            "BEFORE",
            egui::FontId::monospace(11.0),
            UI_MUTED,
        );
        painter.text(
            right_track.left_center() - egui::vec2(12.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            "AFTER",
            egui::FontId::monospace(11.0),
            UI_TEXT,
        );
        let title = if rect.width() < 620.0 {
            "COMPARISON ARCHAEOLOGY"
        } else {
            "COMPARISON ARCHAEOLOGY  /  SEMANTIC CHANGE, NOT JUST RED PIXELS"
        };
        painter.text(
            rect.left_top() + egui::vec2(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::monospace(11.0),
            UI_TEXT,
        );

        let mut hits = Vec::new();
        for region in comparison.regions() {
            let selected = self.selected_comparison == Some(region.id);
            let color = comparison_class_color(region.classification);
            let right_range = region.right.ranges.ranges.first();
            let left_range = region
                .left
                .as_ref()
                .and_then(|provenance| provenance.ranges.ranges.first());
            let left_rect =
                left_range.map(|range| comparison_range_rect(*range, length, left_track));
            let right_rect =
                right_range.map(|range| comparison_range_rect(*range, length, right_track));
            if let Some(range_rect) = left_rect {
                painter.rect_filled(range_rect, 1.0, color);
                hits.push((range_rect, region.id));
            }
            if let Some(range_rect) = right_rect {
                painter.rect_filled(range_rect, 1.0, color);
                painter.rect_stroke(
                    range_rect,
                    1.0,
                    egui::Stroke::new(
                        if selected { 2.0 } else { 1.0 },
                        if selected {
                            egui::Color32::WHITE
                        } else {
                            color
                        },
                    ),
                    egui::StrokeKind::Outside,
                );
                hits.push((range_rect, region.id));
            }
            if let (Some(before), Some(after)) = (left_rect, right_rect) {
                painter.line_segment(
                    [before.center_bottom(), after.center_top()],
                    egui::Stroke::new(if selected { 2.0 } else { 1.0 }, color),
                );
            } else if let Some(after) = right_rect {
                painter.line_segment(
                    [
                        egui::pos2(after.center().x, left_track.bottom()),
                        after.center_top(),
                    ],
                    egui::Stroke::new(1.0, color),
                );
            }
            if selected && let Some(after) = right_rect {
                let selected_label = if rect.width() < 520.0 {
                    comparison_class_label(region.classification).to_owned()
                } else {
                    format!(
                        "{} / {}",
                        comparison_class_label(region.classification),
                        region.explanation
                    )
                };
                painter.text(
                    after.center_bottom() + egui::vec2(0.0, 12.0),
                    egui::Align2::CENTER_TOP,
                    selected_label,
                    egui::FontId::monospace(10.0),
                    UI_AMBER,
                );
            }
        }
        if response.clicked()
            && let Some(position) = response.interact_pointer_pos()
        {
            if let Some((_, id)) = hits.iter().rev().find(|(hit, _)| hit.contains(position)) {
                self.select_comparison(*id);
            } else if let Some((_, read_range, coverage, changed)) = tiled_hits
                .iter()
                .find(|(hit, _, _, _)| hit.contains(position))
            {
                match (
                    usize::try_from(read_range.start),
                    usize::try_from(read_range.end),
                ) {
                    (Ok(start), Ok(end)) => {
                        self.selection = start..end;
                        self.status = format!(
                            "Matched tile selected · {} exact bytes · {changed} changed · logical coverage 0x{:08x}..0x{:08x}",
                            read_range.len(),
                            coverage.start,
                            coverage.end
                        );
                        self.queue_focus_refinement(*read_range);
                        self.invalidate_dossier();
                    }
                    _ => {
                        "Matched tile offsets cannot fit this platform"
                            .clone_into(&mut self.status);
                    }
                }
            }
        }

        ui.add_space(10.0);
        let selected = self
            .selected_comparison
            .and_then(|id| comparison.region(id))
            .cloned();
        egui::Frame::new()
            .fill(UI_RAIL_ALT)
            .stroke(egui::Stroke::new(1.0, UI_BORDER))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let Some(region) = selected.as_ref() else {
                    ui.weak("Select a classified region to compare exact source bytes.");
                    return;
                };
                ui.horizontal_wrapped(|ui| {
                    ui.heading(&region.explanation);
                    ui.colored_label(
                        comparison_class_color(region.classification),
                        comparison_class_label(region.classification),
                    );
                });
                let left = region
                    .left
                    .as_ref()
                    .and_then(|provenance| provenance.ranges.ranges.first());
                let right = region.right.ranges.ranges.first();
                ui.monospace(format!(
                    "before {}  ->  after {}",
                    left.map_or_else(|| "--".to_owned(), format_byte_range),
                    right.map_or_else(|| "--".to_owned(), format_byte_range)
                ));
                if let Some(source) = &self.comparison_source {
                    ui.columns(2, |columns| {
                        columns[0].strong("BEFORE");
                        columns[1].strong("AFTER");
                        if let Some(range) = left {
                            columns[0].monospace(preview_range(self.source_bytes(), *range));
                        } else {
                            columns[0].weak("no semantic predecessor");
                        }
                        if let Some(range) = right {
                            columns[1].monospace(preview_range(&source.bytes, *range));
                        }
                    });
                } else if let Some(data) = &self.data {
                    ui.columns(2, |columns| {
                        columns[0].strong("BEFORE");
                        columns[1].strong("AFTER");
                        if let Some(range) = left {
                            columns[0].monospace(preview_range(&data.revisions.before, *range));
                        } else {
                            columns[0].weak("no semantic predecessor");
                        }
                        if let Some(range) = right {
                            columns[1].monospace(preview_range(&data.revisions.after, *range));
                        }
                    });
                }
            });
    }
}
