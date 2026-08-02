//! Discovery-map presentation and analyst hypothesis actions.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    pub(super) fn cycle_discovery(&mut self, direction: i32) {
        if self.discovery_findings.is_empty() {
            return;
        }
        let current = self
            .discovery_selected
            .and_then(|selected| {
                self.discovery_findings
                    .iter()
                    .position(|finding| finding.id == selected)
            })
            .unwrap_or(0);
        let count = self.discovery_findings.len();
        let next = if direction < 0 {
            current
                .checked_sub(1)
                .unwrap_or_else(|| count.saturating_sub(1))
        } else {
            current.saturating_add(1) % count
        };
        if let Some(finding) = self.discovery_findings.get(next) {
            self.select_discovery_finding(finding.id);
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::suboptimal_flops,
        clippy::too_many_lines
    )]
    pub(super) fn show_discovery_map(&mut self, ui: &mut egui::Ui) {
        let desired = egui::vec2(ui.available_width().max(1.0), 285.0);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, UI_CANVAS_BG);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, UI_BORDER),
            egui::StrokeKind::Inside,
        );

        let source_length = self.source_bytes().len().max(1);
        let map = rect.shrink2(egui::vec2(22.0, 18.0));
        for step in 0..=8 {
            let fraction = step as f32 / 8.0;
            let x = map.width().mul_add(fraction, map.left());
            painter.line_segment(
                [
                    egui::pos2(x, map.top() + 30.0),
                    egui::pos2(x, map.bottom() - 30.0),
                ],
                egui::Stroke::new(
                    if step % 2 == 0 { 0.8 } else { 0.5 },
                    egui::Color32::from_rgba_unmultiplied(72, 92, 105, 45),
                ),
            );
        }
        let track_center = map.center().y + 40.0;
        let track = egui::Rect::from_min_max(
            egui::pos2(map.left(), track_center - 11.0),
            egui::pos2(map.right(), track_center + 11.0),
        );
        painter.rect_filled(track, 2.0, egui::Color32::from_rgb(18, 27, 34));
        painter.rect_stroke(
            track,
            2.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(53, 78, 91)),
            egui::StrokeKind::Inside,
        );

        for block in &self.entropy {
            let start = source_offset_x(block.offset, source_length, track);
            let end = source_offset_x(
                block.offset.saturating_add(block.length),
                source_length,
                track,
            );
            let intensity = (block.shannon_entropy_bits / 8.0).clamp(0.0, 1.0) as f32;
            let color = egui::Color32::from_rgb(
                (30.0 + intensity * 42.0) as u8,
                (47.0 + intensity * 90.0) as u8,
                (59.0 + intensity * 116.0) as u8,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(start, track.top()),
                    egui::pos2(end.max(start + 1.0), track.bottom()),
                ),
                0.0,
                color,
            );
        }

        let selected = self.discovery_selected;
        for (finding_index, finding) in self.discovery_findings.iter().enumerate() {
            let is_selected = selected == Some(finding.id);
            if let (Some(first), Some(second)) =
                (finding.source_ranges.first(), finding.source_ranges.get(1))
            {
                let first_x = source_offset_x(
                    first.start.saturating_add(first.len() / 2),
                    source_length,
                    track,
                );
                let second_x = source_offset_x(
                    second.start.saturating_add(second.len() / 2),
                    source_length,
                    track,
                );
                let arc_height = 68.0 + (finding_index % 4) as f32 * 22.0;
                let color = if is_selected {
                    UI_AMBER
                } else {
                    discovery_lead_color(finding)
                };
                painter.add(egui::Shape::line(
                    discovery_arc(first_x, second_x, track.top(), arc_height),
                    egui::Stroke::new(if is_selected { 2.0 } else { 1.0 }, color),
                ));
                if is_selected {
                    for x in [first_x, second_x] {
                        painter.rect_filled(
                            egui::Rect::from_center_size(
                                egui::pos2(x, track.center().y),
                                egui::vec2(8.0, 30.0),
                            ),
                            1.0,
                            UI_AMBER,
                        );
                    }
                    let relation = discovery_transform(finding).map_or_else(
                        || "EXACT RANGE CORRELATION".to_owned(),
                        |transform| {
                            format!("{} / {} BYTE LINK", transform_label(transform), first.len())
                        },
                    );
                    painter.text(
                        egui::pos2((first_x + second_x) * 0.5, track.top() - arc_height - 8.0),
                        egui::Align2::CENTER_BOTTOM,
                        relation,
                        egui::FontId::monospace(11.0),
                        UI_AMBER,
                    );
                }
            }

            for range in &finding.source_ranges {
                let start = source_offset_x(range.start, source_length, track);
                let end = source_offset_x(range.end, source_length, track);
                let color = if is_selected {
                    UI_AMBER
                } else {
                    discovery_lead_color(finding).gamma_multiply(0.58)
                };
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(start, track.top() - if is_selected { 4.0 } else { 0.0 }),
                        egui::pos2(end.max(start + 2.0), track.bottom()),
                    ),
                    0.0,
                    color,
                );
            }
        }

        painter.text(
            egui::pos2(track.left(), map.top()),
            egui::Align2::LEFT_TOP,
            "CORRELATION GRAPH  /  BYTE ADDRESS SPACE",
            egui::FontId::monospace(11.0),
            UI_TEXT,
        );
        painter.text(
            track.left_bottom() + egui::vec2(0.0, 3.0),
            egui::Align2::LEFT_TOP,
            "0x00000000",
            egui::FontId::monospace(10.0),
            egui::Color32::from_gray(132),
        );
        painter.text(
            track.right_bottom() + egui::vec2(0.0, 3.0),
            egui::Align2::RIGHT_TOP,
            format!("0x{source_length:08x}"),
            egui::FontId::monospace(10.0),
            egui::Color32::from_gray(132),
        );
        painter.text(
            egui::pos2(map.right(), map.top()),
            egui::Align2::RIGHT_TOP,
            "ENTROPY TEXTURE  /  EXACT LINKS  /  AMBER ACTIVE",
            egui::FontId::monospace(10.0),
            UI_MUTED,
        );
        painter.text(
            egui::pos2(map.left(), map.bottom() - 4.0),
            egui::Align2::LEFT_BOTTOM,
            "CLICK A HIGHLIGHTED RANGE TO REMAP THE SHARED SELECTION",
            egui::FontId::monospace(9.5),
            UI_MUTED,
        );

        if response.clicked() {
            if let Some(position) = response.interact_pointer_pos() {
                let normalized = ((position.x - track.left()) / track.width()).clamp(0.0, 1.0);
                let offset = (normalized * source_length as f32) as u64;
                let hit = self.discovery_findings.iter().find_map(|finding| {
                    finding
                        .source_ranges
                        .iter()
                        .any(|range| range.contains(offset))
                        .then_some(finding.id)
                });
                if let Some(finding_id) = hit {
                    self.select_discovery_finding(finding_id);
                } else {
                    let offset = usize::try_from(offset)
                        .unwrap_or_else(|_| source_length.saturating_sub(1))
                        .min(source_length.saturating_sub(1));
                    self.selection = offset..offset.saturating_add(1).min(source_length);
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn show_discovery_detail(&mut self, ui: &mut egui::Ui, finding: &WorkbenchLead) {
        let finding_id = investigation_finding_id(finding.id, 0);
        let status = self
            .investigation
            .finding(finding_id)
            .map_or(FindingStatus::Candidate, |record| record.status);
        ui.heading(discovery_title(finding));
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                discovery_kind_color(finding.kind),
                discovery_kind_label(finding.kind),
            );
            ui.monospace(format!("{:.0}%", finding.confidence * 100.0));
            ui.label(finding_status_label(status));
        });
        ui.label(discovery_evidence_summary(finding));
        ui.colored_label(UI_TEAL, format!("Next: {}", discovery_next_action(finding)));
        ui.add_space(4.0);
        ui.monospace("OBSERVED  ->  RELATED  ->  TEST  ->  EVIDENCE");
        ui.add_space(6.0);

        let mut selected_range = None;
        ui.horizontal_wrapped(|ui| {
            for (index, range) in finding.source_ranges.iter().enumerate() {
                if ui
                    .button(format!(
                        "Range {}  0x{:x}..0x{:x}",
                        index.saturating_add(1),
                        range.start,
                        range.end
                    ))
                    .clicked()
                {
                    selected_range = Some(index);
                }
            }
        });
        if let Some(index) = selected_range {
            self.select_discovery_range(index);
        }

        let mut open_view = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Open Structure").clicked() {
                open_view = Some(ViewKind::Structure);
            }
            if ui.button("Compare Resonance").clicked() {
                open_view = Some(ViewKind::Resonance);
            }
            if ui.button("Map in 3D").clicked() {
                open_view = Some(ViewKind::Projection3d);
            }
        });
        if let Some(view) = open_view {
            self.open_discovery_in(view);
        }

        ui.separator();
        if discovery_transform(finding).is_some() {
            let hypothesis_id = investigation_hypothesis_id(finding.id);
            let hypothesis_status = self
                .investigation
                .hypothesis(hypothesis_id)
                .map_or(HypothesisStatus::Draft, |hypothesis| hypothesis.status);
            ui.strong("Reversible transform branch");
            ui.horizontal_wrapped(|ui| {
                ui.monospace(discovery_hypothesis_statement(finding));
                ui.label(hypothesis_status_label(hypothesis_status));
            });
            ui.weak("Derived preview only · source bytes remain unchanged · exact inverse is the same XOR.");
            let mut test = false;
            let mut promote = false;
            let mut reject = false;
            ui.horizontal_wrapped(|ui| {
                test = ui.button("Test transform").clicked();
                promote = ui
                    .add_enabled(
                        status != FindingStatus::Promoted,
                        egui::Button::new("Promote evidence"),
                    )
                    .clicked();
                reject = ui.button("Reject hypothesis").clicked();
            });
            if test {
                self.test_discovery_transform(finding);
            }
            if promote {
                self.promote_discovery_finding(finding);
            }
            if reject {
                self.reject_discovery_finding(finding);
            }
            if self.discovery_preview_transform {
                self.show_discovery_transform_preview(ui, finding);
            }
        } else {
            let mut promote = false;
            let mut dismiss = false;
            ui.horizontal_wrapped(|ui| {
                promote = ui
                    .add_enabled(
                        status != FindingStatus::Promoted,
                        egui::Button::new("Promote evidence"),
                    )
                    .clicked();
                dismiss = ui.button("Dismiss lead").clicked();
            });
            if promote {
                self.promote_discovery_finding(finding);
            }
            if dismiss {
                self.reject_discovery_finding(finding);
            }
        }
    }

    pub(super) fn show_discovery_transform_preview(
        &self,
        ui: &mut egui::Ui,
        finding: &WorkbenchLead,
    ) {
        let Some(transform) = discovery_transform(finding) else {
            return;
        };
        let Some(range) = discovery_transform_range(finding) else {
            return;
        };
        let (Ok(start), Ok(end)) = (usize::try_from(range.start), usize::try_from(range.end))
        else {
            return;
        };
        let Some(encoded) = self.source_bytes().get(start..end) else {
            return;
        };
        let encoded = encoded.get(..encoded.len().min(48)).unwrap_or(encoded);
        let decoded = apply_reversible_transform(encoded, transform);
        ui.add_space(6.0);
        ui.strong(format!(
            "Counterfactual preview · {}",
            transform_label(transform)
        ));
        ui.monospace(format!("raw      {}", hex_preview(encoded)));
        ui.monospace(format!("derived  {}", hex_preview(&decoded)));
        ui.monospace(format!("ASCII    {}", ascii_preview(&decoded)));
        if let Ok(evaluation) = evaluate_transform_candidate(self.source_bytes(), range, transform)
        {
            ui.separator();
            ui.monospace(format!(
                "text       {:>5.1}% -> {:>5.1}%   {:+.1} pp",
                evaluation.before.text_likelihood * 100.0,
                evaluation.after.text_likelihood * 100.0,
                evaluation.text_likelihood_delta * 100.0
            ));
            ui.monospace(format!(
                "entropy    {:>5.2} -> {:>5.2}   {:+.2} bits",
                evaluation.before.entropy_bits,
                evaluation.after.entropy_bits,
                evaluation.entropy_delta_bits
            ));
            ui.colored_label(
                match evaluation.assessment {
                    TransformAssessment::Supported => UI_TEAL,
                    TransformAssessment::Neutral => UI_MUTED,
                    TransformAssessment::Contradicted => UI_AMBER,
                },
                format!(
                    "Measured result: {} · exact XOR correlation remains separate evidence",
                    transform_assessment_label(evaluation.assessment)
                ),
            );
        }
    }

    pub(super) fn test_discovery_transform(&mut self, finding: &WorkbenchLead) {
        let Some(transform) = discovery_transform(finding) else {
            "This lead does not define a reversible transform".clone_into(&mut self.status);
            return;
        };
        let Some(range) = discovery_transform_range(finding) else {
            "The transform lead has no exact evaluation range".clone_into(&mut self.status);
            return;
        };
        let evaluation = match evaluate_transform_candidate(self.source_bytes(), range, transform) {
            Ok(evaluation) => evaluation,
            Err(error) => {
                self.status = format!("Cannot evaluate transform branch: {error}");
                return;
            }
        };
        let assessment = if matches!(
            finding.evidence,
            WorkbenchEvidence::XorCorrelatedTransform { .. }
        ) {
            TransformAssessment::Supported
        } else {
            evaluation.assessment
        };
        self.add_transform_branch(
            finding.id,
            format!(
                "{} · {}",
                discovery_title(finding),
                transform_label(transform)
            ),
            range,
            &evaluation,
            assessment,
        );
        self.discovery_preview_transform = true;
        let hypothesis_id = investigation_hypothesis_id(finding.id);
        if let Err(error) = self
            .investigation
            .set_hypothesis_status(hypothesis_id, HypothesisStatus::Tested)
        {
            self.status = format!("Transform tested, but hypothesis state failed: {error}");
        }
    }

    pub(super) fn test_manual_branch(&mut self) {
        let source_length = self.source_bytes().len();
        let selection = self.clamped_selection(source_length);
        if selection.is_empty() {
            "Select at least one exact source byte before testing a branch"
                .clone_into(&mut self.status);
            return;
        }
        let (Ok(start), Ok(end)) = (u64::try_from(selection.start), u64::try_from(selection.end))
        else {
            "Selected range cannot fit branch provenance".clone_into(&mut self.status);
            return;
        };
        let Ok(range) = ByteRange::new(start, end) else {
            "Selected range is invalid for branch evaluation".clone_into(&mut self.status);
            return;
        };
        let transform = ReversibleTransform::XorByte(self.branch_key);
        let evaluation = match evaluate_transform_candidate(self.source_bytes(), range, transform) {
            Ok(evaluation) => evaluation,
            Err(error) => {
                self.status = format!("Cannot evaluate manual branch: {error}");
                return;
            }
        };
        let id = manual_branch_lead_id(range, self.branch_key, self.source_generation);
        self.add_transform_branch(
            id,
            format!(
                "{} · 0x{:x}..0x{:x}",
                transform_label(transform),
                range.start,
                range.end
            ),
            range,
            &evaluation,
            evaluation.assessment,
        );
    }

    pub(super) fn add_transform_branch(
        &mut self,
        id: WorkbenchLeadId,
        label: String,
        range: ByteRange,
        evaluation: &TransformEvaluation,
        assessment: TransformAssessment,
    ) {
        let provenance = discovery_provenance(&[range], self.source_generation);
        let branch = match build_branch_from_evaluation(id, label, provenance, evaluation) {
            Ok(branch) => branch,
            Err(error) => {
                self.status = format!("Cannot build reproducible branch: {error}");
                return;
            }
        };
        let branch_id = branch.id;
        match self.branches.add_branch(branch) {
            Ok(()) => {
                self.selected_branch = Some(branch_id);
                self.branch_assessments.insert(branch_id, assessment);
                self.invalidate_dossier();
                self.status = format!(
                    "Created {} branch: text {:+.1} pp, entropy {:+.2} bits ({})",
                    transform_label(evaluation.transform),
                    evaluation.text_likelihood_delta * 100.0,
                    evaluation.entropy_delta_bits,
                    transform_assessment_label(assessment)
                );
            }
            Err(WorkbenchError::DuplicateId) => {
                self.selected_branch = Some(branch_id);
                self.status = format!("Reopened existing transform branch {}", branch_id.0);
            }
            Err(error) => {
                self.status = format!("Cannot add transform branch: {error}");
            }
        }
    }

    pub(super) fn promote_discovery_finding(&mut self, finding: &WorkbenchLead) {
        let finding_id = investigation_finding_id(finding.id, 0);
        let evidence = Evidence {
            id: investigation_evidence_id(finding.id),
            claim: format!(
                "{}: {}",
                discovery_title(finding),
                discovery_evidence_summary(finding)
            ),
            provenance: discovery_provenance(&finding.source_ranges, self.source_generation),
            finding_id: Some(finding_id),
        };
        match self.investigation.promote_finding(evidence) {
            Ok(()) => {
                let hypothesis_id = investigation_hypothesis_id(finding.id);
                if self.investigation.hypothesis(hypothesis_id).is_some() {
                    let _result = self
                        .investigation
                        .set_hypothesis_status(hypothesis_id, HypothesisStatus::Supported);
                }
                self.status = format!(
                    "Promoted {} with {} exact range(s)",
                    discovery_title(finding),
                    finding.source_ranges.len()
                );
                self.invalidate_dossier();
            }
            Err(InvestigationError::DuplicateId) => {
                "This finding is already evidence".clone_into(&mut self.status);
            }
            Err(error) => {
                self.status = format!("Cannot promote finding: {error}");
            }
        }
    }

    pub(super) fn reject_discovery_finding(&mut self, finding: &WorkbenchLead) {
        let finding_id = investigation_finding_id(finding.id, 0);
        if let Err(error) = self
            .investigation
            .set_finding_status(finding_id, FindingStatus::Dismissed)
        {
            self.status = format!("Cannot dismiss finding: {error}");
            return;
        }
        let correlation_id = investigation_correlation_id(finding.id);
        if self.investigation.correlation(correlation_id).is_some() {
            let _result = self
                .investigation
                .set_correlation_strength(correlation_id, CorrelationStrength::Rejected);
        }
        let hypothesis_id = investigation_hypothesis_id(finding.id);
        if self.investigation.hypothesis(hypothesis_id).is_some() {
            let _result = self
                .investigation
                .set_hypothesis_status(hypothesis_id, HypothesisStatus::Rejected);
        }
        self.discovery_preview_transform = false;
        self.invalidate_dossier();
        self.status = format!(
            "Dismissed {} without changing source bytes",
            discovery_title(finding)
        );
    }

    pub(super) fn open_discovery_in(&mut self, view: ViewKind) {
        self.active_view = view;
        self.drag_anchor = None;
        self.selected_digram = None;
        self.selected_resonance = None;
        self.resonance_key = None;
        self.invalidate_texture();
        self.status = format!(
            "Opened discovery range in {} with exact shared selection",
            view.title()
        );
    }
}
