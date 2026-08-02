//! Global, discovery, region, comparison, branch, and resonance controls.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    pub(super) fn show_header_view_tabs(&mut self, ui: &mut egui::Ui, compact_header: bool) {
        let previous = self.active_view;
        for (view, full_label, compact_label) in [
            (ViewKind::Discover, "Discover", "Discover"),
            (ViewKind::Projection3d, "3D Lab", "3D"),
            (ViewKind::Resonance, "Resonance", "Resonance"),
            (ViewKind::Structure, "Structure", "Structure"),
            (ViewKind::Grammar, "Grammar", "Grammar"),
            (ViewKind::Interleave, "Interleave", "Interleave"),
            (ViewKind::RevisionDiff, "Revision diff", "Diff"),
        ] {
            let label = if compact_header {
                compact_label
            } else {
                full_label
            };
            let selected = self.active_view == view;
            let text_color = if selected {
                egui::Color32::WHITE
            } else {
                UI_HEADER_TEXT
            };
            let button = egui::Button::new(egui::RichText::new(label).size(12.0).color(text_color))
                .fill(if selected {
                    egui::Color32::from_rgb(15, 100, 151)
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(if selected {
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(9, 77, 118))
                } else {
                    egui::Stroke::NONE
                })
                .selected(selected)
                .corner_radius(egui::CornerRadius::same(5));
            let tab_width = if compact_header { 61.0 } else { 72.0 };
            if ui.add_sized([tab_width, 29.0], button).clicked() {
                self.active_view = view;
            }
        }
        if previous != self.active_view {
            self.drag_anchor = None;
            self.selected_digram = None;
            self.invalidate_texture();
        }
    }

    pub(super) fn show_header(&mut self, ui: &mut egui::Ui) {
        ui.visuals_mut().override_text_color = Some(UI_HEADER_TEXT);
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
        let compact_header = ui.available_width() < 1_180.0;
        ui.horizontal_centered(|ui| {
            ui.label(
                egui::RichText::new("STRATA")
                    .strong()
                    .size(18.0)
                    .color(UI_HEADER_TEXT),
            );
            if !compact_header {
                ui.label(
                    egui::RichText::new("linked binary investigation workbench")
                        .size(11.5)
                        .color(egui::Color32::from_rgb(107, 115, 120)),
                );
            }
            ui.add_space(if compact_header { 4.0 } else { 12.0 });
            if ui
                .add_sized(
                    [if compact_header { 54.0 } else { 66.0 }, 29.0],
                    egui::Button::new(
                        egui::RichText::new(if compact_header { "Open" } else { "Open…" })
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(UI_HEADER_TEXT)
                    .stroke(egui::Stroke::new(1.0, UI_HEADER_TEXT))
                    .corner_radius(egui::CornerRadius::same(4)),
                )
                .on_hover_text("Open read-only source A (⌘O)")
                .clicked()
            {
                self.browse_primary_source();
            }
            ui.allocate_ui_with_layout(
                egui::vec2(if compact_header { 72.0 } else { 82.0 }, 29.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| self.show_session_menu(ui),
            );
            if ui
                .add_sized(
                    [if compact_header { 46.0 } else { 54.0 }, 29.0],
                    egui::Button::new(
                        egui::RichText::new(if compact_header { "Prefs" } else { "Project" })
                            .color(UI_HEADER_TEXT),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(174, 181, 185),
                    ))
                    .corner_radius(egui::CornerRadius::same(4)),
                )
                .on_hover_text("Local project and launch preferences")
                .clicked()
            {
                self.show_project_preferences = true;
            }
            ui.add_space(if compact_header { 2.0 } else { 8.0 });

            self.show_header_view_tabs(ui, compact_header);

            if !compact_header {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("WGPU")
                            .monospace()
                            .size(10.5)
                            .color(egui::Color32::from_rgb(92, 100, 105)),
                    );
                    ui.label(
                        egui::RichText::new("EXACT PICKS")
                            .strong()
                            .size(10.5)
                            .color(egui::Color32::from_rgb(38, 151, 135)),
                    );
                });
            }
        });
    }

    pub(super) fn show_control_deck(&mut self, ui: &mut egui::Ui) {
        rail_title(ui, "CONTROL DECK", self.active_view.title());
        let detached = self.session_bundle.is_some() && !self.session_attached;
        if detached {
            ui.colored_label(
                UI_AMBER,
                "SOURCE REQUIRED · controls resume after digest verification",
            );
            ui.add_space(4.0);
        }
        egui::ScrollArea::vertical()
            .id_salt(("control-deck", self.active_view.title()))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_enabled_ui(!detached, |ui| match self.active_view {
                    ViewKind::Discover => self.show_discovery_controls(ui),
                    ViewKind::Projection3d => self.show_projection_control_deck(ui),
                    ViewKind::Resonance => {
                        control_section(ui, 1, "MATCH / CORRELATION", |ui| {
                            self.show_resonance_controls(ui);
                        });
                    }
                    ViewKind::Structure => {
                        control_section(ui, 1, "STRUCTURE ATLAS", |ui| {
                            self.show_controls(ui);
                            ui.weak("Byte class and local entropy remain exact per source offset.");
                            if self.texture_tiles.len() > 1 {
                                ui.weak(format!(
                                    "{} bounded GPU tiles preserve the selected row width.",
                                    self.texture_tiles.len()
                                ));
                            }
                        });
                    }
                    ViewKind::Grammar => {
                        control_section(ui, 1, "TRANSITION GRAMMAR", |ui| {
                            self.show_controls(ui);
                            ui.weak("Ordered byte pairs recalculate from the shared selection.");
                        });
                    }
                    ViewKind::Interleave => {
                        control_section(ui, 1, "RECORD / INTERLEAVE", |ui| {
                            self.show_controls(ui);
                            ui.weak("Stride and lane controls preserve exact byte provenance.");
                        });
                    }
                    ViewKind::RevisionDiff => {
                        control_section(ui, 1, "REVISION DIFF", |ui| {
                            self.show_controls(ui);
                            ui.weak("Same-offset classes distinguish stable and changed bytes.");
                        });
                    }
                });
            });
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn show_projection_control_deck(&mut self, ui: &mut egui::Ui) {
        let previous_projection = self.projection_composition.projection_a;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Quick preset")
                    .size(11.0)
                    .color(UI_MUTED),
            );
            egui::ComboBox::from_id_salt("projection-quick-preset")
                .selected_text(self.projection_composition.projection_a.short_label())
                .width(190.0)
                .show_ui(ui, |ui| {
                    for projection in ProjectionKind::BASIC {
                        ui.selectable_value(
                            &mut self.projection_composition.projection_a,
                            projection,
                            projection.label(),
                        );
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("P1 ANALYTICAL").weak().size(9.5));
                    for projection in ProjectionKind::P1 {
                        ui.selectable_value(
                            &mut self.projection_composition.projection_a,
                            projection,
                            projection.label(),
                        );
                    }
                });
        });
        if previous_projection != self.projection_composition.projection_a {
            self.apply_projection_defaults(self.projection_composition.projection_a);
        }
        ui.add_space(8.0);
        let mut clear_cohort = false;
        let mut open_first_cohort_run = false;
        control_section(ui, 1, "INTERACTION / COHORT", |ui| {
            let changed = rail_segmented(
                ui,
                &mut self.projection_interaction,
                &[
                    (ProjectionInteraction::Rotate, "Rotate"),
                    (ProjectionInteraction::SelectCohort, "Select cohort"),
                ],
            );
            if changed {
                self.projection_cohort_anchor = None;
                self.projection_cohort_cursor = None;
            }
            if let Some(cohort) = &self.projection_cohort_selection {
                ui.monospace(format!(
                    "{} voxels / {} exact bytes / {} ranges",
                    cohort.metrics.member_count,
                    cohort.metrics.unique_byte_count,
                    cohort.source_ranges.len()
                ));
                ui.weak("Membership remains stable while the projection rotates or compares A/B.");
                ui.columns(2, |columns| {
                    let open_label = if cohort.source_ranges.len() > 1 {
                        "Open first run"
                    } else {
                        "Open Structure"
                    };
                    if rail_action(&mut columns[0], open_label) {
                        open_first_cohort_run = true;
                    }
                    if rail_action(&mut columns[1], "Clear cohort") {
                        clear_cohort = true;
                    }
                });
            } else {
                ui.weak("Drag a box, or click two corners, around related voxels.");
            }
        });
        if clear_cohort {
            self.projection_cohort_selection = None;
            self.analytical_cohort.clear_selection();
            self.projection_cohort_anchor = None;
            self.projection_cohort_cursor = None;
            "Cleared the 3D cohort; sampled byte identities remain unchanged"
                .clone_into(&mut self.status);
        }
        if open_first_cohort_run
            && let Some(first_run) = self
                .projection_cohort_selection
                .as_ref()
                .and_then(|cohort| cohort.source_ranges.first())
                .cloned()
        {
            let run_count = self
                .projection_cohort_selection
                .as_ref()
                .map_or(0, |cohort| cohort.source_ranges.len());
            self.selection = first_run;
            self.active_view = ViewKind::Structure;
            self.invalidate_texture();
            self.status = if run_count == 1 {
                "Opened the cohort's exact contiguous run in Structure".to_owned()
            } else {
                format!(
                    "Opened the first of {run_count} exact cohort runs; the full discontiguous cohort remains in 3D"
                )
            };
        }
        control_section(ui, 2, "PROJECTION / GEOMETRY", |ui| {
            self.show_projection_composition_controls(ui);
        });
        control_section(ui, 3, "MAPPING / LENS MIXER", |ui| {
            self.show_projection_lens_controls(ui);
        });
        control_section(ui, 4, "EMPHASIS / RENDER", |ui| {
            self.show_projection_field_controls(ui);
        });
        control_section(ui, 5, "SAMPLING / APPEARANCE", |ui| {
            self.show_projection_render_controls(ui);
        });
        control_section(ui, 6, "MOTION / CAMERA", |ui| {
            rail_group_label(ui, "MOTION");
            ui.columns(2, |columns| {
                columns[0].checkbox(&mut self.projection_spin, "Spin");
                columns[1].add_enabled_ui(
                    self.projection_composition.compare_mode == ProjectionCompareMode::Morph,
                    |ui| {
                        ui.checkbox(&mut self.projection_auto_morph, "Morph")
                            .on_hover_text("Animate the A/B morph mix");
                    },
                );
            });
            let speed = format!("{:.2}", self.projection_speed);
            rail_slider_row(
                ui,
                "Speed",
                speed,
                egui::Slider::new(&mut self.projection_speed, 0.05..=1.2),
            );
            if rail_action(ui, "Reset camera") {
                self.projection_yaw = -0.72;
                self.projection_pitch = 0.38;
                self.projection_zoom = 0.92;
            }
        });
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn show_discovery_controls(&mut self, ui: &mut egui::Ui) {
        let previous_mode = self.workbench_mode;
        let mut mode_changed = false;
        control_section(ui, 1, "INVESTIGATION WORKSPACE", |ui| {
            mode_changed = rail_segmented(
                ui,
                &mut self.workbench_mode,
                &[
                    (WorkbenchMode::Leads, "Leads"),
                    (WorkbenchMode::Regions, "Regions"),
                    (WorkbenchMode::Compare, "Compare"),
                ],
            );
            ui.weak(match self.workbench_mode {
                WorkbenchMode::Leads => "Ranked signals, exact evidence, and reversible tests.",
                WorkbenchMode::Regions => "Named byte regions and typed structural relationships.",
                WorkbenchMode::Compare => "Semantic revision changes across two exact sources.",
            });
        });
        if mode_changed && self.workbench_mode != previous_mode {
            self.synchronize_workbench_selection();
        }
        match self.workbench_mode {
            WorkbenchMode::Regions => {
                self.show_region_controls(ui);
                return;
            }
            WorkbenchMode::Compare => {
                self.show_comparison_controls(ui);
                return;
            }
            WorkbenchMode::Leads => {}
        }

        let scanned = self
            .source_bytes()
            .len()
            .min(WorkbenchConfig::default().max_inspected_bytes);
        control_section(ui, 2, "DISCOVERY / SCOPE", |ui| {
            ui.label(egui::RichText::new("Heterogeneous bounded analysis").strong());
            ui.monospace(format!(
                "{} leads / 0x00000000..0x{scanned:08x}",
                self.discovery_findings.len()
            ));
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(UI_TEAL, "BOUNDED");
                ui.label("stable IDs");
                ui.label("exact ranges");
            });
        });

        let mut clicked = None;
        let mut previous = false;
        let mut next = false;
        control_section(ui, 3, "RANKED LEADS", |ui| {
            ui.horizontal(|ui| {
                previous = ui.button("Previous").clicked();
                next = ui.button("Next").clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} total", self.discovery_findings.len()))
                            .monospace()
                            .color(UI_MUTED),
                    );
                });
            });
            ui.add_space(4.0);
            for (index, finding) in self.discovery_findings.iter().enumerate() {
                let selected = self.discovery_selected == Some(finding.id);
                let status = self
                    .investigation
                    .finding(investigation_finding_id(finding.id, 0))
                    .map_or(FindingStatus::Candidate, |record| record.status);
                let label = format!(
                    "{:02}  {}  {:>3.0}%",
                    index.saturating_add(1),
                    discovery_title(finding),
                    finding.confidence * 100.0
                );
                if rail_selectable(ui, selected, label).clicked() {
                    clicked = Some(finding.id);
                }
                ui.indent(("lead-meta", finding.id.0), |ui| {
                    ui.label(
                        egui::RichText::new(finding_status_label(status))
                            .monospace()
                            .size(10.5)
                            .color(if selected { UI_AMBER } else { UI_MUTED }),
                    );
                });
            }
        });
        if previous {
            self.cycle_discovery(-1);
        }
        if next {
            self.cycle_discovery(1);
        }
        if let Some(finding_id) = clicked {
            self.select_discovery_finding(finding_id);
        }

        let selected = self.selected_discovery().cloned();
        let mut selected_range = None;
        let mut open_view = None;
        control_section(ui, 4, "EXACT LINKS / NAVIGATION", |ui| {
            let Some(finding) = selected.as_ref() else {
                ui.weak("Select a lead to expose its linked ranges.");
                return;
            };
            for (index, range) in finding.source_ranges.iter().enumerate() {
                let active = self.selection.start
                    == usize::try_from(range.start).unwrap_or(usize::MAX)
                    && self.selection.end == usize::try_from(range.end).unwrap_or(usize::MAX);
                let label = format!(
                    "Range {}  0x{:08x}..0x{:08x}",
                    index.saturating_add(1),
                    range.start,
                    range.end
                );
                if rail_selectable(ui, active, label).clicked() {
                    selected_range = Some(index);
                }
            }
            ui.add_space(4.0);
            ui.columns(3, |columns| {
                if rail_action(&mut columns[0], "Structure") {
                    open_view = Some(ViewKind::Structure);
                }
                if rail_action(&mut columns[1], "Resonance") {
                    open_view = Some(ViewKind::Resonance);
                }
                if rail_action(&mut columns[2], "3D map") {
                    open_view = Some(ViewKind::Projection3d);
                }
            });
        });
        if let Some(index) = selected_range {
            self.select_discovery_range(index);
        }
        if let Some(view) = open_view {
            self.open_discovery_in(view);
        }

        let mut test = false;
        let mut promote = false;
        let mut reject = false;
        control_section(ui, 5, "TRANSFORM / EVIDENCE", |ui| {
            let Some(finding) = selected.as_ref() else {
                ui.weak("No transform hypothesis selected.");
                return;
            };
            if let Some(transform) = discovery_transform(finding) {
                let hypothesis = self
                    .investigation
                    .hypothesis(investigation_hypothesis_id(finding.id));
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(transform_label(transform)).strong());
                    ui.colored_label(
                        UI_AMBER,
                        hypothesis.map_or("draft", |record| hypothesis_status_label(record.status)),
                    );
                });
                ui.weak("Reversible derived preview; source stays immutable.");
                ui.columns(3, |columns| {
                    test = rail_action(&mut columns[0], "Test");
                    promote = rail_action(&mut columns[1], "Promote");
                    reject = rail_action(&mut columns[2], "Reject");
                });
            } else {
                ui.label("Exact correlation lead");
                ui.columns(2, |columns| {
                    promote = rail_action(&mut columns[0], "Promote evidence");
                    reject = rail_action(&mut columns[1], "Dismiss");
                });
            }
        });
        if let Some(finding) = selected.as_ref() {
            if test {
                self.test_discovery_transform(finding);
            }
            if promote {
                self.promote_discovery_finding(finding);
            }
            if reject {
                self.reject_discovery_finding(finding);
            }
        }

        self.show_branch_controls(ui, 6);

        control_section(ui, 7, "EVIDENCE NOTEBOOK", |ui| {
            ui.monospace(format!(
                "{} promoted record(s)",
                self.investigation.evidence().len()
            ));
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(UI_TEAL, "READ-ONLY SOURCE");
                ui.label("claims retain exact provenance");
            });
        });
    }

    pub(super) fn show_region_controls(&mut self, ui: &mut egui::Ui) {
        control_section(ui, 2, "LIVING REGIONS", |ui| {
            ui.monospace(format!(
                "{} regions / {} typed links",
                self.regions.regions().len(),
                self.regions.relationships().len()
            ));
            ui.weak("Regions are analytical objects; every item retains exact source ranges.");
        });

        let mut clicked = None;
        control_section(ui, 3, "REGION TREE", |ui| {
            for region in self.regions.regions() {
                let selected = self.selected_region == Some(region.id);
                let range = region.provenance.ranges.ranges.first();
                let label = range.map_or_else(
                    || region.label.clone(),
                    |range| format!("{}  0x{:x}..0x{:x}", region.label, range.start, range.end),
                );
                if rail_selectable(ui, selected, label).clicked() {
                    clicked = Some(region.id);
                }
                if region.parent_id.is_some() {
                    ui.indent(("region-child", region.id.0), |ui| {
                        ui.label(
                            egui::RichText::new("nested exact region")
                                .size(10.0)
                                .color(UI_MUTED),
                        );
                    });
                }
            }
        });
        if let Some(id) = clicked {
            self.select_region(id);
        }

        control_section(ui, 4, "RELATIONSHIPS", |ui| {
            let Some(selected) = self.selected_region else {
                ui.weak("Select a region to inspect its links.");
                return;
            };
            let mut count = 0_usize;
            for relationship in
                self.regions.relationships().iter().filter(|relationship| {
                    relationship.from == selected || relationship.to == selected
                })
            {
                count = count.saturating_add(1);
                ui.label(format!(
                    "{}  {} -> {}",
                    region_relationship_label(relationship.kind),
                    relationship.from.0,
                    relationship.to.0
                ));
                ui.weak(&relationship.rationale);
            }
            if count == 0 {
                ui.weak("No peer relationship is recorded for this region.");
            }
        });

        let mut open = None;
        control_section(ui, 5, "OPEN SELECTION IN", |ui| {
            ui.columns(2, |columns| {
                if rail_action(&mut columns[0], "Structure") {
                    open = Some(ViewKind::Structure);
                }
                if rail_action(&mut columns[1], "3D cohort") {
                    open = Some(ViewKind::Projection3d);
                }
            });
        });
        if let Some(view) = open {
            self.active_view = view;
            if view == ViewKind::Projection3d {
                self.projection_interaction = ProjectionInteraction::SelectCohort;
            }
            self.invalidate_texture();
            self.status = format!("Opened living region in {}", view.title());
        }
    }

    pub(super) fn show_comparison_controls(&mut self, ui: &mut egui::Ui) {
        let Some(comparison) = self.comparison.clone() else {
            control_section(ui, 2, "COMPARISON", |ui| {
                ui.weak("Comparison fixture is unavailable.");
            });
            return;
        };
        let mut counts = [0_usize; 4];
        for region in comparison.regions() {
            counts[comparison_class_index(region.classification)] =
                counts[comparison_class_index(region.classification)].saturating_add(1);
        }
        control_section(ui, 2, "ARCHAEOLOGY SUMMARY", |ui| {
            ui.monospace(format!(
                "{} unchanged / {} moved / {} modified / {} new",
                counts[0], counts[1], counts[2], counts[3]
            ));
            ui.weak(
                "Classes use exact fixture truth; moved is a cross-offset structural relation.",
            );
        });

        let mut clicked = None;
        control_section(ui, 3, "CLASSIFIED REGIONS", |ui| {
            for region in comparison.regions() {
                let selected = self.selected_comparison == Some(region.id);
                if rail_selectable(
                    ui,
                    selected,
                    format!(
                        "{}  {}",
                        comparison_class_label(region.classification),
                        region.explanation
                    ),
                )
                .clicked()
                {
                    clicked = Some(region.id);
                }
                let left = region
                    .left
                    .as_ref()
                    .and_then(|provenance| provenance.ranges.ranges.first())
                    .map_or_else(|| "left --".to_owned(), format_byte_range);
                let right = region
                    .right
                    .ranges
                    .ranges
                    .first()
                    .map_or_else(|| "right --".to_owned(), format_byte_range);
                ui.indent(("comparison-ranges", region.id.0), |ui| {
                    ui.monospace(format!("{left}  ->  {right}"));
                });
            }
        });
        if let Some(id) = clicked {
            self.select_comparison(id);
        }

        control_section(ui, 4, "VERIFY", |ui| {
            if rail_action(ui, "Open exact byte diff") {
                self.active_view = ViewKind::RevisionDiff;
                self.invalidate_texture();
                "Opened semantic comparison region in the exact aligned byte diff"
                    .clone_into(&mut self.status);
            }
        });
    }

    pub(super) fn select_region(&mut self, id: RegionId) {
        let provenance = self
            .regions
            .region(id)
            .map(|region| region.provenance.clone());
        let Some(provenance) = provenance else {
            return;
        };
        self.selected_region = Some(id);
        self.select_first_exact_range(&provenance);
        self.status = format!("Selected living region {} with exact provenance", id.0);
    }

    pub(super) fn select_comparison(&mut self, id: ComparisonRegionId) {
        let provenance = self
            .comparison
            .as_ref()
            .and_then(|comparison| comparison.region(id))
            .map(|region| region.right.clone());
        let Some(provenance) = provenance else {
            return;
        };
        self.selected_comparison = Some(id);
        self.select_first_exact_range(&provenance);
        self.status = format!("Selected exact comparison region {}", id.0);
    }

    pub(super) fn synchronize_workbench_selection(&mut self) {
        match self.workbench_mode {
            WorkbenchMode::Leads => {
                if self.selected_discovery().is_some() {
                    self.select_discovery_range(0);
                } else {
                    self.selection = 0..0;
                }
            }
            WorkbenchMode::Regions => {
                if let Some(id) = self.selected_region {
                    self.select_region(id);
                } else {
                    self.selection = 0..0;
                }
            }
            WorkbenchMode::Compare => {
                if let Some(id) = self.selected_comparison {
                    self.select_comparison(id);
                } else {
                    self.selection = 0..0;
                }
            }
        }
    }

    pub(super) fn select_first_exact_range(&mut self, provenance: &ExactProvenance) {
        let Some(range) = provenance.ranges.ranges.first() else {
            return;
        };
        let (Ok(start), Ok(end)) = (usize::try_from(range.start), usize::try_from(range.end))
        else {
            return;
        };
        self.selection = start..end;
        self.selected_digram = None;
        self.selected_projection = None;
        self.selected_resonance = None;
        self.resonance_key = None;
        self.invalidate_texture();
    }

    pub(super) fn show_branch_controls(&mut self, ui: &mut egui::Ui, number: usize) {
        let mut test_manual = false;
        let mut select = None;
        let mut pin = false;
        let mut discard = false;
        control_section(ui, number, "HYPOTHESIS BRANCHES", |ui| {
            let key = format!("0x{:02x}", self.branch_key);
            rail_slider_row(
                ui,
                "XOR key",
                key,
                egui::Slider::new(&mut self.branch_key, 1..=u8::MAX),
            );
            test_manual = rail_action(ui, "Test selected range as branch");
            ui.add_space(4.0);
            for branch in self.branches.branches() {
                if rail_selectable(
                    ui,
                    self.selected_branch == Some(branch.id),
                    format!("{}  [{}]", branch.label, branch_status_label(branch.status)),
                )
                .clicked()
                {
                    select = Some(branch.id);
                }
            }
            if self.branches.branches().is_empty() {
                ui.weak("No branch yet. Test the suggested transform or choose an XOR key.");
            }
            if self.selected_branch.is_some() {
                ui.columns(2, |columns| {
                    pin = rail_action(&mut columns[0], "Pin");
                    discard = rail_action(&mut columns[1], "Discard");
                });
            }
            let pinned: Vec<_> = self
                .branches
                .branches()
                .iter()
                .filter(|branch| branch.status == BranchStatus::Pinned)
                .map(|branch| branch.id)
                .collect();
            if let [first, second, ..] = pinned.as_slice()
                && let Ok(comparison) = self.branches.compare(*first, *second)
            {
                ui.separator();
                ui.label(
                    egui::RichText::new("PINNED EVIDENCE DELTA")
                        .strong()
                        .size(10.0),
                );
                for metric in comparison.after_metrics.iter().take(4) {
                    ui.monospace(format!(
                        "{}  {} -> {}",
                        metric.name,
                        metric
                            .first
                            .map_or_else(|| "--".to_owned(), |value| value.to_string()),
                        metric
                            .second
                            .map_or_else(|| "--".to_owned(), |value| value.to_string())
                    ));
                }
            }
        });
        if test_manual {
            self.test_manual_branch();
        }
        if let Some(id) = select {
            self.selected_branch = Some(id);
        }
        if let Some(id) = self.selected_branch {
            if pin {
                match self.branches.pin(id) {
                    Ok(()) => {
                        self.status = format!("Pinned hypothesis branch {}", id.0);
                        self.invalidate_dossier();
                    }
                    Err(error) => self.status = format!("Cannot pin branch: {error}"),
                }
            }
            if discard {
                match self.branches.discard(id) {
                    Ok(()) => {
                        self.status = format!("Discarded hypothesis branch {}", id.0);
                        self.invalidate_dossier();
                    }
                    Err(error) => self.status = format!("Cannot discard branch: {error}"),
                }
            }
        }
    }

    pub(super) fn show_controls(&mut self, ui: &mut egui::Ui) {
        let changed = match self.active_view {
            ViewKind::Discover => false,
            ViewKind::Projection3d => {
                self.show_projection_controls(ui);
                false
            }
            ViewKind::Resonance => {
                self.show_resonance_controls(ui);
                false
            }
            ViewKind::Structure => {
                let value = self.atlas_width.to_string();
                rail_slider_row(
                    ui,
                    "Bytes / row",
                    value,
                    egui::Slider::new(&mut self.atlas_width, 8..=512),
                )
            }
            ViewKind::Grammar => {
                let value = self.digram_stride.to_string();
                rail_slider_row(
                    ui,
                    "Pair stride",
                    value,
                    egui::Slider::new(&mut self.digram_stride, 1..=64),
                )
            }
            ViewKind::Interleave => {
                let width = self.interleave_width.to_string();
                let width_changed = rail_slider_row(
                    ui,
                    "Samples / row",
                    width,
                    egui::Slider::new(&mut self.interleave_width, 4..=96),
                );
                let stride = self.interleave_stride.to_string();
                let stride_changed = rail_slider_row(
                    ui,
                    "Record stride",
                    stride,
                    egui::Slider::new(&mut self.interleave_stride, 1..=16),
                );
                if self.interleave_lane >= self.interleave_stride {
                    self.interleave_lane = self.interleave_stride.saturating_sub(1);
                }
                let lane = self.interleave_lane.to_string();
                let lane_changed = rail_slider_row(
                    ui,
                    "Lane",
                    lane,
                    egui::Slider::new(
                        &mut self.interleave_lane,
                        0..=self.interleave_stride.saturating_sub(1),
                    ),
                );
                let bit = self.bit_plane.to_string();
                let bit_changed = rail_slider_row(
                    ui,
                    "Bit plane",
                    bit,
                    egui::Slider::new(&mut self.bit_plane, 0..=7),
                );
                width_changed || stride_changed || lane_changed || bit_changed
            }
            ViewKind::RevisionDiff => {
                let value = self.diff_width.to_string();
                rail_slider_row(
                    ui,
                    "Bytes / row",
                    value,
                    egui::Slider::new(&mut self.diff_width, 8..=128),
                )
            }
        };
        if changed {
            if self.active_view == ViewKind::Structure {
                self.request_structure_analysis();
            } else {
                self.invalidate_texture();
            }
        }
    }

    pub(super) fn show_resonance_controls(&mut self, ui: &mut egui::Ui) {
        rail_group_label(ui, "MATCH EVIDENCE");
        let mut changed = rail_segmented(
            ui,
            &mut self.resonance_metric,
            &[
                (ResonanceMetric::ExactBytes, "Exact bytes"),
                (ResonanceMetric::ByteShape, "Byte shape"),
                (ResonanceMetric::Texture, "Texture"),
            ],
        );
        let base_window = self.resonance_base_window.to_string();
        changed |= rail_slider_row(
            ui,
            "Base window",
            base_window,
            egui::Slider::new(&mut self.resonance_base_window, 4..=64),
        );
        let stride = self.resonance_stride.to_string();
        changed |= rail_slider_row(
            ui,
            "Min stride",
            stride,
            egui::Slider::new(&mut self.resonance_stride, 1..=64),
        );
        let budget = self.resonance_sample_budget.to_string();
        changed |= rail_slider_row(
            ui,
            "Samples",
            budget,
            egui::Slider::new(&mut self.resonance_sample_budget, 256..=4_096),
        );
        if changed {
            self.resonance_key = None;
            self.selected_resonance = None;
        }
    }
}
