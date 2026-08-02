//! Inspector, dossier, provenance, and linked-evidence presentation.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    #[allow(clippy::too_many_lines)]
    pub(super) fn show_inspector(&mut self, ui: &mut egui::Ui) {
        rail_title(ui, "INSPECTOR", "evidence + provenance");
        let session_source = self.session_bundle.as_ref().map(|bundle| {
            (
                bundle.manifest().source().alias().to_owned(),
                bundle.manifest().source().byte_length(),
                bundle.manifest().source().sha256().to_owned(),
                bundle.journal().entries().len(),
            )
        });
        let session_detached = session_source.is_some() && !self.session_attached;
        egui::ScrollArea::vertical()
            .id_salt(("inspector", self.active_view.title()))
            .horizontal_scroll_offset(0.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let content_width = ui.available_width();
                ui.set_width(content_width);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                if self.active_view == ViewKind::Discover {
                    self.show_discovery_inspector(ui);
                    ui.separator();
                }

                ui.label(
                    egui::RichText::new("SOURCE")
                        .strong()
                        .size(10.5)
                        .color(UI_TEAL),
                );
                if let Some((alias, source_length, digest, _)) = &session_source {
                    ui.label(alias);
                    ui.monospace(format!("{source_length} bytes"));
                    ui.colored_label(
                        if session_detached { UI_AMBER } else { UI_TEAL },
                        format!(
                            "{} · SHA-256 {}…",
                            if session_detached {
                                "SOURCE REQUIRED"
                            } else {
                                "ATTACHED"
                            },
                            digest_prefix(digest)
                        ),
                    )
                    .on_hover_text(format!("SHA-256 {digest}"));
                } else if let Some(source) = self
                    .loaded_source
                    .as_ref()
                    .filter(|_| self.active_view != ViewKind::Interleave)
                {
                    ui.label(self.source_name());
                    ui.monospace(format!("{} logical bytes", source.source_length));
                    ui.label(egui::RichText::new("Read-only local source").color(UI_MUTED));
                    if source.sampled_overview {
                        ui.colored_label(UI_TEAL, "SAMPLED OVERVIEW + EXACT FOCUS TILES");
                        ui.monospace(format!(
                            "L{} / {} tiles / {} resident bytes",
                            source.tile_overview_level,
                            source.resident_tiles.len(),
                            source.resident_bytes
                        ));
                        let coverage_start = source
                            .resident_tiles
                            .iter()
                            .map(|tile| tile.coverage.start)
                            .min();
                        let coverage_end = source
                            .resident_tiles
                            .iter()
                            .map(|tile| tile.coverage.end)
                            .max();
                        let coverage = coverage_start.zip(coverage_end);
                        if let Some((start, end)) = coverage {
                            ui.weak(format!(
                                "systematic logical coverage 0x{start:08x}..0x{end:08x}; each rendered datum retains its exact read range"
                            ));
                        }
                    }
                } else {
                    ui.label(self.source_name());
                    ui.monospace(format!("{} bytes", self.active_bytes().len()));
                    ui.label(egui::RichText::new("Read-only source").color(UI_MUTED));
                }

                if session_detached {
                    ui.add_space(4.0);
                    ui.add_sized(
                        [ui.available_width(), RAIL_CONTROL_HEIGHT],
                        egui::TextEdit::singleline(&mut self.path_input)
                            .hint_text("Choose matching source…"),
                    );
                    ui.columns(3, |columns| {
                        if rail_action(&mut columns[0], "Browse…") {
                            self.browse_session_source();
                        }
                        if rail_action_enabled(
                            &mut columns[1],
                            self.session_file_load.is_none(),
                            if self.session_file_load.is_some() {
                                "Loading…"
                            } else {
                                "Reattach"
                            },
                        ) {
                            self.reattach_session_path();
                        }
                        if rail_action(&mut columns[2], "Verify held") {
                            self.verify_held_session_source();
                        }
                    });
                    ui.small(
                        "The workspace and event trail are open. Byte previews and analyses remain unavailable until SHA-256 and length both match.",
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new("PRESERVED EXACT SELECTION")
                            .strong()
                            .size(10.5)
                            .color(UI_TEAL),
                    );
                    if self.restored_session_selection.is_empty() {
                        ui.weak("No active source-byte selection was saved.");
                    } else {
                        let selected_bytes: usize = self
                            .restored_session_selection
                            .iter()
                            .map(|range| range.end.saturating_sub(range.start))
                            .sum();
                        ui.monospace(format!(
                            "{} exact range(s) / {selected_bytes} bytes",
                            self.restored_session_selection.len()
                        ));
                        for range in self.restored_session_selection.iter().take(12) {
                            ui.monospace(format!(
                                "0x{:08x}..0x{:08x}",
                                range.start, range.end
                            ));
                        }
                    }
                    ui.separator();
                    ui.label(
                        egui::RichText::new("SESSION TRAIL")
                            .strong()
                            .size(10.5)
                            .color(UI_TEAL),
                    );
                    if let Some((_, _, _, event_count)) = session_source.as_ref() {
                        ui.monospace(format!("{event_count} integrity-checked event(s)"));
                    }
                    for entry in self.session_journal.entries().iter().rev().take(8) {
                        ui.label(format!(
                            "#{:03}  {}",
                            entry.sequence,
                            journal_event_label(&entry.event)
                        ));
                    }
                    return;
                }

                ui.label(egui::RichText::new("A / PRIMARY").strong().size(10.0));
                ui.add_sized(
                    [ui.available_width(), RAIL_CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("Path or Browse…"),
                );
                let can_restore_demo = self.loaded_source.is_some();
                ui.columns(3, |columns| {
                    if rail_action(&mut columns[0], "Browse A…") {
                        self.browse_primary_source();
                    }
                    if rail_action_enabled(
                        &mut columns[1],
                        self.primary_file_load.is_none(),
                        if self.primary_file_load.is_some() {
                            "Loading…"
                        } else {
                            "Load A"
                        },
                    ) {
                        self.load_path();
                    }
                    if can_restore_demo && rail_action(&mut columns[2], "Use demo") {
                        self.restore_demo_source();
                    } else if !can_restore_demo {
                        columns[2].add_enabled(false, egui::Button::new("Use demo"));
                    }
                });
                ui.small(
                    "Read-only · native Browse or drag/drop · large sources use a bounded 16 MiB tile overview.",
                );

                if self.comparison_context() || self.comparison_source.is_some() {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("B / COMPARISON").strong().size(10.0));
                    if let Some(source) = &self.comparison_source {
                        ui.label(&source.display_name);
                        ui.monospace(format!("{} bytes", source.source_length));
                    }
                    ui.add_sized(
                        [ui.available_width(), RAIL_CONTROL_HEIGHT],
                        egui::TextEdit::singleline(&mut self.comparison_path_input)
                            .hint_text("Second source path or Browse…"),
                    );
                    ui.columns(3, |columns| {
                        if rail_action(&mut columns[0], "Browse B…") {
                            self.browse_comparison_source();
                        }
                        if rail_action_enabled(
                            &mut columns[1],
                            self.comparison_file_load.is_none(),
                            if self.comparison_file_load.is_some() {
                                "Loading…"
                            } else {
                                "Load B"
                            },
                        ) {
                            self.load_comparison_path();
                        }
                        if rail_action(&mut columns[2], "Clear B") {
                            self.clear_comparison_source();
                        }
                    });
                    ui.small(&self.comparison_status);
                }
                if let Some((_, _, _, event_count)) = session_source.as_ref() {
                    ui.small(format!(
                        "Source-free session · {event_count} append-only event(s) · local path excluded from bundle"
                    ));
                }
                ui.separator();

                if self.active_view == ViewKind::Discover {
                    self.show_signature_pack_inspector(ui);
                    ui.separator();
                }

                if self.active_view != ViewKind::Interleave {
                    self.show_dossier_inspector(ui);
                    ui.separator();
                }

                let selection = self.inspector_selection();
                let selected_preview = self.exact_selection_preview(&selection);
                let bytes = self.active_bytes();
                ui.label(
                    egui::RichText::new("SELECTION")
                        .strong()
                        .size(10.5)
                        .color(UI_TEAL),
                );
                ui.monospace(format!(
                    "0x{:08x}..0x{:08x}  ({} bytes)",
                    selection.start,
                    selection.end,
                    selection.end.saturating_sub(selection.start)
                ));
                ui.colored_label(egui::Color32::from_rgb(74, 190, 168), "exact byte offsets");

                if let Some(selected) = selected_preview.as_deref() {
                    let preview_len = selected.len().min(64);
                    if let Some(preview) = selected.get(..preview_len) {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("HEX / ASCII").strong().size(10.5));
                        ui.monospace(hex_preview(preview));
                        ui.monospace(ascii_preview(preview));
                        let histogram = byte_histogram(selected);
                        let unique = histogram.bins.iter().filter(|&&count| count > 0).count();
                        ui.monospace(format!("unique byte values: {unique}"));
                        if selection.end.saturating_sub(selection.start) > selected.len() {
                            ui.weak(format!(
                                "preview capped at {} exact resident bytes",
                                selected.len()
                            ));
                        }
                    }
                } else if self.active_view == ViewKind::Projection3d {
                    ui.colored_label(
                        UI_AMBER,
                        "Exact bytes are not resident; click the voxel to refine this tile",
                    );
                }

                if let Some((first, second)) = self.selected_digram {
                    ui.separator();
                    ui.strong("Aggregate matrix cell");
                    ui.monospace(format!("0x{first:02x} -> 0x{second:02x}"));
                    let selected = self.selected_or_full(bytes);
                    if let Ok(counts) = digram_counts(selected, self.digram_stride) {
                        ui.monospace(format!("occurrences: {}", counts.count(first, second)));
                        ui.weak("aggregate; source occurrence index not materialized");
                    }
                }

                if self.active_view == ViewKind::Projection3d {
                    if let Some([first_offset, second_offset, third_offset]) =
                        self.selected_projection
                    {
                        ui.separator();
                        ui.strong("Exact 3D contributors");
                        ui.monospace(format!(
                            "0x{first_offset:08x}  0x{second_offset:08x}  0x{third_offset:08x}"
                        ));
                        ui.weak("three-byte window; click another point to remap");
                    }
                    if let Some(cohort) = &self.projection_cohort_selection {
                        ui.separator();
                        ui.label(
                            egui::RichText::new("3D COHORT")
                                .strong()
                                .size(10.5)
                                .color(UI_TEAL),
                        );
                        ui.monospace(format!(
                            "{} voxels / {} exact bytes",
                            cohort.metrics.member_count, cohort.metrics.unique_byte_count
                        ));
                        if let Some(span) = &cohort.metrics.source_span {
                            ui.monospace(format!(
                                "covering span 0x{:08x}..0x{:08x}",
                                span.start, span.end
                            ));
                        }
                        if let Some(concentration) = cohort.metrics.source_byte_concentration {
                            ui.label(format!(
                                "Dominant byte 0x{:02x}: {} of {} selected offsets",
                                concentration.byte,
                                concentration.occurrences,
                                concentration.observed_offsets
                            ));
                        }
                        ui.weak(
                            "Membership is the exact lasso result; the covering span is navigation context only.",
                        );
                        for range in cohort.source_ranges.iter().take(8) {
                            ui.monospace(format!("0x{:08x}..0x{:08x}", range.start, range.end));
                        }
                        if cohort.source_ranges.len() > 8 {
                            ui.weak(format!(
                                "+ {} more exact ranges",
                                cohort.source_ranges.len().saturating_sub(8)
                            ));
                        }
                        if cohort.truncated {
                            ui.colored_label(UI_AMBER, "Cohort membership reached the POC bound");
                        }
                        if let Some(analytical) = self.analytical_cohort.selection() {
                            ui.separator();
                            ui.label(&analytical.explanation);
                            for factor in &analytical.factors {
                                ui.monospace(format!(
                                    "{}  {:+}",
                                    factor.name, factor.contribution
                                ));
                            }
                            ui.weak(format!(
                                "{} stable sampled-byte identities",
                                analytical.member_ids.len()
                            ));
                        }
                    }
                }

                self.show_selected_resonance(ui);

                ui.separator();
                ui.label(
                    egui::RichText::new("PROVENANCE")
                        .strong()
                        .size(10.5)
                        .color(UI_TEAL),
                );
                ui.monospace(match self.active_view {
                    ViewKind::Discover => match self.workbench_mode {
                        WorkbenchMode::Leads => format!(
                            "workbench-discovery/v2  <= {} bytes  exact ranges  bounded heuristics",
                            WorkbenchConfig::default().max_inspected_bytes
                        ),
                        WorkbenchMode::Regions => {
                            "living-region-map/v1  exact regions  typed relationships".to_owned()
                        }
                        WorkbenchMode::Compare => {
                            "comparison-archaeology/v1  paired exact ranges  move inference disclosed"
                                .to_owned()
                        }
                    },
                    ViewKind::Projection3d => format!(
                        "projection-composition/v1  {} / {}  <= {} samples  stable IDs  exact ranges",
                        self.projection_composition.domain.label(),
                        self.projection_composition.projection_a.short_label(),
                        self.projection_point_budget
                    ),
                    ViewKind::Resonance => format!(
                        "selection-resonance/v1  {}  base={} stride>={} <= {} samples/scale",
                        resonance_metric_label(self.resonance_metric),
                        self.resonance_base_window,
                        self.resonance_stride,
                        self.resonance_sample_budget
                    ),
                    ViewKind::Structure => self.structure_artifact.as_ref().map_or_else(
                        || self.structure_status.clone(),
                        |artifact| {
                            format!(
                                "structure-entropy/v1  gen={}  width={}  exact  artifact={}…",
                                artifact.generation.0,
                                artifact.preset.atlas_width,
                                digest_prefix(&artifact.artifact_digest)
                            )
                        },
                    ),
                    ViewKind::Grammar => format!(
                        "digram/v1  stride={}  exact  range=selection",
                        self.digram_stride
                    ),
                    ViewKind::Interleave => format!(
                        "bit-plane/v1  stride={} lane={} bit={} exact",
                        self.interleave_stride, self.interleave_lane, self.bit_plane
                    ),
                    ViewKind::RevisionDiff => self.comparison_artifact.as_ref().map_or_else(
                        || {
                            format!(
                                "aligned-diff/v1  width={} same-offset exact",
                                self.diff_width
                            )
                        },
                        |artifact| {
                            format!(
                                "tiled-diff/v1  L{}  {} matched samples  {} exact-prefix atlas  artifact={}…",
                                artifact.overview_level,
                                if artifact.is_sampled() {
                                    "sampled"
                                } else {
                                    "exact"
                                },
                                self.comparison_target_bytes().len(),
                                digest_prefix(&artifact.artifact_digest)
                            )
                        },
                    ),
                });
                self.show_ground_truth(ui, selection.start);
                if self.active_view == ViewKind::Projection3d {
                    ui.collapsing("Program / export", |ui| {
                        self.show_video_export_controls(ui);
                    });
                }
            });
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn show_discovery_inspector(&self, ui: &mut egui::Ui) {
        match self.workbench_mode {
            WorkbenchMode::Regions => {
                ui.label(
                    egui::RichText::new("ACTIVE REGION")
                        .strong()
                        .size(10.5)
                        .color(UI_TEAL),
                );
                let Some(region) = self.selected_region.and_then(|id| self.regions.region(id))
                else {
                    ui.weak("No living region selected");
                    return;
                };
                ui.label(egui::RichText::new(&region.label).strong().size(14.0));
                ui.colored_label(
                    region_kind_color(&region.kind),
                    region_kind_label(&region.kind),
                );
                ui.monospace(discovery_range_summary(&region.provenance.ranges.ranges));
                let relationships = self
                    .regions
                    .relationships()
                    .iter()
                    .filter(|relationship| {
                        relationship.from == region.id || relationship.to == region.id
                    })
                    .count();
                ui.label(format!("{relationships} typed relationship(s)"));
                return;
            }
            WorkbenchMode::Compare => {
                ui.label(
                    egui::RichText::new("ACTIVE COMPARISON")
                        .strong()
                        .size(10.5)
                        .color(UI_TEAL),
                );
                let Some(region) = self.comparison.as_ref().and_then(|comparison| {
                    self.selected_comparison
                        .and_then(|id| comparison.region(id))
                }) else {
                    ui.weak("No comparison region selected");
                    return;
                };
                ui.label(egui::RichText::new(&region.explanation).strong().size(14.0));
                ui.colored_label(
                    comparison_class_color(region.classification),
                    comparison_class_label(region.classification),
                );
                let left = region
                    .left
                    .as_ref()
                    .and_then(|provenance| provenance.ranges.ranges.first());
                let right = region.right.ranges.ranges.first();
                ui.monospace(format!(
                    "{}  ->  {}",
                    left.map_or_else(|| "--".to_owned(), format_byte_range),
                    right.map_or_else(|| "--".to_owned(), format_byte_range)
                ));
                return;
            }
            WorkbenchMode::Leads => {}
        }

        ui.label(
            egui::RichText::new("ACTIVE LEAD")
                .strong()
                .size(10.5)
                .color(UI_TEAL),
        );
        let Some(finding) = self.selected_discovery() else {
            ui.weak("No ranked lead selected");
            return;
        };
        let status = self
            .investigation
            .finding(investigation_finding_id(finding.id, 0))
            .map_or(FindingStatus::Candidate, |record| record.status);
        ui.label(
            egui::RichText::new(discovery_title(finding))
                .strong()
                .size(14.0),
        );
        ui.monospace(discovery_range_summary(&finding.source_ranges));
        ui.label(discovery_evidence_summary(finding));
        if let WorkbenchEvidence::CatalogSignature(evidence) = &finding.evidence {
            show_signature_match_evidence(ui, evidence);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("status: {}", finding_status_label(status)));
            ui.separator();
            ui.label(format!(
                "notebook: {} records",
                self.investigation.evidence().len()
            ));
        });
        if let Some(hypothesis) = self
            .investigation
            .hypothesis(investigation_hypothesis_id(finding.id))
        {
            ui.separator();
            ui.label(
                egui::RichText::new("TRANSFORM HYPOTHESIS")
                    .strong()
                    .size(10.5)
                    .color(UI_TEAL),
            );
            ui.label(&hypothesis.statement);
            ui.colored_label(UI_AMBER, hypothesis_status_label(hypothesis.status));
        }
        if let Some(branch) = self
            .selected_branch
            .and_then(|branch_id| self.branches.branch(branch_id))
        {
            ui.separator();
            ui.label(
                egui::RichText::new("ACTIVE BRANCH")
                    .strong()
                    .size(10.5)
                    .color(UI_TEAL),
            );
            ui.label(&branch.label);
            ui.monospace(format!(
                "{} / {}",
                branch_status_label(branch.status),
                self.branch_assessments
                    .get(&branch.id)
                    .map_or("unscored", |assessment| {
                        transform_assessment_label(*assessment)
                    })
            ));
            ui.weak("reversible · loss none · source remains immutable");
            for before in &branch.before_metrics {
                let after = branch
                    .after_metrics
                    .iter()
                    .find(|metric| metric.name == before.name)
                    .map(|metric| metric.value);
                ui.monospace(format!(
                    "{}  {} -> {}",
                    before.name,
                    before.value,
                    after.map_or_else(|| "--".to_owned(), |value| value.to_string())
                ));
            }
        }
    }

    pub(super) fn show_selected_resonance(&self, ui: &mut egui::Ui) {
        if self.active_view != ViewKind::Resonance {
            return;
        }
        let Some(selected) = self.selected_resonance else {
            return;
        };
        ui.separator();
        ui.strong("Selected echo");
        ui.monospace(format!(
            "probe 0x{:08x}  ->  candidate 0x{:08x}",
            selected.probe_offset, selected.candidate_offset
        ));
        ui.monospace(format!(
            "{} bytes  /  {:.1}%  /  {}",
            selected.window_size,
            selected.score * 100.0,
            resonance_metric_label(selected.metric)
        ));
        ui.weak("candidate range is now the shared selection");
    }

    pub(super) fn show_ground_truth(&self, ui: &mut egui::Ui, offset: usize) {
        let Some(data) = &self.data else {
            return;
        };
        let Ok(offset) = u64::try_from(offset) else {
            return;
        };
        match self.active_view {
            ViewKind::Discover if self.workbench_mode == WorkbenchMode::Compare => {
                if let Some(region) = data
                    .revisions
                    .regions
                    .iter()
                    .find(|region| region.range.contains(offset))
                {
                    ui.monospace(format!(
                        "fixture truth: {} / {:?}",
                        region.name, region.kind
                    ));
                }
            }
            ViewKind::Discover
            | ViewKind::Projection3d
            | ViewKind::Resonance
            | ViewKind::Structure
            | ViewKind::Grammar
                if self.loaded_source.is_none() =>
            {
                if let Some(region) = data
                    .investigation
                    .regions
                    .iter()
                    .find(|region| region.range.contains(offset))
                {
                    ui.monospace(format!(
                        "fixture truth: {} / {:?}",
                        region.name, region.kind
                    ));
                }
            }
            ViewKind::Interleave => {
                let layout = data.sensor.layout;
                ui.monospace(format!(
                    "fixture truth: {}x{} / {} lanes / {}-byte records",
                    layout.width_samples,
                    layout.height_rows,
                    layout.lanes,
                    layout.record_stride_bytes
                ));
            }
            ViewKind::RevisionDiff => {
                if let Some(region) = data
                    .revisions
                    .regions
                    .iter()
                    .find(|region| region.range.contains(offset))
                {
                    ui.monospace(format!(
                        "fixture truth: {} / {:?}",
                        region.name, region.kind
                    ));
                }
            }
            ViewKind::Discover
            | ViewKind::Projection3d
            | ViewKind::Resonance
            | ViewKind::Structure
            | ViewKind::Grammar => {}
        }
    }

    pub(super) fn execute_dossier_action(&mut self, action: DossierActionKind) {
        match action {
            DossierActionKind::OpenStructure => self.open_discovery_in(ViewKind::Structure),
            DossierActionKind::OpenGrammar => self.open_discovery_in(ViewKind::Grammar),
            DossierActionKind::QueryResonance => self.open_discovery_in(ViewKind::Resonance),
            DossierActionKind::OpenProjection => self.open_discovery_in(ViewKind::Projection3d),
            DossierActionKind::CompareSelection => {
                self.active_view = ViewKind::RevisionDiff;
                self.invalidate_texture();
                "Opened the exact selection against comparison source B"
                    .clone_into(&mut self.status);
            }
            DossierActionKind::TestXorBranch => {
                self.test_manual_branch();
                self.invalidate_dossier();
            }
            DossierActionKind::PromoteEvidence => {
                let finding_id = self.dossier.as_ref().and_then(|dossier| {
                    dossier.links.iter().find_map(|link| {
                        if link.state == DossierLinkState::Candidate
                            && let DossierLinkTarget::Finding(id) = link.target
                        {
                            Some(id)
                        } else {
                            None
                        }
                    })
                });
                let finding = finding_id.and_then(|id| {
                    self.discovery_findings
                        .iter()
                        .find(|finding| investigation_finding_id(finding.id, 0) == id)
                        .cloned()
                });
                if let Some(finding) = finding {
                    self.promote_discovery_finding(&finding);
                    self.invalidate_dossier();
                } else {
                    "No unreviewed finding intersects the exact selection"
                        .clone_into(&mut self.status);
                }
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub(super) fn show_dossier_strip(&mut self, ui: &mut egui::Ui) {
        let dossier = self.dossier.clone();
        let error = self.dossier_error.clone();
        let mut requested_action = None;
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(18, 25, 30))
            .stroke(egui::Stroke::new(1.0, UI_BORDER))
            .inner_margin(egui::Margin::symmetric(11, 8))
            .show(ui, |ui| {
                ui.set_min_height(106.0);
                let Some(dossier) = dossier.as_ref() else {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("SELECTION DOSSIER")
                                .strong()
                                .size(10.5)
                                .color(UI_TEAL),
                        );
                        ui.label(error.as_deref().unwrap_or("Select exact bytes to begin"));
                    });
                    return;
                };
                let metrics = &dossier.metrics;
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("SELECTION DOSSIER")
                            .strong()
                            .size(10.5)
                            .color(UI_TEAL),
                    );
                    ui.monospace(discovery_range_summary(&dossier.provenance.ranges.ranges));
                    ui.separator();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&dossier.observed_profile)
                                .strong()
                                .size(12.0),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&dossier.observed_profile);
                });
                ui.add_space(3.0);
                ui.columns(5, |columns| {
                    dossier_metric(&mut columns[0], "BYTES", &metrics.byte_count.to_string());
                    dossier_metric(
                        &mut columns[1],
                        "ENTROPY",
                        &format!("{:.2} bits", metrics.shannon_entropy_bits),
                    );
                    dossier_metric(
                        &mut columns[2],
                        "DISTINCT",
                        &format!("{} / 256", metrics.distinct_values),
                    );
                    let text_count = metrics
                        .printable_ascii_count
                        .saturating_add(metrics.whitespace_count);
                    dossier_metric(
                        &mut columns[3],
                        "TEXT-LIKE",
                        &format!(
                            "{:.0}%",
                            text_count as f64 * 100.0 / metrics.byte_count as f64
                        ),
                    );
                    dossier_metric(
                        &mut columns[4],
                        "LINKS",
                        &if dossier.links_truncated {
                            format!("{}+", dossier.links.len())
                        } else {
                            dossier.links.len().to_string()
                        },
                    );
                });
                ui.add_space(4.0);
                ui.columns(dossier.actions.len(), |columns| {
                    for (column, action) in columns.iter_mut().zip(&dossier.actions) {
                        let response = column.add_enabled(
                            action.enabled,
                            egui::Button::new(&action.label)
                                .frame(true)
                                .truncate()
                                .corner_radius(egui::CornerRadius::same(4)),
                        );
                        let response = response.on_hover_text(&action.rationale);
                        if response.clicked() {
                            requested_action = Some(action.kind);
                        }
                    }
                });
            });
        ui.add_space(7.0);
        if let Some(action) = requested_action {
            self.execute_dossier_action(action);
        }
    }

    pub(super) fn show_dossier_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(dossier) = self.dossier.clone() else {
            return;
        };
        ui.separator();
        ui.label(
            egui::RichText::new("DOSSIER / LINKED CONTEXT")
                .strong()
                .size(10.5)
                .color(UI_TEAL),
        );
        if let Some(structure) = &dossier.structure {
            ui.monospace(format!(
                "entropy {:.2} mean · {:.2}..{:.2}",
                structure.mean_entropy_bits,
                structure.minimum_entropy_bits,
                structure.maximum_entropy_bits
            ));
            ui.weak(format!(
                "{} block(s) · {} of {} selected bytes covered{}",
                structure.overlapping_blocks,
                structure.covered_bytes,
                dossier.metrics.byte_count,
                if structure.complete { " exactly" } else { "" }
            ));
        } else {
            ui.weak("Structure context is pending or unavailable for this source snapshot.");
        }
        ui.add_space(4.0);
        let mut requested_link = None;
        for link in dossier.links.iter().take(10) {
            let state = dossier_link_state_label(link.state);
            let response = ui.add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(format!("{state}  {}", link.title))
                    .frame(true)
                    .truncate()
                    .corner_radius(egui::CornerRadius::same(4)),
            );
            let response = response.on_hover_text(format!(
                "{}\n{}",
                link.detail,
                discovery_range_summary(&link.provenance.ranges.ranges)
            ));
            if response.clicked() {
                requested_link = Some(link.target);
            }
        }
        if dossier.links.is_empty() {
            ui.weak("No finding, region, hypothesis, branch, or comparison intersects this range.");
        } else if dossier.links.len() > 10 || dossier.links_truncated {
            ui.weak(format!(
                "+ {} additional linked record(s)",
                dossier.links.len().saturating_sub(10)
            ));
        }
        if let Some(target) = requested_link {
            self.follow_dossier_link(target);
        }
    }

    pub(super) fn show_comparison_empty_state(&mut self, ui: &mut egui::Ui) {
        let (source_name, source_length) = self.loaded_source.as_ref().map_or_else(
            || {
                (
                    "Source A".to_owned(),
                    u64::try_from(self.source_bytes().len()).unwrap_or(u64::MAX),
                )
            },
            |source| (source.display_name.clone(), source.source_length),
        );
        ui.add_space((ui.available_height() * 0.12).max(18.0));
        egui::Frame::new()
            .fill(UI_RAIL_ALT)
            .stroke(egui::Stroke::new(1.0, UI_BORDER))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("PAIR A SOURCE WITH B")
                            .strong()
                            .size(13.0),
                    );
                    ui.label(
                        egui::RichText::new(
                            "same-offset classes stay exact; inferred moves remain disclosed",
                        )
                        .size(10.5)
                        .color(UI_MUTED),
                    );
                });
                ui.separator();
                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    comparison_source_card(
                        &mut columns[0],
                        "A / PRIMARY",
                        &source_name,
                        &format!("{source_length} bytes · read-only"),
                        true,
                    );
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(20, 29, 34))
                        .stroke(egui::Stroke::new(1.0, UI_CYAN))
                        .corner_radius(egui::CornerRadius::same(5))
                        .inner_margin(egui::Margin::same(11))
                        .show(&mut columns[1], |ui| {
                            ui.label(
                                egui::RichText::new("B / COMPARISON")
                                    .monospace()
                                    .strong()
                                    .size(10.0)
                                    .color(UI_CYAN),
                            );
                            ui.label(
                                egui::RichText::new("Drop a second file or choose one")
                                    .strong()
                                    .size(12.0),
                            );
                            ui.weak(
                                "Source B is independently read-only; large pairs use matched bounded tiles with sampled coverage disclosed.",
                            );
                            ui.add_space(8.0);
                            if rail_action_enabled(
                                ui,
                                self.comparison_file_load.is_none(),
                                if self.comparison_file_load.is_some() {
                                    "Loading B…"
                                } else {
                                    "Browse source B…"
                                },
                            ) {
                                self.browse_comparison_source();
                            }
                        });
                });
                ui.add_space(8.0);
                ui.monospace("A  ── exact aligned offsets ──>  B");
                ui.weak(
                    "After B loads, the dossier gains exact unchanged/modified/new links and the diff atlas becomes pickable.",
                );
            });
    }

    pub(super) fn follow_dossier_link(&mut self, target: DossierLinkTarget) {
        match target {
            DossierLinkTarget::Finding(id) => {
                let finding = self
                    .discovery_findings
                    .iter()
                    .find(|finding| investigation_finding_id(finding.id, 0) == id)
                    .map(|finding| finding.id);
                if let Some(finding) = finding {
                    self.workbench_mode = WorkbenchMode::Leads;
                    self.active_view = ViewKind::Discover;
                    self.select_discovery_finding(finding);
                }
            }
            DossierLinkTarget::Evidence(id) => {
                let provenance = self
                    .investigation
                    .evidence()
                    .iter()
                    .find(|evidence| evidence.id == id)
                    .map(|evidence| evidence.provenance.clone());
                if let Some(provenance) = provenance {
                    self.active_view = ViewKind::Discover;
                    self.select_first_exact_range(&provenance);
                }
            }
            DossierLinkTarget::Correlation(id) => {
                let provenance = self
                    .investigation
                    .correlation(id)
                    .map(|record| record.provenance.clone());
                if let Some(provenance) = provenance {
                    self.active_view = ViewKind::Discover;
                    self.select_first_exact_range(&provenance);
                }
            }
            DossierLinkTarget::Hypothesis(id) => {
                let provenance = self
                    .investigation
                    .hypothesis(id)
                    .map(|record| record.provenance.clone());
                if let Some(provenance) = provenance {
                    self.active_view = ViewKind::Discover;
                    self.select_first_exact_range(&provenance);
                }
            }
            DossierLinkTarget::Region(id) => {
                self.workbench_mode = WorkbenchMode::Regions;
                self.active_view = ViewKind::Discover;
                self.select_region(id);
            }
            DossierLinkTarget::RegionRelationship(id) => {
                let region = self
                    .regions
                    .relationships()
                    .iter()
                    .find(|relationship| relationship.id == id)
                    .map(|relationship| relationship.from);
                if let Some(region) = region {
                    self.workbench_mode = WorkbenchMode::Regions;
                    self.active_view = ViewKind::Discover;
                    self.select_region(region);
                }
            }
            DossierLinkTarget::Branch(id) => {
                let provenance = self
                    .branches
                    .branch(id)
                    .map(|branch| branch.provenance.clone());
                if let Some(provenance) = provenance {
                    self.selected_branch = Some(id);
                    self.active_view = ViewKind::Discover;
                    self.select_first_exact_range(&provenance);
                }
            }
            DossierLinkTarget::Comparison(id) => {
                self.workbench_mode = WorkbenchMode::Compare;
                self.active_view = ViewKind::Discover;
                self.select_comparison(id);
            }
        }
        self.invalidate_dossier();
    }
}
