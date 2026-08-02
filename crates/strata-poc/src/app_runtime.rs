//! Application initialization, cached analysis, rendering, and projection enrichment.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    #[allow(clippy::too_many_lines)]
    pub(super) fn new(
        context: &eframe::CreationContext<'_>,
        initial_path: Option<&PathBuf>,
    ) -> Self {
        configure_workbench_style(&context.egui_ctx);

        let (gpu_backend, gpu_status) = initialize_gpu_backend(context);
        let project_preferences_path = default_project_preferences_path();
        let project_preferences =
            load_project_preferences_file(&project_preferences_path).unwrap_or_default();

        let (data, mut initialization_error) = match (
            investigation_binary(),
            interleaved_sensor_image(),
            aligned_revision_pair(),
        ) {
            (Ok(investigation), Ok(sensor), Ok(revisions)) => (
                Some(PocData {
                    investigation,
                    sensor,
                    revisions,
                }),
                None,
            ),
            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => (
                None,
                Some(format!("Fixture initialization failed: {error}")),
            ),
        };
        let analysis_runtime = match InvestigationRuntime::new(ProductionRuntimeConfig::default()) {
            Ok(runtime) => Some(runtime),
            Err(error) => {
                record_initialization_error(
                    &mut initialization_error,
                    format!("Analysis runtime failed: {error}"),
                );
                None
            }
        };
        let analysis_source = data.as_ref().and_then(|fixtures| {
            match retained_source(
                &fixtures.investigation.bytes,
                SourceGeneration(0),
                "demo://investigation-binary",
            ) {
                Ok(source) => Some(source),
                Err(error) => {
                    record_initialization_error(
                        &mut initialization_error,
                        format!("Fixture source failed: {error}"),
                    );
                    None
                }
            }
        });
        let (file_load_sender, file_load_receiver) = mpsc::channel();

        let mut app = Self {
            data,
            initialization_error,
            loaded_source: None,
            comparison_source: None,
            comparison_artifact: None,
            pending_session_source: None,
            comparison_path_input: String::new(),
            comparison_status: "Bundled revision pair active".to_owned(),
            file_load_sender,
            file_load_receiver,
            next_file_load_request: 1,
            primary_file_load: None,
            comparison_file_load: None,
            session_file_load: None,
            focus_file_load: None,
            analysis_source,
            analysis_runtime,
            source_digest_request: None,
            next_digest_request: 1,
            structure_artifact: None,
            structure_request: None,
            next_analysis_request: 1,
            structure_status: "Structure analysis queued".to_owned(),
            active_view: ViewKind::Discover,
            selection: 320..576,
            drag_anchor: None,
            selected_digram: None,
            selected_projection: None,
            path_input: initial_path
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
            status: "Bundled deterministic firmware fixture ready".to_owned(),
            source_generation: 0,
            discovery_findings: Vec::new(),
            discovery_selected: None,
            discovery_generation: None,
            discovery_preview_transform: false,
            discovery_error: None,
            signature_catalog: None,
            signature_pack_path_input: std::env::var_os("STRATA_SIGNATURE_PACK").map_or_else(
                || project_preferences.default_signature_pack_path.clone(),
                |path| PathBuf::from(path).display().to_string(),
            ),
            signature_pack_status: "Built-in five-signature fallback active".to_owned(),
            signature_scan_status: "No external knowledge pack loaded".to_owned(),
            project_path_input: project_preferences.last_project_path.clone(),
            reopen_last_project: project_preferences.reopen_last_project,
            show_project_preferences: false,
            pending_project_save: None,
            project_preferences_path,
            investigation: InvestigationModel::new(),
            workbench_mode: WorkbenchMode::Leads,
            regions: RegionModel::new(),
            selected_region: None,
            comparison: None,
            selected_comparison: None,
            branches: BranchModel::new(),
            selected_branch: None,
            branch_key: 0xa7,
            branch_assessments: BTreeMap::new(),
            session_path_input: std::env::temp_dir()
                .join("strata-investigation.strata-session")
                .display()
                .to_string(),
            session_bundle: None,
            session_journal: Journal::new(),
            session_attached: true,
            restored_session_selection: Vec::new(),
            atlas_width: 32,
            digram_stride: 1,
            interleave_width: 24,
            interleave_stride: 6,
            interleave_lane: 5,
            bit_plane: 3,
            diff_width: 32,
            projection_point_budget: 12_000,
            projection_composition: ProjectionComposition::default(),
            projection_relief: 1.0,
            projection_context_light: 0.72,
            projection_point_size: 1.8,
            projection_brightness: 1.25,
            projection_perspective: 0.72,
            projection_field_radius: 20.0,
            projection_field_exposure: 1.4,
            projection_contour_mode: ProjectionContourMode::default(),
            projection_yaw: -0.72,
            projection_pitch: 0.38,
            projection_zoom: 0.92,
            projection_spin: false,
            projection_auto_morph: false,
            projection_speed: 0.32,
            projection_phase: 0.0,
            projection_interaction: ProjectionInteraction::Rotate,
            projection_cohort_anchor: None,
            projection_cohort_cursor: None,
            projection_cohort_selection: None,
            analytical_cohort: CohortModel::new(SourceSnapshot {
                source_id: SourceId(1),
                generation: SourceGeneration(0),
            }),
            projection_samples: Vec::new(),
            alignment_candidates: Vec::new(),
            gpu_backend,
            gpu_status,
            projection_sample_key: None,
            projection_field_texture: None,
            projection_field_key: None,
            resonance_metric: ResonanceMetric::ByteShape,
            resonance_base_window: 8,
            resonance_stride: 1,
            resonance_sample_budget: 1_024,
            resonance_layers: Vec::new(),
            resonance_key: None,
            selected_resonance: None,
            dossier: None,
            dossier_key: None,
            dossier_error: None,
            dossier_epoch: 0,
            video_output_path: std::env::temp_dir()
                .join("strata-morph.mp4")
                .display()
                .to_string(),
            video_duration_seconds: 4.0,
            video_fps: 30,
            video_width: 1_280,
            video_height: 720,
            video_overwrite: false,
            video_export_receiver: None,
            entropy: Vec::new(),
            texture_tiles: Vec::new(),
            texture_key: None,
            texture_dimensions: [1, 1],
            active_mapping: None,
            render_error: None,
        };
        app.request_structure_analysis();
        app.recompute_discovery();
        if !app.signature_pack_path_input.is_empty() {
            app.load_signature_pack();
        }
        app.rebuild_workspace_models();
        if let Some(path) = initial_path {
            if is_local_project_path(path) {
                if let Err(error) = app.open_local_project_path(path) {
                    app.status = error;
                }
            } else if path.is_dir() && path.join("manifest.json").is_file() {
                app.session_path_input = path.display().to_string();
                app.path_input.clear();
                app.open_session();
            } else {
                app.load_path();
            }
        } else if app.reopen_last_project && !app.project_path_input.trim().is_empty() {
            let project_path = PathBuf::from(app.project_path_input.trim());
            if let Err(error) = app.open_local_project_path(&project_path) {
                app.status = format!("Could not reopen the last local project: {error}");
            }
        }
        app
    }

    pub(super) fn source_bytes(&self) -> &[u8] {
        if self.session_bundle.is_some() && !self.session_attached {
            return &[];
        }
        self.raw_source_bytes()
    }

    pub(super) fn raw_source_bytes(&self) -> &[u8] {
        if let Some(source) = &self.loaded_source {
            &source.bytes
        } else if let Some(data) = &self.data {
            &data.investigation.bytes
        } else {
            &[]
        }
    }

    pub(super) fn logical_source_length(&self) -> u64 {
        self.loaded_source.as_ref().map_or_else(
            || u64::try_from(self.raw_source_bytes().len()).unwrap_or(u64::MAX),
            |source| source.source_length,
        )
    }

    pub(super) fn comparison_target_bytes(&self) -> &[u8] {
        if let Some(source) = &self.comparison_source {
            return &source.bytes;
        }
        if self.loaded_source.is_some() {
            return self.source_bytes();
        }
        self.data
            .as_ref()
            .map_or(&[] as &[u8], |data| data.revisions.after.as_slice())
    }

    pub(super) fn comparison_target_name(&self) -> &str {
        if let Some(source) = &self.comparison_source {
            return &source.display_name;
        }
        if let Some(source) = &self.loaded_source {
            return &source.display_name;
        }
        "demo://revision-pair/after"
    }

    pub(super) fn active_bytes(&self) -> &[u8] {
        if self.session_bundle.is_some() && !self.session_attached {
            return &[];
        }
        match self.active_view {
            ViewKind::Discover if self.workbench_mode == WorkbenchMode::Compare => {
                self.comparison_target_bytes()
            }
            ViewKind::Discover
            | ViewKind::Projection3d
            | ViewKind::Resonance
            | ViewKind::Structure
            | ViewKind::Grammar => self.source_bytes(),
            ViewKind::Interleave => self
                .data
                .as_ref()
                .map_or(&[], |data| data.sensor.bytes.as_slice()),
            ViewKind::RevisionDiff => self.comparison_target_bytes(),
        }
    }

    pub(super) fn source_name(&self) -> &str {
        if let Some(bundle) = &self.session_bundle
            && !self.session_attached
        {
            return bundle.manifest().source().alias();
        }
        match self.active_view {
            ViewKind::Discover if self.workbench_mode == WorkbenchMode::Compare => {
                self.comparison_target_name()
            }
            ViewKind::Discover
            | ViewKind::Projection3d
            | ViewKind::Resonance
            | ViewKind::Structure
            | ViewKind::Grammar => self
                .loaded_source
                .as_ref()
                .map_or("demo://investigation-binary", |source| {
                    source.display_name.as_str()
                }),
            ViewKind::Interleave => "demo://interleaved-sensor",
            ViewKind::RevisionDiff => self.comparison_target_name(),
        }
    }

    pub(super) fn clamped_selection(&self, length: usize) -> Range<usize> {
        let start = self.selection.start.min(length);
        let end = self.selection.end.min(length).max(start);
        start..end
    }

    pub(super) fn inspector_selection(&self) -> Range<usize> {
        if (self.active_view == ViewKind::Projection3d
            || self.active_view == ViewKind::RevisionDiff)
            && let Some(source) = self
                .loaded_source
                .as_ref()
                .filter(|source| source.sampled_overview)
        {
            let length = usize::try_from(source.source_length).unwrap_or(usize::MAX);
            return self.clamped_selection(length);
        }
        self.clamped_selection(self.active_bytes().len())
    }

    pub(super) fn exact_selection_preview(&self, selection: &Range<usize>) -> Option<Vec<u8>> {
        let preview_end = selection
            .start
            .saturating_add(
                selection
                    .end
                    .saturating_sub(selection.start)
                    .min(INSPECTOR_SELECTION_PREVIEW_BYTES),
            )
            .min(selection.end);
        if preview_end <= selection.start {
            return Some(Vec::new());
        }
        if self.active_view == ViewKind::Projection3d
            && let Some(source) = self
                .loaded_source
                .as_ref()
                .filter(|source| source.sampled_overview)
        {
            let start = u64::try_from(selection.start).ok()?;
            let end = u64::try_from(preview_end).ok()?;
            let tile = source
                .resident_tiles
                .iter()
                .find(|tile| tile.read_range.start <= start && tile.read_range.end >= end)?;
            let local_start = usize::try_from(start.saturating_sub(tile.read_range.start)).ok()?;
            let local_end = usize::try_from(end.saturating_sub(tile.read_range.start)).ok()?;
            return tile.bytes.get(local_start..local_end).map(<[u8]>::to_vec);
        }
        if self.active_view == ViewKind::RevisionDiff
            && let Some(artifact) = &self.comparison_artifact
        {
            let start = u64::try_from(selection.start).ok()?;
            let end = u64::try_from(preview_end).ok()?;
            let tile = artifact
                .tiles
                .iter()
                .find(|tile| tile.read_range.start <= start && tile.read_range.end >= end)?;
            let local_start = usize::try_from(start.saturating_sub(tile.read_range.start)).ok()?;
            let local_end = usize::try_from(end.saturating_sub(tile.read_range.start)).ok()?;
            return tile
                .right_bytes
                .get(local_start..local_end)
                .map(<[u8]>::to_vec);
        }
        self.active_bytes()
            .get(selection.start..preview_end)
            .map(<[u8]>::to_vec)
    }

    pub(super) fn selected_or_full<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let selection = self.clamped_selection(bytes.len());
        if selection.end.saturating_sub(selection.start) >= 2 {
            bytes.get(selection).unwrap_or(bytes)
        } else {
            bytes
        }
    }

    pub(super) fn render_key(&self, max_texture_side: usize) -> RenderKey {
        RenderKey {
            view: self.active_view,
            generation: self.source_generation,
            selection: self.selection.clone(),
            width: match self.active_view {
                ViewKind::Discover | ViewKind::Projection3d | ViewKind::Resonance => 0,
                ViewKind::Structure => self.atlas_width,
                ViewKind::Grammar => 256,
                ViewKind::Interleave => self.interleave_width,
                ViewKind::RevisionDiff => self.diff_width,
            },
            stride: match self.active_view {
                ViewKind::Projection3d => self.projection_composition.parameters.lag,
                ViewKind::Resonance => self.resonance_stride,
                ViewKind::Grammar => self.digram_stride,
                ViewKind::Interleave => self.interleave_stride,
                ViewKind::Discover | ViewKind::Structure | ViewKind::RevisionDiff => 1,
            },
            lane: self.interleave_lane,
            bit: self.bit_plane,
            max_texture_side,
        }
    }

    pub(super) fn render_current(&self) -> Result<(RgbaImage, ActiveMapping), String> {
        let data = self
            .data
            .as_ref()
            .ok_or_else(|| "POC fixtures are unavailable".to_owned())?;
        match self.active_view {
            ViewKind::Discover => Err("discovery uses the live evidence renderer".to_owned()),
            ViewKind::Projection3d => Err("3D projection uses the live vector renderer".to_owned()),
            ViewKind::Resonance => Err("resonance uses the live vector renderer".to_owned()),
            ViewKind::Structure => {
                let artifact = self
                    .structure_artifact
                    .as_ref()
                    .filter(|artifact| artifact.generation.0 == self.source_generation)
                    .ok_or_else(|| self.structure_status.clone())?;
                let classified = artifact
                    .classified_ranges
                    .first()
                    .filter(|classified| {
                        classified.range.start == 0
                            && classified.classes.len() == self.source_bytes().len()
                    })
                    .ok_or_else(|| {
                        "Structure artifact does not cover the active source exactly".to_owned()
                    })?;
                let (image, layout) =
                    render_classified_byte_atlas(&classified.classes, self.atlas_width)
                        .ok_or_else(|| "Structure atlas dimensions overflowed".to_owned())?;
                Ok((image, ActiveMapping::Raster(layout)))
            }
            ViewKind::Grammar => {
                let bytes = self.selected_or_full(self.source_bytes());
                let counts =
                    digram_counts(bytes, self.digram_stride).map_err(|error| error.to_string())?;
                let image = render_log_digram_matrix(&counts.counts)
                    .ok_or_else(|| "Digram matrix could not be rendered".to_owned())?;
                Ok((image, ActiveMapping::Digram))
            }
            ViewKind::Interleave => {
                let config = BitPlaneConfig {
                    width: self.interleave_width,
                    stride: self.interleave_stride,
                    lane: self.interleave_lane,
                    bit: self.bit_plane,
                };
                let (image, layout) = render_bit_plane_stride_atlas(&data.sensor.bytes, config)
                    .ok_or_else(|| "Interleave controls produced an invalid layout".to_owned())?;
                Ok((image, ActiveMapping::BitPlane(layout)))
            }
            ViewKind::RevisionDiff => {
                let (before, after) = if let Some(comparison) = &self.comparison_source {
                    (self.source_bytes(), comparison.bytes.as_slice())
                } else if self.loaded_source.is_some() {
                    return Err("Choose comparison source B to render a custom diff".to_owned());
                } else {
                    (
                        data.revisions.before.as_slice(),
                        data.revisions.after.as_slice(),
                    )
                };
                let (image, layout) = render_revision_diff_atlas(before, after, self.diff_width)
                    .ok_or_else(|| "Revision diff dimensions overflowed".to_owned())?;
                Ok((image, ActiveMapping::Raster(layout)))
            }
        }
    }

    pub(super) fn ensure_texture(&mut self, context: &egui::Context) {
        let max_texture_side = context.input(|input| input.max_texture_side);
        let key = self.render_key(max_texture_side);
        if self.texture_key.as_ref() == Some(&key) {
            return;
        }

        match self.render_current() {
            Ok((image, mapping)) => {
                let dimensions = [image.width, image.height];
                let name = format!(
                    "strata-poc-{:?}-{}-{}",
                    self.active_view, self.source_generation, key.width
                );
                match upload_raster_texture_tiles(context, &name, &image, max_texture_side) {
                    Ok(tiles) => {
                        self.texture_tiles = tiles;
                        self.texture_dimensions = dimensions;
                        self.active_mapping = Some(mapping);
                        self.render_error = None;
                        self.texture_key = Some(key);
                    }
                    Err(error) => {
                        self.texture_tiles.clear();
                        self.active_mapping = None;
                        self.render_error = Some(error);
                        self.texture_key = Some(key);
                    }
                }
            }
            Err(error) => {
                self.texture_tiles.clear();
                self.active_mapping = None;
                self.render_error = Some(error);
                self.texture_key = Some(key);
            }
        }
    }

    pub(super) fn invalidate_texture(&mut self) {
        self.texture_key = None;
        self.invalidate_dossier();
    }

    pub(super) fn invalidate_dossier(&mut self) {
        self.dossier = None;
        self.dossier_key = None;
        self.dossier_error = None;
        self.dossier_epoch = self.dossier_epoch.saturating_add(1);
    }

    pub(super) fn ensure_dossier(&mut self) {
        if self.active_view == ViewKind::Interleave
            || (self.session_bundle.is_some() && !self.session_attached)
        {
            self.dossier = None;
            self.dossier_key = None;
            self.dossier_error = None;
            return;
        }

        let comparison_active = self.comparison_context() && self.comparison.is_some();
        let snapshot = if comparison_active {
            self.comparison
                .as_ref()
                .map(|comparison| comparison.pair.right)
        } else {
            self.analysis_source
                .as_ref()
                .map(AttachedSource::descriptor)
                .map(|descriptor| SourceSnapshot {
                    source_id: descriptor.id,
                    generation: descriptor.generation,
                })
        };
        let Some(snapshot) = snapshot else {
            self.dossier = None;
            self.dossier_key = None;
            self.dossier_error = Some("No exact source snapshot is attached".to_owned());
            return;
        };
        let source_bytes = if comparison_active {
            self.comparison_target_bytes()
        } else {
            self.source_bytes()
        };
        let selection = self.clamped_selection(source_bytes.len());
        if selection.is_empty() {
            self.dossier = None;
            self.dossier_key = None;
            self.dossier_error = Some("Select at least one exact byte".to_owned());
            return;
        }
        let (Ok(start), Ok(end)) = (u64::try_from(selection.start), u64::try_from(selection.end))
        else {
            self.dossier = None;
            self.dossier_key = None;
            self.dossier_error = Some("Selection cannot fit exact provenance".to_owned());
            return;
        };
        let ranges = ByteRangeSet {
            ranges: vec![ByteRange { start, end }],
        };
        let artifact = self.structure_artifact.as_ref().filter(|artifact| {
            artifact.source_id == snapshot.source_id && artifact.generation == snapshot.generation
        });
        let artifact_digest = artifact.map(|artifact| artifact.artifact_digest.clone());
        let key = DossierKey {
            source_id: snapshot.source_id,
            generation: snapshot.generation,
            ranges: vec![(start, end)],
            epoch: self.dossier_epoch,
            structure_artifact_digest: artifact_digest.clone(),
        };
        if self.dossier_key.as_ref() == Some(&key) {
            return;
        }
        let empty_entropy = &[] as &[EntropyBlock];
        let entropy_blocks =
            artifact.map_or(empty_entropy, |artifact| artifact.entropy_blocks.as_slice());
        let provenance = ExactProvenance {
            source_id: snapshot.source_id,
            generation: snapshot.generation,
            ranges,
        };
        let result = build_investigation_dossier(DossierContext {
            source_bytes,
            selection: provenance,
            entropy_blocks,
            structure_artifact_digest: artifact_digest.as_deref(),
            investigation: &self.investigation,
            regions: &self.regions,
            branches: &self.branches,
            comparison: self.comparison.as_ref(),
        });
        self.dossier_key = Some(key);
        match result {
            Ok(dossier) => {
                self.dossier = Some(dossier);
                self.dossier_error = None;
            }
            Err(error) => {
                self.dossier = None;
                self.dossier_error = Some(format!("Dossier unavailable: {error}"));
            }
        }
    }

    pub(super) fn ensure_projection_samples(&mut self) {
        let sampling = ProjectionSamplingConfig::from(self.projection_composition);
        let point_budget = self
            .projection_point_budget
            .checked_div(projection_instance_multiplier(self.projection_composition))
            .unwrap_or(1)
            .max(1);
        let key = ProjectionSampleKey {
            generation: self.source_generation,
            sampling,
            parameters: self.projection_composition.parameters,
            projection_a: self.projection_composition.projection_a,
            projection_b: self.projection_composition.projection_b,
            point_budget,
        };
        if self.projection_sample_key == Some(key) {
            return;
        }
        let analytical_budget = p1_point_budget(self.projection_composition, point_budget);
        self.projection_samples = if let Some(source) = self
            .loaded_source
            .as_ref()
            .filter(|source| !source.resident_tiles.is_empty())
        {
            let Ok(source_length) = usize::try_from(source.source_length) else {
                "Source length does not fit this platform's projection offsets"
                    .clone_into(&mut self.status);
                return;
            };
            let per_tile = analytical_budget
                .checked_div(source.resident_tiles.len())
                .unwrap_or(1)
                .max(1);
            let mut by_point = BTreeMap::<u64, (TilePrecision, ProjectionSample)>::new();
            for tile in &source.resident_tiles {
                let Ok(base_offset) = usize::try_from(tile.read_range.start) else {
                    continue;
                };
                for sample in sample_projection_samples_in_source(
                    &tile.bytes,
                    base_offset,
                    source_length,
                    sampling,
                    per_tile,
                ) {
                    let replace = by_point.get(&sample.point_id).is_none_or(|(precision, _)| {
                        *precision == TilePrecision::OverviewSample
                            && tile.key.precision == TilePrecision::Exact
                    });
                    if replace {
                        by_point.insert(sample.point_id, (tile.key.precision, sample));
                    }
                }
            }
            by_point.into_values().map(|(_, sample)| sample).collect()
        } else {
            sample_projection_samples_in_source(
                self.source_bytes(),
                0,
                self.source_bytes().len(),
                sampling,
                analytical_budget,
            )
        };
        let signature_offsets = signature_projection_offsets(&self.discovery_findings);
        if !signature_offsets.is_empty() {
            let source_length = usize::try_from(self.logical_source_length())
                .map_or_else(|_| self.source_bytes().len(), std::convert::identity);
            let evidence_samples = {
                let bytes = self.source_bytes();
                signature_offsets
                    .into_iter()
                    .filter_map(|offset| {
                        sample_projection_sample_at_source_offset(
                            bytes,
                            0,
                            source_length,
                            sampling,
                            offset,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let mut retained_ids = self
                .projection_samples
                .iter()
                .map(|sample| sample.point_id)
                .collect::<BTreeSet<_>>();
            self.projection_samples.extend(
                evidence_samples
                    .into_iter()
                    .filter(|sample| retained_ids.insert(sample.point_id)),
            );
            self.projection_samples
                .sort_unstable_by_key(|sample| sample.point_id);
        }
        self.enrich_projection_samples_with_p1();
        self.projection_sample_key = Some(key);
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn enrich_projection_samples_with_p1(&mut self) {
        let request = p1_feature_request(self.projection_composition);
        if !composition_uses_p1(self.projection_composition) {
            self.alignment_candidates.clear();
            return;
        }
        let source_length = self.logical_source_length();
        if source_length == 0 {
            return;
        }
        let config = p1_analysis_config(self.projection_composition.parameters);
        let mut records = Vec::new();
        let mut candidates = Vec::new();
        let analysis_result = if let Some(source) = self
            .loaded_source
            .as_ref()
            .filter(|source| !source.resident_tiles.is_empty())
        {
            for tile in &source.resident_tiles {
                let ranges = projection_ranges_inside(&self.projection_samples, tile.read_range);
                if ranges.is_empty() {
                    continue;
                }
                match analyze_p1_tile(
                    &tile.bytes,
                    tile.read_range.start,
                    source_length,
                    &ranges,
                    config,
                    request,
                    tile.key.precision == TilePrecision::OverviewSample,
                ) {
                    Ok(artifact) => {
                        candidates.extend(artifact.alignment_candidates);
                        records.extend(artifact.records);
                    }
                    Err(error) => {
                        self.status = format!("P1 tile reference failed: {error}");
                        return;
                    }
                }
            }
            Ok(())
        } else {
            let range = ByteRange::new(0, source_length);
            range.and_then(|resident| {
                let ranges = projection_ranges_inside(&self.projection_samples, resident);
                analyze_p1_tile(
                    self.source_bytes(),
                    0,
                    source_length,
                    &ranges,
                    config,
                    request,
                    false,
                )
                .map(|artifact| {
                    candidates = artifact.alignment_candidates;
                    records = artifact.records;
                })
            })
        };
        if let Err(error) = analysis_result {
            self.status = format!("P1 CPU reference failed: {error}");
            return;
        }
        self.alignment_candidates = merge_alignment_candidates(&candidates, 8);
        if composition_uses_gpu_coordinates(self.projection_composition)
            && let Some(backend) = &self.gpu_backend
        {
            let data = self
                .projection_samples
                .iter()
                .map(|sample| P1GpuDatum {
                    offset: sample.point_id,
                    byte: sample.primary_byte(),
                })
                .collect::<Vec<_>>();
            match backend.project(
                &data,
                source_length,
                self.projection_composition.parameters.alignment_stride,
            ) {
                Ok(gpu) => {
                    let gpu_by_point = data
                        .iter()
                        .zip(gpu)
                        .map(|(datum, projected)| (datum.offset, projected))
                        .collect::<BTreeMap<_, _>>();
                    for record in &mut records {
                        if let Some(projected) = gpu_by_point.get(&record.point_id) {
                            record.alignment = projected.alignment;
                            record.hypercube = projected.hypercube;
                        }
                    }
                }
                Err(error) => {
                    self.gpu_status = format!("CPU fallback · GPU dispatch failed: {error}");
                }
            }
        }
        let record_map = records
            .into_iter()
            .map(|record| (record.point_id, record))
            .collect::<BTreeMap<_, _>>();
        for sample in &mut self.projection_samples {
            if let Some(record) = record_map.get(&sample.point_id).copied() {
                sample.attach_p1(record);
            }
        }
    }

    pub(super) fn ensure_resonance_layers(&mut self) {
        let source_length = self.source_bytes().len();
        if source_length == 0 {
            self.resonance_layers.clear();
            self.resonance_key = None;
            return;
        }
        let probe_offset = self.selection.start.min(source_length.saturating_sub(1));
        let key = ResonanceKey {
            generation: self.source_generation,
            probe_offset,
            base_window: self.resonance_base_window,
            stride: self.resonance_stride,
            sample_budget: self.resonance_sample_budget,
            metric: self.resonance_metric,
        };
        if self.resonance_key == Some(key) {
            return;
        }

        let layers = [1_usize, 2, 4, 8, 16]
            .into_iter()
            .map(|multiplier| {
                selection_resonance(
                    self.source_bytes(),
                    probe_offset,
                    self.resonance_base_window.saturating_mul(multiplier),
                    self.resonance_stride,
                    self.resonance_sample_budget,
                    self.resonance_metric,
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>();
        match layers {
            Ok(layers) => {
                self.resonance_layers = layers;
                self.render_error = None;
            }
            Err(error) => {
                self.resonance_layers.clear();
                self.render_error = Some(format!("Selection resonance failed: {error}"));
            }
        }
        self.resonance_key = Some(key);
    }

    pub(super) fn structure_preset(&self) -> Result<StructureEntropyPreset, String> {
        let entropy_block_size = self.source_bytes().len().div_ceil(64).max(1);
        Ok(StructureEntropyPreset {
            atlas_width: u32::try_from(self.atlas_width)
                .map_err(|_| "atlas width cannot fit the production preset".to_owned())?,
            entropy_block_size: u32::try_from(entropy_block_size)
                .map_err(|_| "entropy block cannot fit the production preset".to_owned())?,
        })
    }

    pub(super) fn request_structure_analysis(&mut self) {
        if let Some(request_id) = self.structure_request.take()
            && let Some(runtime) = &self.analysis_runtime
        {
            let _ = runtime.cancel(request_id);
        }
        self.structure_artifact = None;
        self.entropy.clear();
        self.invalidate_texture();

        if self.session_bundle.is_some() && !self.session_attached {
            "Structure unavailable until the source is reattached"
                .clone_into(&mut self.structure_status);
            return;
        }
        let Some(source) = self.analysis_source.clone() else {
            "No production source is attached".clone_into(&mut self.structure_status);
            return;
        };
        let Some(runtime) = self.analysis_runtime.as_ref() else {
            "Production analysis runtime is unavailable".clone_into(&mut self.structure_status);
            return;
        };
        let preset = match self.structure_preset() {
            Ok(preset) => preset,
            Err(error) => {
                self.structure_status = error;
                return;
            }
        };
        let Ok(source_length) = u64::try_from(self.source_bytes().len()) else {
            "Source length cannot fit the analysis contract".clone_into(&mut self.structure_status);
            return;
        };
        let range = match ByteRange::new(0, source_length) {
            Ok(range) => range,
            Err(error) => {
                self.structure_status = format!("Structure range failed: {error}");
                return;
            }
        };
        let request_id = AnalysisRequestId(self.next_analysis_request);
        self.next_analysis_request = self.next_analysis_request.saturating_add(1);
        let request = RuntimeStructureRequest {
            request_id,
            source,
            ranges: ByteRangeSet {
                ranges: vec![range],
            },
            preset,
            priority: Priority::Visible,
        };
        match runtime.submit_structure(request) {
            Ok(()) => {
                self.structure_request = Some(request_id);
                self.structure_status = if let Some(source) = self
                    .loaded_source
                    .as_ref()
                    .filter(|source| source.sampled_overview)
                {
                    format!(
                        "Analyzing exact {}-byte prefix of {} logical bytes; tiled overview remains available in 3D",
                        source.bytes.len(),
                        source.source_length
                    )
                } else {
                    format!(
                        "Analyzing generation {} in bounded background chunks",
                        self.source_generation
                    )
                };
            }
            Err(error) => {
                self.structure_status = format!("Structure analysis rejected: {error}");
            }
        }
    }

    pub(super) fn poll_production_analysis(&mut self) {
        loop {
            let event = self
                .analysis_runtime
                .as_ref()
                .and_then(InvestigationRuntime::poll_event);
            let Some(event) = event else {
                break;
            };
            match event {
                ProductionRuntimeEvent::Started { request_id } => {
                    if self.structure_request == Some(request_id) {
                        "Reading exact source ranges".clone_into(&mut self.structure_status);
                    }
                }
                ProductionRuntimeEvent::Completed {
                    request_id,
                    artifact,
                    cache_hit,
                } => {
                    if self.structure_request != Some(request_id)
                        || artifact.generation.0 != self.source_generation
                    {
                        continue;
                    }
                    self.entropy.clone_from(&artifact.entropy_blocks);
                    let scope = self
                        .loaded_source
                        .as_ref()
                        .filter(|source| source.sampled_overview)
                        .map_or_else(
                            || "full source".to_owned(),
                            |source| {
                                format!(
                                    "exact {}-byte prefix / {} logical bytes",
                                    source.bytes.len(),
                                    source.source_length
                                )
                            },
                        );
                    self.structure_status = format!(
                        "Ready · {scope} · generation {} · {} · artifact {}…",
                        artifact.generation.0,
                        if cache_hit { "cache hit" } else { "computed" },
                        digest_prefix(&artifact.artifact_digest)
                    );
                    self.structure_artifact = Some(artifact);
                    self.structure_request = None;
                    self.invalidate_texture();
                }
                ProductionRuntimeEvent::Cancelled { request_id } => {
                    if self.structure_request == Some(request_id) {
                        self.structure_request = None;
                        "Structure analysis cancelled".clone_into(&mut self.structure_status);
                    }
                }
                ProductionRuntimeEvent::Stale { request_id } => {
                    if self.structure_request == Some(request_id) {
                        self.structure_request = None;
                        "Stale structure result suppressed".clone_into(&mut self.structure_status);
                    }
                }
                ProductionRuntimeEvent::Failed { request_id, error } => {
                    if self.structure_request == Some(request_id) {
                        self.structure_request = None;
                        self.structure_status = format!("Structure analysis failed: {error}");
                    }
                }
            }
        }
    }
}
