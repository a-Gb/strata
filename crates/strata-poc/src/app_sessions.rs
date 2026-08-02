//! Source-free session capture, restore, reattachment, and menu flow.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    pub(super) fn capture_session_workspace(&self) -> Result<PocWorkspaceSnapshot, String> {
        let source_length = self.logical_source_length();
        let cohort = self
            .projection_cohort_selection
            .as_ref()
            .map(stored_cohort_from_selection)
            .transpose()?;
        let exact_selection = if let Some(cohort) = &cohort {
            cohort.exact_ranges.clone()
        } else {
            let selection = self.clamped_selection(self.raw_source_bytes().len());
            if selection.is_empty() {
                self.restored_session_selection
                    .iter()
                    .map(stored_range_from_usize)
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![stored_range_from_usize(&selection)?]
            }
        };

        let finding_dispositions = self
            .discovery_findings
            .iter()
            .filter_map(|lead| {
                let status = self
                    .investigation
                    .finding(investigation_finding_id(lead.id, 0))?
                    .status;
                let status = match status {
                    FindingStatus::Candidate => return None,
                    FindingStatus::Promoted => StoredFindingStatus::Promoted,
                    FindingStatus::Dismissed => StoredFindingStatus::Dismissed,
                };
                Some(StoredFindingDisposition {
                    lead_id: lead.id.0,
                    status,
                })
            })
            .collect();
        let branches = self
            .branches
            .branches()
            .iter()
            .map(stored_xor_branch)
            .collect::<Result<Vec<_>, _>>()?;

        let snapshot = PocWorkspaceSnapshot {
            version: POC_WORKSPACE_VERSION,
            source_generation: self.source_generation,
            active_view: stored_view(self.active_view),
            workbench_mode: stored_workbench_mode(self.workbench_mode),
            exact_selection,
            selected_lead: self.discovery_selected.map(|id| id.0),
            selected_region: self.selected_region.map(|id| id.0),
            selected_comparison: if self.comparison_source.is_some() {
                None
            } else {
                self.selected_comparison.map(|id| id.0)
            },
            finding_dispositions,
            branches,
            selected_branch: self.selected_branch.map(|id| split_u128(id.0)),
            branch_key: self.branch_key,
            cohort,
            atlas_width: self.atlas_width,
            digram_stride: self.digram_stride,
            interleave_width: self.interleave_width,
            interleave_stride: self.interleave_stride,
            interleave_lane: self.interleave_lane,
            bit_plane: self.bit_plane,
            diff_width: self.diff_width,
            resonance_metric: stored_resonance_metric(self.resonance_metric),
            resonance_base_window: self.resonance_base_window,
            resonance_stride: self.resonance_stride,
            resonance_sample_budget: self.resonance_sample_budget,
            projection: StoredProjectionState {
                composition: Some(self.projection_composition),
                stride: self.projection_composition.parameters.lag,
                point_budget: self.projection_point_budget,
                morph: legacy_morph_for_projection(self.projection_composition.projection_a),
                color_mix: legacy_color_mix(self.projection_composition.channels.color),
                relief: self.projection_relief,
                context_light: self.projection_context_light,
                point_size: self.projection_point_size,
                brightness: self.projection_brightness,
                perspective: self.projection_perspective,
                render_style: stored_render_style(self.projection_composition.geometry),
                field_radius: self.projection_field_radius,
                field_exposure: self.projection_field_exposure,
                contour_mode: stored_contour_mode(self.projection_contour_mode),
                yaw: self.projection_yaw,
                pitch: self.projection_pitch,
                zoom: self.projection_zoom,
                spin: self.projection_spin,
                auto_morph: self.projection_auto_morph,
                speed: self.projection_speed,
            },
        };
        snapshot.validate(source_length)?;
        Ok(snapshot)
    }

    pub(super) fn save_session(&mut self) {
        if !self.prepare_session_save() {
            return;
        }
        let path = PathBuf::from(self.session_path_input.trim());
        if path.as_os_str().is_empty() {
            "Enter a session bundle directory first".clone_into(&mut self.status);
            return;
        }
        match self.persist_session_bundle(&path) {
            Ok(()) => {
                self.status = format!(
                    "Saved source-free session to {} · source bytes and path excluded",
                    path.display()
                );
            }
            Err(error) => self.status = error,
        }
    }

    pub(super) fn prepare_session_save(&mut self) -> bool {
        if !self.session_attached {
            "Cannot save while the session source is detached".clone_into(&mut self.status);
            return false;
        }
        let Some(source_handle) = self.analysis_source.clone() else {
            "Cannot save without an attached immutable source".clone_into(&mut self.status);
            return false;
        };
        if source_handle.descriptor().content_digest.is_none() {
            if self.source_digest_request.is_none() {
                self.queue_source_digest(source_handle, SourceDigestPurpose::ActiveSource);
            }
            "Session save is waiting for the progressive whole-source SHA-256"
                .clone_into(&mut self.status);
            return false;
        }
        true
    }

    pub(super) fn persist_session_bundle(&mut self, path: &Path) -> Result<(), String> {
        let snapshot = self.capture_session_workspace()?;
        let value = serde_json::to_value(&snapshot)
            .map_err(|error| format!("cannot encode workspace: {error}"))?;
        let workspace = WorkspaceSnapshot::from_value(value);
        let mut journal = self.session_journal.clone();
        let last_workspace = journal.entries().iter().rev().find_map(|entry| {
            if let JournalEvent::WorkspaceChanged(snapshot) = &entry.event {
                Some(snapshot)
            } else {
                None
            }
        });
        if !last_workspace.is_some_and(|saved| poc_workspace_equivalent(saved, &workspace)) {
            journal
                .append(JournalEvent::WorkspaceChanged(workspace.clone()))
                .map_err(|error| format!("cannot append session event: {error}"))?;
        }
        let descriptor = self
            .analysis_source
            .as_ref()
            .map(AttachedSource::descriptor)
            .ok_or_else(|| "attached source disappeared during save".to_owned())?;
        let source = SourceFingerprint::new(
            "redacted-primary-source",
            descriptor
                .length
                .ok_or_else(|| "attached source length is unknown".to_owned())?,
            descriptor
                .content_digest
                .ok_or_else(|| "attached source digest is not sealed".to_owned())?,
        )
        .map_err(|error| format!("cannot fingerprint source: {error}"))?;
        let bundle = SessionBundle::new(source, workspace, journal)
            .map_err(|error| format!("cannot construct bundle: {error}"))?;
        bundle
            .save_to_directory(path)
            .map_err(|error| format!("cannot save session: {error}"))?;
        self.session_journal = bundle.journal().clone();
        self.session_bundle = Some(bundle);
        Ok(())
    }

    pub(super) fn open_session(&mut self) {
        let path = PathBuf::from(self.session_path_input.trim());
        if path.as_os_str().is_empty() {
            "Enter a session bundle directory first".clone_into(&mut self.status);
            return;
        }
        if let Err(error) = self.open_session_path(&path) {
            self.status = error;
        }
    }

    pub(super) fn open_session_path(&mut self, path: &Path) -> Result<(), String> {
        let bundle = SessionBundle::load_from_directory(path)
            .map_err(|error| format!("cannot open session: {error}"))?;
        validate_bundle_workspace_event(&bundle)?;
        let snapshot: PocWorkspaceSnapshot =
            serde_json::from_value(bundle.manifest().workspace().value().clone())
                .map_err(|error| format!("invalid POC workspace: {error}"))?;
        snapshot.validate(bundle.manifest().source().byte_length())?;
        let event_count = bundle.journal().entries().len();
        let source_length = bundle.manifest().source().byte_length();
        self.session_journal = bundle.journal().clone();
        self.session_bundle = Some(bundle);
        self.enter_detached_session(&snapshot);
        self.session_path_input = path.display().to_string();
        self.status = format!(
            "Reopened source-free session · {event_count} events · source required ({source_length} bytes)"
        );
        Ok(())
    }

    pub(super) fn enter_detached_session(&mut self, snapshot: &PocWorkspaceSnapshot) {
        if let Some(request_id) = self.structure_request.take()
            && let Some(runtime) = &self.analysis_runtime
        {
            let _ = runtime.cancel(request_id);
        }
        if let Some(pending) = self.source_digest_request.take()
            && let Some(runtime) = &self.analysis_runtime
        {
            let _ = runtime.cancel_digest(pending.request_id);
        }
        self.primary_file_load = None;
        self.comparison_file_load = None;
        self.session_file_load = None;
        self.focus_file_load = None;
        self.pending_project_save = None;
        self.session_attached = false;
        self.path_input.clear();
        self.comparison_source = None;
        self.comparison_artifact = None;
        self.pending_session_source = None;
        self.comparison_path_input.clear();
        "Choose comparison source B".clone_into(&mut self.comparison_status);
        self.analysis_runtime = match InvestigationRuntime::new(ProductionRuntimeConfig::default())
        {
            Ok(runtime) => Some(runtime),
            Err(error) => {
                self.initialization_error = Some(format!(
                    "Analysis runtime could not reset for the reopened session: {error}"
                ));
                None
            }
        };
        self.apply_session_controls(snapshot);
        self.restored_session_selection = snapshot
            .exact_selection
            .iter()
            .filter_map(range_from_stored)
            .collect();
        self.selection = 0..0;
        self.drag_anchor = None;
        self.selected_digram = None;
        self.selected_projection = None;
        self.selected_resonance = None;
        self.discovery_findings.clear();
        self.discovery_selected = None;
        self.discovery_generation = None;
        self.discovery_preview_transform = false;
        self.discovery_error = None;
        self.investigation = InvestigationModel::new();
        self.regions = RegionModel::new();
        self.selected_region = None;
        self.comparison = None;
        self.selected_comparison = None;
        self.branches = BranchModel::new();
        self.selected_branch = None;
        self.branch_assessments.clear();
        self.projection_cohort_anchor = None;
        self.projection_cohort_cursor = None;
        self.projection_cohort_selection = None;
        self.analytical_cohort = CohortModel::new(SourceSnapshot {
            source_id: SourceId(1),
            generation: SourceGeneration(snapshot.source_generation),
        });
        self.entropy.clear();
        self.structure_artifact = None;
        "Structure unavailable until the source is reattached"
            .clone_into(&mut self.structure_status);
        self.projection_samples.clear();
        self.projection_sample_key = None;
        self.projection_field_texture = None;
        self.projection_field_key = None;
        self.resonance_layers.clear();
        self.resonance_key = None;
        self.texture_tiles.clear();
        self.texture_key = None;
        self.active_mapping = None;
        self.render_error = None;
    }

    pub(super) fn verify_held_session_source(&mut self) {
        let Some(source) = self.analysis_source.clone() else {
            "No held immutable source is available".clone_into(&mut self.status);
            return;
        };
        let descriptor = source.descriptor();
        if let (Some(byte_length), Some(sha256)) =
            (descriptor.length, descriptor.content_digest.clone())
        {
            self.finish_source_digest(
                SourceDigestPurpose::SessionCandidate,
                &SourceDigestArtifact {
                    source_id: descriptor.id,
                    generation: descriptor.generation,
                    byte_length,
                    sha256,
                },
            );
        } else {
            self.queue_source_digest(source, SourceDigestPurpose::SessionCandidate);
        }
    }

    pub(super) fn reattach_session_path(&mut self) {
        let input = self.path_input.trim();
        if input.is_empty() {
            "Choose a candidate source path first".clone_into(&mut self.status);
            return;
        }
        let path = PathBuf::from(input);
        let generation = match self.session_workspace() {
            Ok(snapshot) => SourceGeneration(snapshot.source_generation),
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let _ = self.queue_file_load(FileLoadSlot::SessionReattachment, path, generation);
    }

    pub(super) fn verify_candidate_digest(
        &self,
        artifact: &SourceDigestArtifact,
    ) -> Result<(), String> {
        let bundle = self
            .session_bundle
            .as_ref()
            .ok_or_else(|| "No saved session is open".to_owned())?;
        match bundle.reattach_digest(artifact.byte_length, artifact.sha256.clone()) {
            Reattachment::Match => Ok(()),
            Reattachment::Mismatch {
                expected_byte_length,
                actual_byte_length,
                expected_sha256,
                actual_sha256,
            } => Err(format!(
                "Not attached: source differs · expected {} bytes {}… · got {} bytes {}…",
                expected_byte_length,
                digest_prefix(&expected_sha256),
                actual_byte_length,
                digest_prefix(&actual_sha256)
            )),
        }
    }

    pub(super) fn activate_attached_session(&mut self) {
        let snapshot = match self.session_workspace() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if let Err(error) = snapshot.validate(self.logical_source_length()) {
            self.status = format!("Cannot restore workspace: {error}");
            return;
        }
        if let Err(error) =
            self.align_analysis_source_generation(SourceGeneration(snapshot.source_generation))
        {
            self.status = error;
            return;
        }
        self.session_attached = true;
        if let Err(error) = self.restore_attached_workspace(&snapshot) {
            self.enter_detached_session(&snapshot);
            self.status = format!("Source matched, but workspace restore failed: {error}");
            return;
        }
        self.path_input.clear();
        self.status = match self.session_changed_keys() {
            Ok(changed) if changed.is_empty() => format!(
                "Reattached · SHA-256 matched · {} exact range(s) restored",
                snapshot.exact_selection.len()
            ),
            Ok(changed) => format!("Reattached with restore drift in: {}", changed.join(", ")),
            Err(error) => format!("Reattached, but cannot compare restored state: {error}"),
        };
    }

    pub(super) fn session_workspace(&self) -> Result<PocWorkspaceSnapshot, String> {
        let bundle = self
            .session_bundle
            .as_ref()
            .ok_or_else(|| "No saved session is open".to_owned())?;
        serde_json::from_value(bundle.manifest().workspace().value().clone())
            .map_err(|error| format!("invalid POC workspace: {error}"))
    }

    pub(super) fn restore_attached_workspace(
        &mut self,
        snapshot: &PocWorkspaceSnapshot,
    ) -> Result<(), String> {
        self.source_generation = snapshot.source_generation;
        self.apply_session_controls(snapshot);
        self.request_structure_analysis();
        self.recompute_discovery();
        self.rebuild_workspace_models();

        for disposition in &snapshot.finding_dispositions {
            let finding = self
                .discovery_findings
                .iter()
                .find(|finding| finding.id.0 == disposition.lead_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "saved finding {} is absent after analysis",
                        disposition.lead_id
                    )
                })?;
            match disposition.status {
                StoredFindingStatus::Promoted => self.promote_discovery_finding(&finding),
                StoredFindingStatus::Dismissed => self.reject_discovery_finding(&finding),
            }
        }
        self.discovery_selected = match snapshot.selected_lead {
            Some(id) => Some(
                self.discovery_findings
                    .iter()
                    .find(|finding| finding.id.0 == id)
                    .map(|finding| finding.id)
                    .ok_or_else(|| format!("saved lead {id} is absent after analysis"))?,
            ),
            None => None,
        };
        self.selected_region = match snapshot.selected_region {
            Some(id) => Some(
                self.regions
                    .region(RegionId(id))
                    .map(|region| region.id)
                    .ok_or_else(|| format!("saved region {id} is absent after analysis"))?,
            ),
            None => None,
        };
        self.selected_comparison = match snapshot.selected_comparison {
            Some(id) => Some(
                self.comparison
                    .as_ref()
                    .and_then(|comparison| comparison.region(ComparisonRegionId(id)))
                    .map(|region| region.id)
                    .ok_or_else(|| format!("saved comparison region {id} is absent"))?,
            ),
            None => None,
        };

        self.branches = BranchModel::new();
        self.branch_assessments.clear();
        for stored in &snapshot.branches {
            let range = byte_range_from_stored(stored.range)?;
            let transform = ReversibleTransform::XorByte(stored.key);
            let evaluation = evaluate_transform_candidate(self.source_bytes(), range, transform)
                .map_err(|error| format!("cannot restore XOR branch: {error}"))?;
            let branch_id = join_u128(stored.id);
            let lead_id = WorkbenchLeadId(stored.id[0] ^ stored.id[1]);
            let provenance = discovery_provenance(&[range], self.source_generation);
            let mut branch = build_branch_from_evaluation(
                lead_id,
                stored.label.clone(),
                provenance,
                &evaluation,
            )?;
            branch.id = BranchId(branch_id);
            branch.status = branch_status_from_stored(stored.status);
            let id = branch.id;
            self.branches
                .add_branch(branch)
                .map_err(|error| format!("cannot restore branch: {error}"))?;
            self.branch_assessments.insert(id, evaluation.assessment);
        }
        self.selected_branch = match snapshot.selected_branch {
            Some(id) => {
                let id = BranchId(join_u128(id));
                self.branches
                    .branch(id)
                    .ok_or_else(|| "saved selected branch is absent".to_owned())?;
                Some(id)
            }
            None => None,
        };

        self.restore_session_cohort(snapshot.cohort.as_ref())?;
        self.restore_exact_selection(&snapshot.exact_selection)?;
        self.invalidate_texture();
        Ok(())
    }

    pub(super) fn restore_session_cohort(
        &mut self,
        cohort: Option<&StoredCohort>,
    ) -> Result<(), String> {
        self.projection_cohort_anchor = None;
        self.projection_cohort_cursor = None;
        self.projection_cohort_selection = None;
        self.analytical_cohort = CohortModel::new(SourceSnapshot {
            source_id: SourceId(1),
            generation: SourceGeneration(self.source_generation),
        });
        let Some(cohort) = cohort else {
            return Ok(());
        };
        let members = cohort
            .members
            .iter()
            .enumerate()
            .map(|(index, offsets)| {
                let source_offsets = [
                    usize::try_from(offsets[0])
                        .map_err(|_| "cohort offset cannot fit this platform".to_owned())?,
                    usize::try_from(offsets[1])
                        .map_err(|_| "cohort offset cannot fit this platform".to_owned())?,
                    usize::try_from(offsets[2])
                        .map_err(|_| "cohort offset cannot fit this platform".to_owned())?,
                ];
                let source_range = if let Some(range) = cohort.member_ranges.get(index) {
                    let range = range_from_stored(range)
                        .ok_or_else(|| "cohort member range cannot fit this platform".to_owned())?;
                    [range.start, range.end]
                } else {
                    let start = *source_offsets
                        .iter()
                        .min()
                        .ok_or_else(|| "cohort member has no source contributor".to_owned())?;
                    let maximum = *source_offsets
                        .iter()
                        .max()
                        .ok_or_else(|| "cohort member has no source contributor".to_owned())?;
                    [
                        start,
                        maximum
                            .checked_add(1)
                            .ok_or_else(|| "cohort range overflow".to_owned())?,
                    ]
                };
                Ok(ProjectedMember {
                    screen_x: 0.0,
                    screen_y: 0.0,
                    source_offsets,
                    source_range,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let rectangle = SelectionRect::from_endpoints(-1.0, -1.0, 1.0, 1.0)
            .map_err(|error| format!("cannot reconstruct cohort bounds: {error:?}"))?;
        let selection = select_cohort(rectangle, &members, Some(self.source_bytes()))
            .map_err(|error| format!("cannot reconstruct cohort: {error:?}"))?;
        let expected = cohort
            .exact_ranges
            .iter()
            .map(|range| {
                range_from_stored(range)
                    .ok_or_else(|| "cohort range cannot fit this platform".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if selection.source_ranges != expected {
            return Err("reconstructed cohort lost exact range identity".to_owned());
        }
        self.analytical_cohort = materialize_analytical_cohort(
            &selection,
            self.source_bytes(),
            self.source_generation,
            self.projection_composition.projection_a,
        )?;
        self.projection_cohort_selection = Some(selection);
        Ok(())
    }

    pub(super) fn restore_exact_selection(&mut self, ranges: &[StoredRange]) -> Result<(), String> {
        let exact = ranges
            .iter()
            .map(|range| {
                range_from_stored(range)
                    .ok_or_else(|| "selection range cannot fit this platform".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.restored_session_selection.clone_from(&exact);
        self.selection = if exact.len() == 1 {
            exact.first().cloned().unwrap_or(0..0)
        } else {
            0..0
        };
        if ranges.is_empty() {
            return Ok(());
        }
        let provenance_ranges = ranges
            .iter()
            .copied()
            .map(byte_range_from_stored)
            .collect::<Result<Vec<_>, _>>()?;
        self.investigation
            .select_ranges(ExactProvenance {
                source_id: SourceId(1),
                generation: SourceGeneration(self.source_generation),
                ranges: ByteRangeSet {
                    ranges: provenance_ranges,
                },
            })
            .map_err(|error| format!("cannot restore exact selection: {error}"))
    }

    pub(super) fn apply_session_controls(&mut self, snapshot: &PocWorkspaceSnapshot) {
        self.active_view = view_from_stored(snapshot.active_view);
        self.workbench_mode = workbench_mode_from_stored(snapshot.workbench_mode);
        self.branch_key = snapshot.branch_key;
        self.atlas_width = snapshot.atlas_width;
        self.digram_stride = snapshot.digram_stride;
        self.interleave_width = snapshot.interleave_width;
        self.interleave_stride = snapshot.interleave_stride;
        self.interleave_lane = snapshot.interleave_lane;
        self.bit_plane = snapshot.bit_plane;
        self.diff_width = snapshot.diff_width;
        self.resonance_metric = resonance_metric_from_stored(snapshot.resonance_metric);
        self.resonance_base_window = snapshot.resonance_base_window;
        self.resonance_stride = snapshot.resonance_stride;
        self.resonance_sample_budget = snapshot.resonance_sample_budget;
        self.projection_point_budget = snapshot.projection.point_budget;
        self.projection_composition = snapshot.projection.composition.unwrap_or_else(|| {
            legacy_projection_composition(
                snapshot.projection.morph,
                snapshot.projection.render_style,
                snapshot.projection.stride,
                snapshot.projection.color_mix,
            )
        });
        self.projection_relief = snapshot.projection.relief;
        self.projection_context_light = snapshot.projection.context_light;
        self.projection_point_size = snapshot.projection.point_size;
        self.projection_brightness = snapshot.projection.brightness;
        self.projection_perspective = snapshot.projection.perspective;
        self.projection_field_radius = snapshot.projection.field_radius;
        self.projection_field_exposure = snapshot.projection.field_exposure;
        self.projection_contour_mode = contour_mode_from_stored(snapshot.projection.contour_mode);
        self.projection_yaw = snapshot.projection.yaw;
        self.projection_pitch = snapshot.projection.pitch;
        self.projection_zoom = snapshot.projection.zoom;
        self.projection_spin = snapshot.projection.spin;
        self.projection_auto_morph = snapshot.projection.auto_morph;
        self.projection_speed = snapshot.projection.speed;
        self.projection_phase = projection_phase_for_mix(self.projection_composition.mix);
        self.projection_interaction = ProjectionInteraction::Rotate;
    }

    pub(super) fn session_changed_keys(&self) -> Result<Vec<String>, String> {
        if !self.session_attached {
            return Ok(Vec::new());
        }
        let Some(bundle) = &self.session_bundle else {
            return Ok(Vec::new());
        };
        let current = serde_json::to_value(self.capture_session_workspace()?)
            .map_err(|error| error.to_string())?;
        let saved: PocWorkspaceSnapshot =
            serde_json::from_value(bundle.manifest().workspace().value().clone())
                .map_err(|error| format!("invalid saved POC workspace: {error}"))?;
        let saved = serde_json::to_value(saved).map_err(|error| error.to_string())?;
        let (Some(current), Some(saved)) = (current.as_object(), saved.as_object()) else {
            return Err("workspace checkpoint is not an object".to_owned());
        };
        let keys = current
            .keys()
            .chain(saved.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .flat_map(|key| {
                let current_value = current.get(key);
                let saved_value = saved.get(key);
                if current_value == saved_value {
                    return Vec::new();
                }
                match (
                    current_value.and_then(serde_json::Value::as_object),
                    saved_value.and_then(serde_json::Value::as_object),
                ) {
                    (Some(current_object), Some(saved_object)) => current_object
                        .keys()
                        .chain(saved_object.keys())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .filter(|nested| current_object.get(*nested) != saved_object.get(*nested))
                        .map(|nested| format!("{key}.{nested}"))
                        .collect(),
                    _ => vec![key.clone()],
                }
            })
            .collect();
        Ok(keys)
    }

    pub(super) fn show_session_menu(&mut self, ui: &mut egui::Ui) {
        let changed_keys = self.session_changed_keys().unwrap_or_default();
        let dirty = !changed_keys.is_empty();
        let attached = self.session_attached;
        let events = self.session_journal.entries().len();
        ui.menu_button(
            egui::RichText::new(if dirty { "SESSION •" } else { "SESSION" })
                .strong()
                .size(10.5)
                .color(if attached { UI_TEAL } else { UI_AMBER }),
            |ui| {
                ui.set_min_width(264.0);
                ui.label(
                    egui::RichText::new(if attached {
                        "ATTACHED / SOURCE EXCLUDED FROM SAVE"
                    } else {
                        "SOURCE REQUIRED / WORKSPACE READ-ONLY"
                    })
                    .monospace()
                    .size(10.0)
                    .color(if attached { UI_TEAL } else { UI_AMBER }),
                );
                ui.add_sized(
                    [264.0, RAIL_CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut self.session_path_input)
                        .hint_text("bundle directory"),
                );
                if ui
                    .add_sized(
                        [264.0, RAIL_CONTROL_HEIGHT],
                        egui::Button::new("Save snapshot  ⌘S"),
                    )
                    .clicked()
                {
                    self.save_session();
                    ui.close();
                }
                if ui
                    .add_sized(
                        [264.0, RAIL_CONTROL_HEIGHT],
                        egui::Button::new("Reopen bundle  ⌘O"),
                    )
                    .clicked()
                {
                    self.open_session();
                    ui.close();
                }
                if !attached
                    && ui
                        .add_sized(
                            [264.0, RAIL_CONTROL_HEIGHT],
                            egui::Button::new("Verify held source"),
                        )
                        .clicked()
                {
                    self.verify_held_session_source();
                    ui.close();
                }
                ui.separator();
                ui.monospace(format!("{events} append-only event(s)"));
                if dirty {
                    ui.colored_label(UI_AMBER, format!("Changed: {}", changed_keys.join(", ")));
                }
                for entry in self.session_journal.entries().iter().rev().take(4) {
                    ui.label(format!(
                        "#{:03}  {}",
                        entry.sequence,
                        journal_event_label(&entry.event)
                    ));
                }
            },
        )
        .response
        .on_hover_text("Save or reopen a source-free investigation session");
    }
}
