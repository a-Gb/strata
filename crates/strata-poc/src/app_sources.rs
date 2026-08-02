//! Discovery-model rebuilds plus asynchronous source and comparison loading.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    pub(super) fn recompute_discovery(&mut self) {
        let result = analyze_workbench(self.source_bytes(), WorkbenchConfig::default());
        let signature_result = self.external_signature_leads();
        match result {
            Ok(report) => {
                let mut findings = report.leads;
                match signature_result {
                    Ok(Some((signature_leads, status))) => {
                        findings
                            .retain(|finding| finding.kind != WorkbenchLeadKind::EmbeddedSignature);
                        findings.extend(signature_leads);
                        self.signature_scan_status = status;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.signature_scan_status = format!("Catalog scan failed: {error}");
                    }
                }
                findings.sort_by(|first, second| {
                    discovery_priority(first.kind)
                        .cmp(&discovery_priority(second.kind))
                        .then_with(|| second.confidence.total_cmp(&first.confidence))
                        .then_with(|| first.id.cmp(&second.id))
                });
                match build_investigation_model(&findings, self.source_generation) {
                    Ok(model) => {
                        self.investigation = model;
                        self.discovery_selected = findings.first().map(|finding| finding.id);
                        self.discovery_findings = findings;
                        self.discovery_error = None;
                        self.discovery_generation = Some(self.source_generation);
                        self.discovery_preview_transform = false;
                        self.select_discovery_range(0);
                    }
                    Err(error) => {
                        self.investigation = InvestigationModel::new();
                        self.discovery_findings.clear();
                        self.discovery_selected = None;
                        self.discovery_error = Some(format!(
                            "Cannot construct exact investigation state: {error}"
                        ));
                    }
                }
            }
            Err(error) => {
                self.investigation = InvestigationModel::new();
                self.discovery_findings.clear();
                self.discovery_selected = None;
                self.discovery_error = Some(format!("Discovery pass failed: {error}"));
            }
        }
    }

    pub(super) fn rebuild_workspace_models(&mut self) {
        let matches_investigation_fixture = self
            .data
            .as_ref()
            .is_some_and(|data| data.investigation.bytes.as_slice() == self.source_bytes());
        let region_result = if matches_investigation_fixture {
            self.data.as_ref().map_or_else(
                || Err("POC fixture data is unavailable".to_owned()),
                |data| {
                    build_region_model(
                        &data.investigation,
                        SourceGeneration(self.source_generation),
                    )
                },
            )
        } else {
            build_detected_region_model(&self.discovery_findings, self.source_generation)
        };
        match region_result {
            Ok(regions) => {
                self.selected_region = regions.regions().first().map(|region| region.id);
                self.regions = regions;
            }
            Err(error) => {
                self.regions = RegionModel::new();
                self.selected_region = None;
                self.status = format!("Cannot build living region map: {error}");
            }
        }

        let comparison_result = if let Some(right) = &self.comparison_source {
            let left_snapshot = self
                .analysis_source
                .as_ref()
                .map(AttachedSource::descriptor)
                .map(|descriptor| SourceSnapshot {
                    source_id: descriptor.id,
                    generation: descriptor.generation,
                })
                .ok_or_else(|| "Primary comparison source is unavailable".to_owned());
            left_snapshot.and_then(|left_snapshot| {
                let right_descriptor = right.source.descriptor();
                build_bytewise_comparison(
                    self.source_bytes(),
                    &right.bytes,
                    left_snapshot,
                    SourceSnapshot {
                        source_id: right_descriptor.id,
                        generation: right_descriptor.generation,
                    },
                )
            })
        } else if self.loaded_source.is_some() {
            Err("Choose comparison source B to classify changes".to_owned())
        } else {
            self.data.as_ref().map_or_else(
                || Err("POC revision pair is unavailable".to_owned()),
                |data| build_comparison_archaeology(&data.revisions),
            )
        };
        match comparison_result {
            Ok(comparison) => {
                self.selected_comparison = comparison.regions().first().map(|region| region.id);
                self.comparison = Some(comparison);
            }
            Err(error) => {
                self.comparison = None;
                self.selected_comparison = None;
                self.status = format!("Cannot build comparison archaeology: {error}");
            }
        }
        self.branches = BranchModel::new();
        self.selected_branch = None;
        self.branch_assessments.clear();
    }

    pub(super) fn selected_discovery(&self) -> Option<&WorkbenchLead> {
        let selected = self.discovery_selected?;
        self.discovery_findings
            .iter()
            .find(|finding| finding.id == selected)
    }

    pub(super) fn select_discovery_finding(&mut self, finding_id: WorkbenchLeadId) {
        self.discovery_selected = Some(finding_id);
        self.discovery_preview_transform = false;
        if let Some(ReversibleTransform::XorByte(key)) =
            self.selected_discovery().and_then(discovery_transform)
        {
            self.branch_key = key;
        }
        self.select_discovery_range(0);
    }

    pub(super) fn select_discovery_range(&mut self, range_index: usize) {
        let Some(finding) = self.selected_discovery().cloned() else {
            return;
        };
        let Some(range) = finding.source_ranges.get(range_index).copied() else {
            return;
        };
        let (Ok(start), Ok(end)) = (usize::try_from(range.start), usize::try_from(range.end))
        else {
            "Discovery range cannot fit this platform".clone_into(&mut self.status);
            return;
        };
        self.selection = start..end;
        let provenance = discovery_provenance(&[range], self.source_generation);
        if let Err(error) = self.investigation.select_ranges(provenance) {
            self.status = format!("Cannot select discovery evidence: {error}");
        }
        self.selected_digram = None;
        self.selected_projection = None;
        self.selected_resonance = None;
        self.invalidate_dossier();
    }

    pub(super) fn browse_primary_source(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open binary source A")
            .pick_file()
        {
            if is_local_project_path(&path) {
                if let Err(error) = self.open_local_project_path(&path) {
                    self.status = error;
                }
                return;
            }
            self.path_input = path.display().to_string();
            self.load_path();
        }
    }

    pub(super) fn browse_session_source(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Reattach matching binary source")
            .pick_file()
        {
            self.path_input = path.display().to_string();
            self.reattach_session_path();
        }
    }

    pub(super) fn browse_comparison_source(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open comparison source B")
            .pick_file()
        {
            self.comparison_path_input = path.display().to_string();
            self.load_comparison_path();
        }
    }

    pub(super) const fn active_file_load(&self, slot: FileLoadSlot) -> Option<u64> {
        match slot {
            FileLoadSlot::Primary => self.primary_file_load,
            FileLoadSlot::Comparison => self.comparison_file_load,
            FileLoadSlot::SessionReattachment => self.session_file_load,
            FileLoadSlot::FocusRefinement => self.focus_file_load,
        }
    }

    pub(super) const fn set_active_file_load(
        &mut self,
        slot: FileLoadSlot,
        request_id: Option<u64>,
    ) {
        match slot {
            FileLoadSlot::Primary => self.primary_file_load = request_id,
            FileLoadSlot::Comparison => self.comparison_file_load = request_id,
            FileLoadSlot::SessionReattachment => self.session_file_load = request_id,
            FileLoadSlot::FocusRefinement => self.focus_file_load = request_id,
        }
    }

    pub(super) const fn has_active_file_load(&self) -> bool {
        self.primary_file_load.is_some()
            || self.comparison_file_load.is_some()
            || self.session_file_load.is_some()
            || self.focus_file_load.is_some()
    }

    pub(super) fn queue_file_load(
        &mut self,
        slot: FileLoadSlot,
        path: PathBuf,
        generation: SourceGeneration,
    ) -> bool {
        self.queue_file_load_with_focus(slot, path, generation, None)
    }

    pub(super) fn queue_file_load_with_focus(
        &mut self,
        slot: FileLoadSlot,
        path: PathBuf,
        generation: SourceGeneration,
        focus: Option<ByteRange>,
    ) -> bool {
        match slot {
            FileLoadSlot::Primary => {
                self.session_file_load = None;
                self.focus_file_load = None;
            }
            FileLoadSlot::SessionReattachment => self.primary_file_load = None,
            FileLoadSlot::Comparison | FileLoadSlot::FocusRefinement => {}
        }
        let request_id = self.next_file_load_request;
        self.next_file_load_request = self.next_file_load_request.saturating_add(1);
        self.set_active_file_load(slot, Some(request_id));
        let sender = self.file_load_sender.clone();
        let source_id = match slot {
            FileLoadSlot::Comparison => SourceId(2),
            FileLoadSlot::Primary
            | FileLoadSlot::SessionReattachment
            | FileLoadSlot::FocusRefinement => SourceId(1),
        };
        let display_path = path.display().to_string();
        let worker_path = path;
        let comparison_left = if slot == FileLoadSlot::Comparison {
            self.analysis_source.clone()
        } else {
            None
        };
        let primary_is_sampled = self
            .loaded_source
            .as_ref()
            .is_some_and(|source| source.sampled_overview);
        let thread_name = format!("strata-file-load-{request_id}");
        let spawn = thread::Builder::new().name(thread_name).spawn(move || {
            let result = open_local_source_with_focus(&worker_path, source_id, generation, focus)
                .and_then(|loaded| {
                    let needs_tiled_diff = slot == FileLoadSlot::Comparison
                        && (primary_is_sampled || loaded.sampled_overview);
                    let tiled_diff = if needs_tiled_diff {
                        let left = comparison_left
                            .as_ref()
                            .ok_or_else(|| "cannot build tiled diff without source A".to_owned())?;
                        Some(Arc::new(
                            build_tiled_diff(
                                left,
                                &loaded.source,
                                TiledDiffConfig::default(),
                                b"strata-poc/matched-diff",
                            )
                            .map_err(|error| format!("cannot build matched tiled diff: {error}"))?,
                        ))
                    } else {
                        None
                    };
                    Ok(FileLoadOutcome { loaded, tiled_diff })
                });
            let _ = sender.send(FileLoadMessage {
                request_id,
                slot,
                result,
            });
        });
        if let Err(error) = spawn {
            self.set_active_file_load(slot, None);
            self.status = format!("Cannot start background file load: {error}");
            return false;
        }
        self.status = format!("Opening {display_path} read-only in the background…");
        true
    }

    pub(super) fn queue_primary_path(&mut self, path: PathBuf) -> Option<SourceGeneration> {
        let generation = SourceGeneration(self.source_generation.saturating_add(1));
        self.queue_file_load(FileLoadSlot::Primary, path, generation)
            .then_some(generation)
    }

    pub(super) fn queue_comparison_path(&mut self, path: PathBuf, generation: SourceGeneration) {
        if self.queue_file_load(FileLoadSlot::Comparison, path, generation) {
            "Loading comparison source B…".clone_into(&mut self.comparison_status);
        }
    }

    pub(super) fn queue_focus_refinement(&mut self, focus: ByteRange) {
        let Some(source) = self.loaded_source.as_ref() else {
            return;
        };
        if !source.sampled_overview
            || source.resident_tiles.iter().any(|tile| {
                tile.key.precision == TilePrecision::Exact
                    && tile.coverage.start <= focus.start
                    && tile.coverage.end >= focus.end
            })
        {
            return;
        }
        let path = source.path.clone();
        let generation = source.source.descriptor().generation;
        if self.queue_file_load_with_focus(
            FileLoadSlot::FocusRefinement,
            path,
            generation,
            Some(focus),
        ) {
            self.status = format!(
                "Loading exact focus tiles for 0x{:08x}..0x{:08x} in the background…",
                focus.start, focus.end
            );
        }
    }

    pub(super) fn load_path(&mut self) {
        let input = self.path_input.trim();
        if input.is_empty() {
            "Enter a local file path first".clone_into(&mut self.status);
            return;
        }
        let path = PathBuf::from(input);
        let _ = self.queue_primary_path(path);
    }

    pub(super) fn accept_primary_source(&mut self, loaded: LoadedSource) {
        let byte_len = loaded.bytes.len();
        let source_length = loaded.source_length;
        let tile_count = loaded.resident_tiles.len();
        let resident_bytes = loaded.resident_bytes;
        let overview_level = loaded.tile_overview_level;
        let sampled_overview = loaded.sampled_overview;
        let path = loaded.path.clone();
        let generation = loaded.source.descriptor().generation;
        let analysis_source = loaded.source.clone();
        let digest_source = analysis_source.clone();
        self.session_bundle = None;
        self.session_journal = Journal::new();
        self.session_attached = true;
        self.pending_project_save = None;
        self.restored_session_selection.clear();
        self.loaded_source = Some(loaded);
        self.analysis_source = Some(analysis_source);
        self.comparison_artifact = None;
        self.pending_session_source = None;
        self.active_view = ViewKind::Discover;
        self.source_generation = generation.0;
        self.selection = 0..byte_len.min(256);
        self.selected_digram = None;
        self.selected_projection = None;
        self.selected_resonance = None;
        self.projection_cohort_anchor = None;
        self.projection_cohort_cursor = None;
        self.projection_cohort_selection = None;
        self.analytical_cohort = CohortModel::new(SourceSnapshot {
            source_id: SourceId(1),
            generation: SourceGeneration(self.source_generation),
        });
        self.request_structure_analysis();
        self.recompute_discovery();
        self.refresh_comparison_status();
        self.rebuild_workspace_models();
        self.invalidate_texture();
        self.status = if sampled_overview {
            format!(
                "Loaded {} read-only · {source_length} logical bytes · L{overview_level} overview · {tile_count} tiles / {resident_bytes} resident bytes",
                path.display()
            )
        } else {
            format!("Loaded {} read-only ({byte_len} bytes)", path.display())
        };
        self.queue_source_digest(digest_source, SourceDigestPurpose::ActiveSource);
    }

    pub(super) fn accept_focus_source(&mut self, loaded: LoadedSource) {
        let Some(current) = self.loaded_source.as_ref() else {
            return;
        };
        if current.source.local_identity() != loaded.source.local_identity()
            || current.source_length != loaded.source_length
        {
            "Exact focus refinement rejected: the opened source identity changed"
                .clone_into(&mut self.status);
            return;
        }
        let exact_tiles = loaded
            .resident_tiles
            .iter()
            .filter(|tile| tile.key.precision == TilePrecision::Exact)
            .count();
        let resident_bytes = loaded.resident_bytes;
        self.loaded_source = Some(loaded);
        self.projection_samples.clear();
        self.projection_sample_key = None;
        self.projection_field_texture = None;
        self.projection_field_key = None;
        self.status = format!(
            "Exact focus ready · {exact_tiles} level-zero tile(s) · {resident_bytes} resident bytes"
        );
    }

    pub(super) fn load_comparison_path(&mut self) {
        let input = self.comparison_path_input.trim();
        if input.is_empty() {
            "Choose a comparison source B first".clone_into(&mut self.status);
            return;
        }
        let path = PathBuf::from(input);
        let generation = SourceGeneration(self.source_generation);
        self.queue_comparison_path(path, generation);
    }

    pub(super) fn accept_comparison_source(
        &mut self,
        loaded: LoadedSource,
        tiled_diff: Option<Arc<TiledDiffArtifact>>,
    ) {
        let sampled = self
            .loaded_source
            .as_ref()
            .is_some_and(|source| source.sampled_overview)
            || loaded.sampled_overview;
        if sampled && tiled_diff.is_none() {
            "Large-source comparison failed closed because no matched tiled artifact was produced"
                .clone_into(&mut self.comparison_status);
            self.status = self.comparison_status.clone();
            return;
        }
        let path = loaded.path.clone();
        let aligned = self.source_bytes().len().min(loaded.bytes.len());
        self.comparison_source = Some(loaded);
        self.comparison_artifact = tiled_diff;
        self.refresh_comparison_status();
        self.active_view = ViewKind::RevisionDiff;
        self.selection = 0..aligned.min(256);
        self.selected_comparison = None;
        self.rebuild_workspace_models();
        self.invalidate_texture();
        self.status = if sampled {
            format!(
                "Comparison B loaded read-only with matched tiled diff: {}",
                path.display()
            )
        } else {
            format!("Comparison B loaded read-only: {}", path.display())
        };
    }

    pub(super) fn accept_session_source(&mut self, loaded: LoadedSource) {
        let digest_source = loaded.source.clone();
        self.pending_session_source = Some(loaded);
        self.queue_source_digest(digest_source, SourceDigestPurpose::SessionCandidate);
    }

    pub(super) fn queue_source_digest(
        &mut self,
        source: AttachedSource,
        purpose: SourceDigestPurpose,
    ) {
        if let Some(pending) = self.source_digest_request.take()
            && let Some(runtime) = &self.analysis_runtime
        {
            let _ = runtime.cancel_digest(pending.request_id);
        }
        let descriptor = source.descriptor();
        if let (Some(byte_length), Some(sha256)) =
            (descriptor.length, descriptor.content_digest.clone())
        {
            self.finish_source_digest(
                purpose,
                &SourceDigestArtifact {
                    source_id: descriptor.id,
                    generation: descriptor.generation,
                    byte_length,
                    sha256,
                },
            );
            return;
        }
        let Some(runtime) = &self.analysis_runtime else {
            "Whole-source fingerprint runtime is unavailable".clone_into(&mut self.status);
            return;
        };
        let request_id = AnalysisRequestId(self.next_digest_request);
        self.next_digest_request = self.next_digest_request.saturating_add(1);
        match runtime.submit_digest(RuntimeDigestRequest { request_id, source }) {
            Ok(()) => {
                self.source_digest_request = Some(PendingSourceDigest {
                    request_id,
                    purpose,
                });
                "Fingerprinting the immutable source in bounded background chunks…"
                    .clone_into(&mut self.status);
            }
            Err(error) => {
                self.status = format!("Whole-source fingerprint rejected: {error}");
            }
        }
    }

    pub(super) fn poll_source_digests(&mut self) {
        loop {
            let event = self
                .analysis_runtime
                .as_ref()
                .and_then(InvestigationRuntime::poll_digest_event);
            let Some(event) = event else {
                break;
            };
            let request_id = match &event {
                DigestRuntimeEvent::Started { request_id, .. }
                | DigestRuntimeEvent::Progress { request_id, .. }
                | DigestRuntimeEvent::Completed { request_id, .. }
                | DigestRuntimeEvent::Cancelled { request_id }
                | DigestRuntimeEvent::Stale { request_id }
                | DigestRuntimeEvent::Failed { request_id, .. } => *request_id,
            };
            let Some(pending) = self
                .source_digest_request
                .filter(|pending| pending.request_id == request_id)
            else {
                continue;
            };
            match event {
                DigestRuntimeEvent::Started { total_bytes, .. } => {
                    self.status =
                        format!("Fingerprinting immutable source · 0 / {total_bytes} bytes");
                }
                DigestRuntimeEvent::Progress {
                    bytes_hashed,
                    total_bytes,
                    ..
                } => {
                    let percent = if total_bytes == 0 {
                        100
                    } else {
                        bytes_hashed.saturating_mul(100) / total_bytes
                    };
                    self.status = format!(
                        "Fingerprinting immutable source · {bytes_hashed} / {total_bytes} bytes · {percent}%"
                    );
                }
                DigestRuntimeEvent::Completed { artifact, .. } => {
                    self.source_digest_request = None;
                    self.finish_source_digest(pending.purpose, &artifact);
                }
                DigestRuntimeEvent::Cancelled { .. } => {
                    self.source_digest_request = None;
                    if pending.purpose == SourceDigestPurpose::SessionCandidate {
                        self.pending_session_source = None;
                    } else {
                        self.pending_project_save = None;
                    }
                    "Whole-source fingerprint cancelled".clone_into(&mut self.status);
                }
                DigestRuntimeEvent::Stale { .. } => {
                    self.source_digest_request = None;
                    if pending.purpose == SourceDigestPurpose::SessionCandidate {
                        self.pending_session_source = None;
                    } else {
                        self.pending_project_save = None;
                    }
                    "Stale whole-source fingerprint suppressed".clone_into(&mut self.status);
                }
                DigestRuntimeEvent::Failed { error, .. } => {
                    self.source_digest_request = None;
                    if pending.purpose == SourceDigestPurpose::SessionCandidate {
                        self.pending_session_source = None;
                    } else {
                        self.pending_project_save = None;
                    }
                    self.status = format!("Whole-source fingerprint failed: {error}");
                }
            }
        }
    }

    pub(super) fn finish_source_digest(
        &mut self,
        purpose: SourceDigestPurpose,
        artifact: &SourceDigestArtifact,
    ) {
        match purpose {
            SourceDigestPurpose::ActiveSource => {
                let is_current = self.analysis_source.as_ref().is_some_and(|source| {
                    let descriptor = source.descriptor();
                    if descriptor.id != artifact.source_id {
                        return false;
                    }
                    descriptor.generation == artifact.generation
                });
                if is_current {
                    self.status = format!(
                        "Whole-source fingerprint sealed · {} bytes · SHA-256 {}…",
                        artifact.byte_length,
                        digest_prefix(&artifact.sha256)
                    );
                    self.complete_pending_project_save();
                }
            }
            SourceDigestPurpose::SessionCandidate => {
                if let Err(error) = self.verify_candidate_digest(artifact) {
                    self.pending_session_source = None;
                    self.status = error;
                    return;
                }
                if let Some(loaded) = self.pending_session_source.take() {
                    let analysis_source = loaded.source.clone();
                    self.loaded_source = Some(loaded);
                    self.analysis_source = Some(analysis_source);
                }
                self.activate_attached_session();
            }
        }
    }

    pub(super) fn poll_file_loads(&mut self) {
        loop {
            let message = match self.file_load_receiver.try_recv() {
                Ok(message) => message,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            if self.active_file_load(message.slot) != Some(message.request_id) {
                continue;
            }
            self.set_active_file_load(message.slot, None);
            match (message.slot, message.result) {
                (_, Err(error)) => self.status = error,
                (FileLoadSlot::Primary, Ok(outcome)) => {
                    self.accept_primary_source(outcome.loaded);
                }
                (FileLoadSlot::Comparison, Ok(outcome)) => {
                    self.accept_comparison_source(outcome.loaded, outcome.tiled_diff);
                }
                (FileLoadSlot::SessionReattachment, Ok(outcome)) => {
                    self.accept_session_source(outcome.loaded);
                }
                (FileLoadSlot::FocusRefinement, Ok(outcome)) => {
                    self.accept_focus_source(outcome.loaded);
                }
            }
        }
    }

    pub(super) fn clear_comparison_source(&mut self) {
        self.comparison_file_load = None;
        self.comparison_source = None;
        self.comparison_artifact = None;
        self.comparison_path_input.clear();
        self.comparison_status = if self.loaded_source.is_some() {
            "Choose comparison source B".to_owned()
        } else {
            "Bundled revision pair active".to_owned()
        };
        self.rebuild_workspace_models();
        self.invalidate_texture();
        "Comparison source B cleared".clone_into(&mut self.status);
    }

    pub(super) fn refresh_comparison_status(&mut self) {
        let Some(comparison) = &self.comparison_source else {
            self.comparison_status = if self.loaded_source.is_some() {
                "Choose comparison source B".to_owned()
            } else {
                "Bundled revision pair active".to_owned()
            };
            return;
        };
        if let Some(artifact) = &self.comparison_artifact {
            let precision = if artifact.is_sampled() {
                "sampled overview"
            } else {
                "exact matched tiles"
            };
            self.comparison_status = format!(
                "{} changed / {} compared bytes · {precision} L{} · {} tiles · A {} bytes · B {} bytes",
                artifact.changed_sample_bytes,
                artifact.compared_sample_bytes,
                artifact.overview_level,
                artifact.tiles.len(),
                artifact.left_length,
                artifact.right_length
            );
            return;
        }
        let aligned = self.source_bytes().len().min(comparison.bytes.len());
        let changed = self
            .source_bytes()
            .iter()
            .take(aligned)
            .zip(comparison.bytes.iter().take(aligned))
            .filter(|(left, right)| left != right)
            .count();
        self.comparison_status = format!(
            "{changed} changed aligned bytes · A {} bytes · B {} bytes",
            self.source_bytes().len(),
            comparison.bytes.len()
        );
    }

    pub(super) fn comparison_context(&self) -> bool {
        self.active_view == ViewKind::RevisionDiff
            || (self.active_view == ViewKind::Discover
                && self.workbench_mode == WorkbenchMode::Compare)
    }

    pub(super) fn handle_dropped_files(&mut self, context: &egui::Context) {
        let paths = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        let Some(first) = paths.first() else {
            return;
        };
        if is_local_project_path(first) {
            if let Err(error) = self.open_local_project_path(first) {
                self.status = error;
            }
            return;
        }
        if first.is_dir() && first.join("manifest.json").is_file() {
            self.session_path_input = first.display().to_string();
            self.open_session();
            return;
        }
        if paths.len() >= 2 {
            self.path_input = first.display().to_string();
            let generation = self.queue_primary_path(first.clone());
            if let Some(second) = paths.get(1) {
                self.comparison_path_input = second.display().to_string();
                if let Some(generation) = generation {
                    self.queue_comparison_path(second.clone(), generation);
                }
            }
        } else if self.comparison_context() {
            self.comparison_path_input = first.display().to_string();
            self.load_comparison_path();
        } else {
            self.path_input = first.display().to_string();
            self.load_path();
        }
    }

    pub(super) fn show_drop_overlay(&self, context: &egui::Context) {
        let hovering = context.input(|input| !input.raw.hovered_files.is_empty());
        if !hovering {
            return;
        }
        let label = if self.comparison_context() {
            "DROP AS COMPARISON SOURCE B"
        } else {
            "DROP TO OPEN READ-ONLY SOURCE A"
        };
        egui::Area::new(egui::Id::new("source-drop-overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(12, 20, 25, 242))
                    .stroke(egui::Stroke::new(2.0, UI_CYAN))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(30, 22))
                    .show(ui, |ui| {
                        ui.strong(label);
                        ui.weak("One file opens the shown slot; two files open A + B.");
                    });
            });
    }

    pub(super) fn restore_demo_source(&mut self) {
        self.primary_file_load = None;
        self.session_file_load = None;
        self.focus_file_load = None;
        self.session_bundle = None;
        self.session_journal = Journal::new();
        self.session_attached = true;
        self.pending_project_save = None;
        self.restored_session_selection.clear();
        self.loaded_source = None;
        self.comparison_artifact = None;
        self.pending_session_source = None;
        self.active_view = ViewKind::Discover;
        self.source_generation = self.source_generation.saturating_add(1);
        let restored_source = self.data.as_ref().map(|data| {
            retained_source(
                &data.investigation.bytes,
                SourceGeneration(self.source_generation),
                "demo://investigation-binary",
            )
        });
        self.analysis_source = match restored_source {
            Some(Ok(source)) => Some(source),
            Some(Err(error)) => {
                self.status = format!("Cannot restore fixture source: {error}");
                None
            }
            None => None,
        };
        self.selection = 320..576;
        self.selected_digram = None;
        self.selected_projection = None;
        self.selected_resonance = None;
        self.projection_cohort_anchor = None;
        self.projection_cohort_cursor = None;
        self.projection_cohort_selection = None;
        self.analytical_cohort = CohortModel::new(SourceSnapshot {
            source_id: SourceId(1),
            generation: SourceGeneration(self.source_generation),
        });
        self.request_structure_analysis();
        self.recompute_discovery();
        self.refresh_comparison_status();
        self.rebuild_workspace_models();
        self.invalidate_texture();
        "Restored deterministic firmware fixture".clone_into(&mut self.status);
    }

    pub(super) fn align_analysis_source_generation(
        &mut self,
        generation: SourceGeneration,
    ) -> Result<(), String> {
        if let Some(loaded) = &self.loaded_source {
            if loaded.source.descriptor().generation == generation {
                self.analysis_source = Some(loaded.source.clone());
                return Ok(());
            }
            let path = loaded.path.clone();
            let reopened = open_local_source(&path, SourceId(1), generation)?;
            if self.loaded_source.as_ref().is_none_or(|loaded| {
                loaded.source_length != reopened.source_length || loaded.bytes != reopened.bytes
            }) {
                return Err(
                    "Attached source changed while aligning its analysis generation".to_owned(),
                );
            }
            let analysis_source = reopened.source.clone();
            self.loaded_source = Some(reopened);
            self.analysis_source = Some(analysis_source);
            return Ok(());
        }

        let data = self
            .data
            .as_ref()
            .ok_or_else(|| "Bundled source is unavailable".to_owned())?;
        self.analysis_source = Some(
            retained_source(
                &data.investigation.bytes,
                generation,
                "demo://investigation-binary",
            )
            .map_err(|error| format!("Cannot bind fixture analysis source: {error}"))?,
        );
        Ok(())
    }
}
