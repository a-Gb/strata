//! Projection and resonance canvas rendering and interaction.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    pub(super) fn show_projection_canvas(&mut self, ui: &mut egui::Ui) {
        let animation_changed = self.advance_projection_animation(ui.ctx());
        self.ensure_projection_samples();
        if self.projection_samples.is_empty() {
            ui.label("Current domain and parameters produce no valid samples for this source.");
            return;
        }

        let available = ui.available_size();
        let desired = egui::vec2(available.x.max(240.0), available.y.max(240.0));
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
        let camera_changed = self.projection_interaction == ProjectionInteraction::Rotate
            && self.handle_projection_camera(&response);
        let field_suppressed = animation_changed || camera_changed;

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::BLACK);
        let render_settings = ProjectionRenderSettings {
            yaw: self.projection_yaw,
            pitch: self.projection_pitch,
            zoom: self.projection_zoom,
            perspective: self.projection_perspective,
            point_size: self.projection_point_size,
            brightness: self.projection_brightness,
            relief: self.projection_relief,
        };
        let mut screen_points = project_points(
            &self.projection_samples,
            self.projection_composition,
            &self.regions,
            rect,
            render_settings,
        );
        screen_points.sort_by(|first, second| first.depth.total_cmp(&second.depth));

        self.paint_projection_field(&painter, rect, &screen_points, field_suppressed);
        self.paint_projection_path(&painter, &screen_points);
        self.paint_projection_particles(&painter, &screen_points, field_suppressed);
        if self.projection_interaction == ProjectionInteraction::SelectCohort {
            self.handle_projection_cohort(&painter, &response, &screen_points);
        } else {
            self.handle_projection_hover(&painter, &response, &screen_points);
        }

        paint_projection_labels(
            &painter,
            rect,
            ProjectionLabelState {
                point_count: self.projection_samples.len(),
                composition: self.projection_composition,
                relief: self.projection_relief,
                context_light: self.projection_context_light,
                field_radius: self.projection_field_radius,
                field_exposure: self.projection_field_exposure,
                field_contours: self.projection_contour_mode.enabled(),
            },
        );
    }

    pub(super) fn paint_projection_field(
        &mut self,
        painter: &egui::Painter,
        rect: egui::Rect,
        screen_points: &[ScreenProjection],
        suppressed: bool,
    ) {
        if !self.projection_composition.geometry.uses_field() {
            return;
        }
        if suppressed {
            self.projection_field_key = None;
            return;
        }
        let Some(sample) = self.projection_sample_key else {
            return;
        };
        let field_key = ProjectionFieldKey {
            sample,
            composition: self.projection_composition,
            yaw: self.projection_yaw,
            pitch: self.projection_pitch,
            zoom: self.projection_zoom,
            perspective: self.projection_perspective,
            brightness: self.projection_brightness,
            relief: self.projection_relief,
            field_radius: self.projection_field_radius,
            field_exposure: self.projection_field_exposure,
            contour_mode: self.projection_contour_mode,
            canvas_size: rect.size(),
        };
        if self.projection_field_texture.is_none() || self.projection_field_key != Some(field_key) {
            self.refresh_projection_field(painter.ctx(), rect, screen_points);
            self.projection_field_key = Some(field_key);
        }
        let Some(texture) = self.projection_field_texture.as_ref() else {
            return;
        };
        painter.image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::from_white_alpha(self.projection_composition.geometry.field_alpha()),
        );
    }

    pub(super) fn refresh_projection_field(
        &mut self,
        context: &egui::Context,
        rect: egui::Rect,
        screen_points: &[ScreenProjection],
    ) {
        let potential_points: Vec<_> = screen_points
            .iter()
            .map(|point| {
                let depth_strength = (point.depth + 1.0).mul_add(0.18, 0.82).clamp(0.55, 1.3);
                PotentialPoint::new(point.position, point.color, depth_strength)
            })
            .collect();
        let field = render_potential_field(
            rect,
            &potential_points,
            PotentialSettings::new(
                self.projection_field_radius,
                self.projection_field_exposure,
                self.projection_contour_mode.enabled(),
            ),
        );
        if let Some(texture) = self.projection_field_texture.as_mut() {
            texture.set(field, egui::TextureOptions::LINEAR);
        } else {
            let texture = context.load_texture(
                "projection-potential-field",
                field,
                egui::TextureOptions::LINEAR,
            );
            self.projection_field_texture = Some(texture);
        }
    }

    pub(super) fn paint_projection_particles(
        &self,
        painter: &egui::Painter,
        screen_points: &[ScreenProjection],
        surface_fallback: bool,
    ) {
        for point in screen_points {
            let signature_evidence = self.visible_signature_evidence(point);
            let selected = self.projection_composition.overlays.selection
                && (projection_point_is_selected(point, &self.selection)
                    || self
                        .projection_cohort_selection
                        .as_ref()
                        .is_some_and(|cohort| {
                            cohort
                                .members
                                .iter()
                                .any(|member| member.source_offsets == point.source_offsets)
                        }));
            let show_particle = selected
                || signature_evidence.is_some()
                || self.projection_composition.geometry != ProjectionGeometry::Surface
                || surface_fallback;
            if !show_particle {
                continue;
            }
            let focused_plane = point
                .bit_plane
                .is_some_and(|plane| plane == self.projection_composition.parameters.bit_plane);
            let (radius, color) = if selected {
                (point.radius, point.color)
            } else {
                let context_light = if self.projection_composition.channels.opacity
                    == ProjectionOpacityFeature::SelectionContext
                {
                    self.projection_context_light
                } else {
                    1.0
                };
                (
                    if focused_plane {
                        point.radius * 1.18
                    } else {
                        point.radius
                    },
                    projection_context_color(
                        point.color,
                        context_light
                            * if point.bit_plane.is_some() && !focused_plane {
                                0.72
                            } else {
                                1.0
                            },
                    ),
                )
            };
            let voxel = projection_voxel_rect(point.position, radius);
            match self.projection_composition.geometry {
                ProjectionGeometry::Points | ProjectionGeometry::Path => {
                    painter.circle_filled(point.position, radius.max(0.75) * 0.72, color);
                }
                ProjectionGeometry::Voxels | ProjectionGeometry::Surface => {
                    painter.rect_filled(voxel, 0.0, color);
                }
            }
            if selected {
                painter.rect_stroke(
                    voxel.expand(1.5),
                    0.0,
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(235, 179, 66, 232),
                    ),
                    egui::StrokeKind::Outside,
                );
            } else if let Some(evidence) = signature_evidence {
                paint_signature_outline(
                    painter,
                    point,
                    voxel,
                    radius,
                    self.projection_composition.geometry,
                    signature_category_color(evidence),
                );
            } else if self.projection_composition.overlays.regions
                && let Some(region_slot) = point.region_slot
            {
                let stroke = egui::Stroke::new(0.75, projection_region_color(region_slot));
                match self.projection_composition.geometry {
                    ProjectionGeometry::Points | ProjectionGeometry::Path => {
                        painter.circle_stroke(point.position, radius.max(0.75), stroke);
                    }
                    ProjectionGeometry::Voxels | ProjectionGeometry::Surface => {
                        painter.rect_stroke(voxel, 0.0, stroke, egui::StrokeKind::Inside);
                    }
                }
            }
        }
    }

    pub(super) fn paint_projection_path(
        &self,
        painter: &egui::Painter,
        screen_points: &[ScreenProjection],
    ) {
        if self.projection_composition.geometry != ProjectionGeometry::Path {
            return;
        }
        for slot in [ProjectionSlot::A, ProjectionSlot::B] {
            let mut path: Vec<_> = screen_points
                .iter()
                .filter(|point| point.slot == slot)
                .copied()
                .collect();
            path.sort_by_key(|point| (point.bit_plane, point.point_id));
            for pair in path.windows(2) {
                let [first, second] = pair else {
                    continue;
                };
                painter.line_segment(
                    [first.position, second.position],
                    egui::Stroke::new(
                        0.75,
                        projection_context_color(first.color, self.projection_context_light * 0.5),
                    ),
                );
            }
        }
    }

    pub(super) fn handle_projection_cohort(
        &mut self,
        painter: &egui::Painter,
        response: &egui::Response,
        screen_points: &[ScreenProjection],
    ) {
        let mut click_completed = false;
        if response.drag_started()
            && let Some(position) = response.interact_pointer_pos()
        {
            self.projection_cohort_anchor = Some(position - response.drag_delta());
            self.projection_cohort_cursor = Some(position);
        }
        if (response.dragged() || response.drag_stopped())
            && let Some(position) = response.interact_pointer_pos()
        {
            self.projection_cohort_cursor = Some(position);
        }
        if response.clicked()
            && let Some(position) = response.interact_pointer_pos()
        {
            if self.projection_cohort_anchor.is_some() {
                self.projection_cohort_cursor = Some(position);
                click_completed = true;
            } else {
                self.projection_cohort_anchor = Some(position);
                self.projection_cohort_cursor = Some(position);
                "Cohort anchor set; click the opposite corner or drag a new box"
                    .clone_into(&mut self.status);
            }
        }

        self.paint_projection_cohort_box(painter, response.rect);

        if !response.drag_stopped() && !click_completed {
            return;
        }
        let Some(anchor) = self.projection_cohort_anchor.take() else {
            return;
        };
        let Some(cursor) = self.projection_cohort_cursor.take() else {
            return;
        };
        let rectangle = match SelectionRect::from_endpoints(anchor.x, anchor.y, cursor.x, cursor.y)
        {
            Ok(rectangle) => rectangle,
            Err(error) => {
                self.status = format!("Cohort selection needs a non-empty finite box: {error:?}");
                return;
            }
        };
        if self
            .loaded_source
            .as_ref()
            .is_some_and(|source| source.sampled_overview)
        {
            self.projection_cohort_selection = None;
            self.analytical_cohort.clear_selection();
            "Box cohorts across a sampled overview are disabled; use exact voxel picks or Shift-click a recurrence partner"
                .clone_into(&mut self.status);
            return;
        }
        let members: Vec<_> = screen_points
            .iter()
            .map(|point| ProjectedMember {
                screen_x: point.position.x,
                screen_y: point.position.y,
                source_offsets: point.source_offsets,
                source_range: point.analysis_range,
            })
            .collect();
        match select_cohort(rectangle, &members, Some(self.source_bytes())) {
            Ok(cohort) if cohort.members.is_empty() => {
                self.projection_cohort_selection = None;
                self.analytical_cohort.clear_selection();
                "No projection voxels were inside the cohort box".clone_into(&mut self.status);
            }
            Ok(cohort) => {
                let analytical = match materialize_analytical_cohort(
                    &cohort,
                    self.source_bytes(),
                    self.source_generation,
                    self.projection_composition.projection_a,
                ) {
                    Ok(model) => model,
                    Err(error) => {
                        self.status = format!("Cannot explain exact cohort: {error}");
                        return;
                    }
                };
                self.selection = 0..0;
                self.selected_projection = None;
                self.selected_digram = None;
                self.status = format!(
                    "Selected {} stable voxels from {} exact source bytes{}",
                    cohort.metrics.member_count,
                    cohort.metrics.unique_byte_count,
                    if cohort.truncated { " (bounded)" } else { "" }
                );
                self.analytical_cohort = analytical;
                self.projection_cohort_selection = Some(cohort);
            }
            Err(error) => {
                self.status = format!("Cannot materialize cohort: {error:?}");
            }
        }
    }

    pub(super) fn paint_projection_cohort_box(&self, painter: &egui::Painter, canvas: egui::Rect) {
        let (Some(anchor), Some(cursor)) =
            (self.projection_cohort_anchor, self.projection_cohort_cursor)
        else {
            return;
        };
        let selection_rect = egui::Rect::from_two_pos(anchor, cursor).intersect(canvas);
        painter.rect_filled(
            selection_rect,
            1.0,
            egui::Color32::from_rgba_unmultiplied(41, 166, 225, 24),
        );
        painter.rect_stroke(
            selection_rect,
            1.0,
            egui::Stroke::new(1.0, UI_CYAN),
            egui::StrokeKind::Inside,
        );
    }

    pub(super) fn handle_projection_hover(
        &mut self,
        painter: &egui::Painter,
        response: &egui::Response,
        screen_points: &[ScreenProjection],
    ) {
        let hovered = response
            .hover_pos()
            .and_then(|position| closest_screen_point(screen_points, position));
        let Some(point) = hovered else {
            return;
        };
        painter.rect_stroke(
            projection_voxel_rect(point.position, point.radius).expand(4.0),
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(196)),
            egui::StrokeKind::Outside,
        );
        let place_left = point.position.x > painter.clip_rect().center().x;
        let place_below = point.position.y < painter.clip_rect().center().y;
        let anchor = point.position
            + egui::vec2(
                if place_left { -10.0 } else { 10.0 },
                if place_below { 10.0 } else { -10.0 },
            );
        let alignment = match (place_left, place_below) {
            (true, true) => egui::Align2::RIGHT_TOP,
            (true, false) => egui::Align2::RIGHT_BOTTOM,
            (false, true) => egui::Align2::LEFT_TOP,
            (false, false) => egui::Align2::LEFT_BOTTOM,
        };
        let p1_detail = point.p1.map_or_else(String::new, |feature| {
            let partner = feature.partner_range.map_or_else(
                || "partner --".to_owned(),
                |range| format!("partner 0x{:08x}..0x{:08x}", range.start, range.end),
            );
            format!(
                "\n{partner} · sim {:.1}% · match {} B\nbin {} · magnitude {:.3} · hierarchy d{}",
                feature.recurrence_score * 100.0,
                feature.match_length,
                feature.dominant_frequency_bin,
                feature.spectral_magnitude,
                feature.hierarchy_depth
            )
        });
        painter.text(
            anchor,
            alignment,
            format!(
                "{}{}  id:{:016x}\n0x{:08x}..0x{:08x}  [{} / {} / {}]{}",
                point.slot.label(),
                point
                    .bit_plane
                    .map_or_else(String::new, |plane| format!(" / BIT {plane}")),
                point.point_id,
                point.analysis_range[0],
                point.analysis_range[1],
                point.source_offsets[0],
                point.source_offsets[1],
                point.source_offsets[2],
                p1_detail
            ),
            egui::FontId::monospace(11.0),
            egui::Color32::WHITE,
        );
        if response.clicked() && !response.double_clicked() {
            let partner = response
                .ctx
                .input(|input| input.modifiers.shift)
                .then(|| {
                    let range = point.p1?.partner_range?;
                    Some(usize::try_from(range.start).ok()?..usize::try_from(range.end).ok()?)
                })
                .flatten();
            self.selected_projection = Some(point.source_offsets);
            self.selection = partner.unwrap_or(point.analysis_range[0]..point.analysis_range[1]);
            self.selected_digram = None;
            if let (Ok(start), Ok(end)) = (
                u64::try_from(self.selection.start),
                u64::try_from(self.selection.end),
            ) && let Ok(focus) = ByteRange::new(start, end)
            {
                self.queue_focus_refinement(focus);
            }
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines
    )]
    pub(super) fn show_resonance_canvas(&mut self, ui: &mut egui::Ui) {
        self.ensure_resonance_layers();
        if let Some(error) = &self.render_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            return;
        }
        if self.resonance_layers.is_empty() {
            ui.label("Select at least one source byte to create a resonance query.");
            return;
        }

        let available = ui.available_size();
        let desired = egui::vec2(available.x.max(420.0), available.y.max(360.0));
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(2, 5, 12));

        let plot = egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(74.0, 42.0),
            rect.right_bottom() - egui::vec2(74.0, 38.0),
        );
        let layer_count = self.resonance_layers.len().max(1);
        let gap = 8.0;
        let row_height = ((plot.height() - (gap * (layer_count.saturating_sub(1) as f32)))
            / layer_count as f32)
            .max(24.0);
        let source_length = self.source_bytes().len().max(1) as f32;
        let mut screen_points = Vec::new();

        painter.text(
            rect.left_top() + egui::vec2(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            "SELECTION AS QUERY  /  BRIGHT RIDGES SURVIVE ACROSS SCALES",
            egui::FontId::monospace(11.0),
            egui::Color32::from_gray(170),
        );
        painter.text(
            rect.right_top() + egui::vec2(-14.0, 12.0),
            egui::Align2::RIGHT_TOP,
            resonance_metric_label(self.resonance_metric).to_uppercase(),
            egui::FontId::monospace(11.0),
            egui::Color32::from_rgb(74, 190, 168),
        );

        for (layer_index, layer) in self.resonance_layers.iter().enumerate() {
            let top = (layer_index as f32).mul_add(row_height + gap, plot.top());
            let row = egui::Rect::from_min_size(
                egui::pos2(plot.left(), top),
                egui::vec2(plot.width(), row_height),
            );
            painter.rect_filled(row, 0.0, egui::Color32::from_rgb(7, 14, 25));
            for score in [0.5_f32, 0.75, 1.0] {
                let y = row.height().mul_add(-score, row.bottom());
                painter.line_segment(
                    [egui::pos2(row.left(), y), egui::pos2(row.right(), y)],
                    egui::Stroke::new(0.5, egui::Color32::from_gray(34)),
                );
            }
            painter.text(
                egui::pos2(row.left() - 10.0, row.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{} B", layer.window_size),
                egui::FontId::monospace(11.0),
                egui::Color32::from_gray(185),
            );
            painter.text(
                egui::pos2(row.right() + 10.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                format!("Δ{}", layer.sampled_step),
                egui::FontId::monospace(10.0),
                egui::Color32::from_gray(105),
            );

            let cell_width =
                (layer.sampled_step as f32 / source_length * row.width()).clamp(1.0, 18.0);
            let mut waveform = Vec::with_capacity(layer.matches.len());
            for candidate in &layer.matches {
                let normalized = (candidate.offset as f32 / source_length).clamp(0.0, 1.0);
                let x = normalized.mul_add(row.width(), row.left());
                let score = candidate.score.clamp(0.0, 1.0) as f32;
                let point = egui::pos2(x, row.height().mul_add(-score, row.bottom()));
                let heat = egui::Rect::from_center_size(
                    egui::pos2(x, row.center().y),
                    egui::vec2(cell_width, row.height()),
                )
                .intersect(row);
                painter.rect_filled(heat, 0.0, resonance_color(score, 0.58));
                waveform.push(point);
                if score >= 0.86 {
                    painter.circle_filled(point, 2.0, resonance_color(score, 1.0));
                }
                screen_points.push(ResonanceScreenPoint {
                    position: point,
                    probe_offset: layer.probe_offset,
                    candidate_offset: candidate.offset,
                    window_size: candidate.length,
                    score: candidate.score,
                    metric: self.resonance_metric,
                });
            }
            if waveform.len() >= 2 {
                painter.add(egui::Shape::line(
                    waveform,
                    egui::Stroke::new(
                        0.7,
                        egui::Color32::from_rgba_unmultiplied(145, 225, 237, 95),
                    ),
                ));
            }
        }

        let probe_offset = self
            .resonance_layers
            .first()
            .map_or(0, |layer| layer.probe_offset);
        let probe_x = (probe_offset as f32 / source_length)
            .clamp(0.0, 1.0)
            .mul_add(plot.width(), plot.left());
        painter.line_segment(
            [
                egui::pos2(probe_x, plot.top() - 6.0),
                egui::pos2(probe_x, plot.bottom() + 6.0),
            ],
            egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 190, 58)),
        );
        painter.text(
            egui::pos2(probe_x, plot.top() - 10.0),
            egui::Align2::CENTER_BOTTOM,
            format!("probe 0x{probe_offset:x}"),
            egui::FontId::monospace(10.0),
            egui::Color32::from_rgb(255, 190, 58),
        );
        for (fraction, label) in [
            (0.0_f32, "0%"),
            (0.25, "25%"),
            (0.5, "50%"),
            (0.75, "75%"),
            (1.0, "100% address"),
        ] {
            let x = plot.width().mul_add(fraction, plot.left());
            painter.text(
                egui::pos2(x, plot.bottom() + 12.0),
                egui::Align2::CENTER_TOP,
                label,
                egui::FontId::monospace(10.0),
                egui::Color32::from_gray(105),
            );
        }

        let hovered = response
            .hover_pos()
            .and_then(|position| closest_resonance_point(&screen_points, position));
        if let Some(point) = hovered {
            painter.circle_stroke(
                point.position,
                5.5,
                egui::Stroke::new(1.5, egui::Color32::WHITE),
            );
            painter.line_segment(
                [
                    egui::pos2(probe_x, plot.top()),
                    egui::pos2(point.position.x, point.position.y),
                ],
                egui::Stroke::new(
                    0.8,
                    egui::Color32::from_rgba_unmultiplied(255, 190, 58, 110),
                ),
            );
            painter.text(
                point.position + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                format!(
                    "0x{:x}  {} B  {:.1}%",
                    point.candidate_offset,
                    point.window_size,
                    point.score * 100.0
                ),
                egui::FontId::monospace(11.0),
                egui::Color32::WHITE,
            );
            if response.clicked() {
                if let (Ok(start), Ok(length)) = (
                    usize::try_from(point.candidate_offset),
                    usize::try_from(point.window_size),
                ) {
                    let end = start.saturating_add(length).min(self.source_bytes().len());
                    self.selected_resonance = Some(SelectedResonance {
                        probe_offset: point.probe_offset,
                        candidate_offset: point.candidate_offset,
                        window_size: point.window_size,
                        score: point.score,
                        metric: point.metric,
                    });
                    self.selection = start..end;
                    self.selected_digram = None;
                    self.selected_projection = None;
                    self.resonance_key = None;
                    self.invalidate_texture();
                    self.status = format!(
                        "Jumped to {:.1}% {} echo at 0x{:x}",
                        point.score * 100.0,
                        resonance_metric_label(point.metric),
                        point.candidate_offset
                    );
                }
            }
        }
    }

    pub(super) fn advance_projection_animation(&mut self, context: &egui::Context) -> bool {
        let morphing = self.projection_auto_morph
            && self.projection_composition.compare_mode == ProjectionCompareMode::Morph;
        if !(self.projection_spin || morphing) {
            return false;
        }
        let delta_time = context.input(|input| input.stable_dt.min(0.05));
        if self.projection_spin {
            self.projection_yaw += delta_time * self.projection_speed;
        }
        if morphing {
            self.projection_phase += delta_time * self.projection_speed * 1.8;
            self.projection_composition.mix = self.projection_phase.sin().mul_add(0.5, 0.5);
        }
        context.request_repaint();
        true
    }

    pub(super) fn handle_projection_camera(&mut self, response: &egui::Response) -> bool {
        let dragged = response.dragged_by(egui::PointerButton::Primary);
        if dragged {
            let delta = response.drag_delta();
            self.projection_yaw += delta.x * 0.009;
            self.projection_pitch = delta
                .y
                .mul_add(0.009, self.projection_pitch)
                .clamp(-1.48, 1.48);
            self.projection_spin = false;
        }
        let scroll = if response.hovered() {
            response.ctx.input(|input| input.smooth_scroll_delta.y)
        } else {
            0.0
        };
        let zoomed = scroll.abs() > f32::EPSILON;
        if zoomed {
            self.projection_zoom =
                (self.projection_zoom * (scroll * 0.001_5).exp()).clamp(0.2, 5.0);
        }
        let reset = response.double_clicked();
        if reset {
            self.projection_yaw = -0.72;
            self.projection_pitch = 0.38;
            self.projection_zoom = 0.92;
        }
        dragged || zoomed || reset
    }

    #[allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]
    pub(super) fn paint_active_selection(&self, ui: &egui::Ui, response: &egui::Response) {
        let Some(mapping) = self.active_mapping else {
            return;
        };
        let cell = match mapping {
            ActiveMapping::Raster(layout) => layout.offset_to_pixel(self.selection.start),
            ActiveMapping::BitPlane(layout) => layout.offset_to_pixel(self.selection.start),
            ActiveMapping::Digram => self
                .selected_digram
                .map(|(first, second)| (usize::from(first), usize::from(second))),
        };
        let Some((x, y)) = cell else {
            return;
        };
        let cell_width = response.rect.width() / self.texture_dimensions[0] as f32;
        let cell_height = response.rect.height() / self.texture_dimensions[1] as f32;
        let minimum = egui::pos2(
            response.rect.left() + (x as f32 * cell_width),
            response.rect.top() + (y as f32 * cell_height),
        );
        let rect = egui::Rect::from_min_size(
            minimum,
            egui::vec2(cell_width.max(2.0), cell_height.max(2.0)),
        );
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }

    pub(super) fn handle_image_interaction(&mut self, response: &egui::Response) {
        if !(response.clicked() || response.drag_started() || response.dragged()) {
            if response.drag_stopped() {
                self.drag_anchor = None;
            }
            return;
        }
        let Some(position) = response.interact_pointer_pos() else {
            return;
        };
        let Some((x, y)) = image_pixel(position, response.rect, self.texture_dimensions) else {
            return;
        };
        let Some(mapping) = self.active_mapping else {
            return;
        };

        match mapping {
            ActiveMapping::Digram => {
                let (Ok(first), Ok(second)) = (u8::try_from(x), u8::try_from(y)) else {
                    return;
                };
                self.selected_digram = Some((first, second));
            }
            ActiveMapping::Raster(layout) => {
                if let Some(offset) = layout.pixel_to_offset(x, y) {
                    self.update_selection_from_pointer(offset, response);
                }
            }
            ActiveMapping::BitPlane(layout) => {
                if let Some(offset) = layout.pixel_to_offset(x, y) {
                    self.update_selection_from_pointer(offset, response);
                }
            }
        }
        if response.drag_stopped() {
            self.drag_anchor = None;
        }
    }

    pub(super) fn update_selection_from_pointer(
        &mut self,
        offset: usize,
        response: &egui::Response,
    ) {
        if response.clicked() {
            self.selection = offset..offset.saturating_add(1);
            self.drag_anchor = None;
        } else {
            if response.drag_started() || self.drag_anchor.is_none() {
                self.drag_anchor = Some(offset);
            }
            if let Some(anchor) = self.drag_anchor {
                self.selection = anchor.min(offset)..anchor.max(offset).saturating_add(1);
            }
        }
        self.selected_digram = None;
        self.invalidate_texture();
    }

    pub(super) fn prepare_frame(&mut self, ui: &egui::Ui) {
        self.poll_file_loads();
        self.handle_dropped_files(ui.ctx());
        let save_session = ui.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::S,
            ))
        });
        let open_source = ui.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::O,
            ))
        });
        if save_session {
            self.save_session();
        } else if open_source {
            self.browse_primary_source();
        }
        self.poll_production_analysis();
        self.poll_source_digests();
        self.ensure_dossier();
        if self.structure_request.is_some() || self.source_digest_request.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
        self.poll_video_export();
        if self.video_export_receiver.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }
}

fn paint_signature_outline(
    painter: &egui::Painter,
    point: &ScreenProjection,
    voxel: egui::Rect,
    radius: f32,
    geometry: ProjectionGeometry,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.15, color);
    match geometry {
        ProjectionGeometry::Points | ProjectionGeometry::Path => {
            painter.circle_stroke(point.position, radius.max(0.75) + 0.8, stroke);
        }
        ProjectionGeometry::Voxels | ProjectionGeometry::Surface => {
            painter.rect_stroke(voxel.expand(0.8), 0.0, stroke, egui::StrokeKind::Outside);
        }
    }
}
