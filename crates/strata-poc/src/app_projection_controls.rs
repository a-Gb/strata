//! Projection composition, rendering, and video controls.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

impl StrataPoc {
    pub(super) fn show_projection_controls(&mut self, ui: &mut egui::Ui) {
        self.show_projection_composition_controls(ui);
        if composition_uses_p1(self.projection_composition) {
            ui.separator();
            ui.label(
                egui::RichText::new(&self.gpu_status)
                    .monospace()
                    .size(10.0)
                    .color(if self.gpu_backend.is_some() {
                        UI_TEAL
                    } else {
                        UI_AMBER
                    }),
            );
            ui.weak("Search, DFT, and hierarchy retain bounded CPU reference semantics.");
        }
        self.show_projection_lens_controls(ui);
        self.show_projection_field_controls(ui);
        ui.collapsing("Sampling / appearance", |ui| {
            self.show_projection_render_controls(ui);
        });
    }

    pub(super) fn apply_projection_defaults(&mut self, projection: ProjectionKind) {
        let composition = &mut self.projection_composition;
        composition.projection_a = projection;
        match projection {
            ProjectionKind::AddressRaster
            | ProjectionKind::Bitplanes
            | ProjectionKind::AlignmentLattice
            | ProjectionKind::HammingHypercube => {
                composition.domain = ProjectionDomain::Byte;
                composition.geometry = ProjectionGeometry::Voxels;
            }
            ProjectionKind::Hilbert => {
                composition.domain = ProjectionDomain::Window;
                composition.geometry = ProjectionGeometry::Voxels;
                composition.parameters.window_bytes = 32;
                composition.parameters.hop_bytes = 2;
                composition.parameters.aggregation_bytes = 2;
            }
            ProjectionKind::Transitions => {
                composition.domain = ProjectionDomain::Byte;
                composition.geometry = ProjectionGeometry::Points;
            }
            ProjectionKind::Complexity | ProjectionKind::RecurrencePlane => {
                composition.domain = ProjectionDomain::Window;
                composition.geometry = ProjectionGeometry::Points;
            }
            ProjectionKind::Sections => {
                composition.domain = ProjectionDomain::Region;
                composition.geometry = ProjectionGeometry::Voxels;
                composition.parameters.aggregation_bytes = 64;
            }
            ProjectionKind::SpectralWaterfall | ProjectionKind::HierarchicalBlockVolume => {
                composition.domain = ProjectionDomain::Window;
                composition.geometry = ProjectionGeometry::Voxels;
            }
            ProjectionKind::RepetitionSkyline => {
                composition.domain = ProjectionDomain::Window;
                composition.geometry = ProjectionGeometry::Path;
            }
            ProjectionKind::PolarAddressPath | ProjectionKind::HelicalAddressPath => {
                composition.domain = ProjectionDomain::Byte;
                composition.geometry = ProjectionGeometry::Path;
            }
        }
        self.projection_auto_morph = false;
        self.projection_sample_key = None;
        self.projection_field_key = None;
        self.status = format!("Projection A set to {}", projection.label());
    }

    pub(super) fn show_projection_composition_controls(&mut self, ui: &mut egui::Ui) {
        rail_group_label(ui, "DOMAIN");
        rail_segmented(
            ui,
            &mut self.projection_composition.domain,
            &[
                (ProjectionDomain::Byte, "Byte"),
                (ProjectionDomain::Word, "Word"),
                (ProjectionDomain::Window, "Window"),
                (ProjectionDomain::Region, "Region"),
            ],
        );

        rail_group_label(ui, "PROJECTION");
        let previous = self.projection_composition.projection_a;
        if rail_projection_grid(ui, &mut self.projection_composition.projection_a) {
            self.apply_projection_defaults(self.projection_composition.projection_a);
        }
        ui.horizontal(|ui| {
            ui.monospace(self.projection_composition.projection_a.family_label());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(self.projection_composition.projection_a.evidence_label());
            });
        });
        if previous != self.projection_composition.projection_a {
            self.projection_sample_key = None;
        }

        rail_group_label(ui, "GEOMETRY");
        rail_segmented(
            ui,
            &mut self.projection_composition.geometry,
            &[
                (ProjectionGeometry::Points, "Points"),
                (ProjectionGeometry::Path, "Path"),
                (ProjectionGeometry::Voxels, "Voxels"),
                (ProjectionGeometry::Surface, "Surface"),
            ],
        );

        rail_group_label(ui, "COMPARE");
        if rail_segmented(
            ui,
            &mut self.projection_composition.compare_mode,
            &[
                (ProjectionCompareMode::Single, "Single"),
                (ProjectionCompareMode::Split, "Split"),
                (ProjectionCompareMode::Overlay, "Overlay"),
                (ProjectionCompareMode::Morph, "Morph"),
            ],
        ) {
            self.projection_auto_morph = false;
        }
        rail_projection_combo(
            ui,
            "projection-slot-a",
            "A",
            &mut self.projection_composition.projection_a,
        );
        ui.add_enabled_ui(
            self.projection_composition.compare_mode != ProjectionCompareMode::Single,
            |ui| {
                rail_projection_combo(
                    ui,
                    "projection-slot-b",
                    "B",
                    &mut self.projection_composition.projection_b,
                );
            },
        );
        let mix_enabled = matches!(
            self.projection_composition.compare_mode,
            ProjectionCompareMode::Overlay | ProjectionCompareMode::Morph
        );
        ui.add_enabled_ui(mix_enabled, |ui| {
            let mix = format!("{:.0}%", self.projection_composition.mix * 100.0);
            if rail_slider_row(
                ui,
                "Mix",
                mix,
                egui::Slider::new(&mut self.projection_composition.mix, 0.0..=1.0),
            ) {
                self.projection_auto_morph = false;
                self.projection_phase = projection_phase_for_mix(self.projection_composition.mix);
            }
        });
        ui.weak("A/B coordinates share stable point IDs and exact source contributors.");

        rail_group_label(ui, "PARAMETERS");
        self.show_projection_context_parameters(ui);
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn show_projection_context_parameters(&mut self, ui: &mut egui::Ui) {
        let projection = self.projection_composition.projection_a;
        let parameters = &mut self.projection_composition.parameters;
        match projection {
            ProjectionKind::AddressRaster => {
                rail_dimensions(ui, &mut parameters.dimensions);
                let width = format!("{} B", parameters.row_width);
                rail_slider_row(
                    ui,
                    "Row width",
                    width,
                    egui::Slider::new(&mut parameters.row_width, 4..=4096).logarithmic(true),
                );
            }
            ProjectionKind::Hilbert => {
                rail_dimensions(ui, &mut parameters.dimensions);
                let order = parameters.curve_order.to_string();
                rail_slider_row(
                    ui,
                    "Curve order",
                    order,
                    egui::Slider::new(&mut parameters.curve_order, 2..=8),
                );
                let aggregation = format!("{} B", parameters.aggregation_bytes);
                rail_slider_row(
                    ui,
                    "Aggregation",
                    aggregation,
                    egui::Slider::new(&mut parameters.aggregation_bytes, 1..=65_536)
                        .logarithmic(true),
                );
            }
            ProjectionKind::Transitions => {
                let lag = format!("{} B", parameters.lag);
                rail_slider_row(
                    ui,
                    "Lag",
                    lag,
                    egui::Slider::new(&mut parameters.lag, 1..=1024).logarithmic(true),
                );
                rail_segmented(
                    ui,
                    &mut parameters.ngram_order,
                    &[(2, "Digram"), (3, "Trigram")],
                );
                ui.weak("Point density is observed transition frequency; no classifier claim.");
            }
            ProjectionKind::Bitplanes => {
                let plane = parameters.bit_plane.to_string();
                rail_slider_row(
                    ui,
                    "Focus plane",
                    plane,
                    egui::Slider::new(&mut parameters.bit_plane, 0..=7),
                );
                let width = format!("{} B", parameters.row_width);
                rail_slider_row(
                    ui,
                    "Row width",
                    width,
                    egui::Slider::new(&mut parameters.row_width, 4..=4096).logarithmic(true),
                );
                rail_segmented(
                    ui,
                    &mut parameters.word_bits,
                    &[(8, "8"), (16, "16"), (32, "32"), (64, "64")],
                );
                ui.weak("All eight planes remain visible; the focus plane is emphasized.");
                rail_segmented(
                    ui,
                    &mut parameters.little_endian,
                    &[(true, "Little endian"), (false, "Big endian")],
                );
            }
            ProjectionKind::Complexity => {
                let window = format!("{} B", parameters.window_bytes);
                rail_slider_row(
                    ui,
                    "Window",
                    window,
                    egui::Slider::new(&mut parameters.window_bytes, 4..=65_536).logarithmic(true),
                );
                let hop = format!("{} B", parameters.hop_bytes);
                rail_slider_row(
                    ui,
                    "Hop",
                    hop,
                    egui::Slider::new(&mut parameters.hop_bytes, 1..=16_384).logarithmic(true),
                );
                ui.monospace("X entropy / Y change / Z unique symbols");
            }
            ProjectionKind::Sections => {
                let aggregation = format!("{} B", parameters.aggregation_bytes);
                rail_slider_row(
                    ui,
                    "Fallback block",
                    aggregation,
                    egui::Slider::new(&mut parameters.aggregation_bytes, 16..=1_048_576)
                        .logarithmic(true),
                );
                ui.weak(
                    "Uses exact detected/parser regions; address blocks are a labelled fallback.",
                );
            }
            ProjectionKind::AlignmentLattice => {
                let stride = format!("{} B", parameters.alignment_stride);
                rail_slider_row(
                    ui,
                    "Candidate stride",
                    stride,
                    egui::Slider::new(&mut parameters.alignment_stride, 1..=4096).logarithmic(true),
                );
                let maximum = format!("{} B", parameters.alignment_max_stride);
                rail_slider_row(
                    ui,
                    "Auto sweep max",
                    maximum,
                    egui::Slider::new(&mut parameters.alignment_max_stride, 2..=4096)
                        .logarithmic(true),
                );
                ui.weak("X residue / Y byte value / Z record index; ranked widths are hypotheses.");
            }
            ProjectionKind::RecurrencePlane | ProjectionKind::RepetitionSkyline => {
                let window = format!("{} B", parameters.recurrence_window);
                rail_slider_row(
                    ui,
                    "Match window",
                    window,
                    egui::Slider::new(&mut parameters.recurrence_window, 4..=4096)
                        .logarithmic(true),
                );
                let search = format!("{} B", parameters.recurrence_search_bytes);
                rail_slider_row(
                    ui,
                    "Prior search",
                    search,
                    egui::Slider::new(
                        &mut parameters.recurrence_search_bytes,
                        4..=16 * 1024 * 1024,
                    )
                    .logarithmic(true),
                );
                let budget = parameters.recurrence_candidate_budget.to_string();
                rail_slider_row(
                    ui,
                    "Candidates",
                    budget,
                    egui::Slider::new(&mut parameters.recurrence_candidate_budget, 1..=4096)
                        .logarithmic(true),
                );
                let threshold = format!("{}%", parameters.recurrence_threshold_percent);
                rail_slider_row(
                    ui,
                    "Min similarity",
                    threshold,
                    egui::Slider::new(&mut parameters.recurrence_threshold_percent, 0..=100),
                );
                ui.weak("Bounded prior-candidate search; retained partners map to exact ranges.");
            }
            ProjectionKind::SpectralWaterfall => {
                let window = format!("{} B", parameters.spectrum_window);
                rail_slider_row(
                    ui,
                    "DFT window",
                    window,
                    egui::Slider::new(&mut parameters.spectrum_window, 8..=4096).logarithmic(true),
                );
                let bins = parameters.spectrum_bins.to_string();
                rail_slider_row(
                    ui,
                    "Frequency bins",
                    bins,
                    egui::Slider::new(&mut parameters.spectrum_bins, 1..=256).logarithmic(true),
                );
                ui.weak("X address / Y dominant non-DC bin / Z normalized magnitude.");
            }
            ProjectionKind::HammingHypercube => {
                ui.monospace("8-bit vectors / fixed 3D basis");
                ui.weak("One-bit changes remain comparable across files; GPU verified when shown below.");
            }
            ProjectionKind::HierarchicalBlockVolume => {
                let depth = parameters.hierarchy_max_depth.to_string();
                rail_slider_row(
                    ui,
                    "Maximum depth",
                    depth,
                    egui::Slider::new(&mut parameters.hierarchy_max_depth, 0..=16),
                );
                let minimum = format!("{} B", parameters.hierarchy_min_block);
                rail_slider_row(
                    ui,
                    "Minimum block",
                    minimum,
                    egui::Slider::new(&mut parameters.hierarchy_min_block, 8..=16 * 1024 * 1024)
                        .logarithmic(true),
                );
                let threshold = format!("{}%", parameters.hierarchy_threshold_percent);
                rail_slider_row(
                    ui,
                    "Split threshold",
                    threshold,
                    egui::Slider::new(&mut parameters.hierarchy_threshold_percent, 0..=100),
                );
                ui.weak("Deterministic mean/printability discontinuities; explicitly heuristic.");
            }
            ProjectionKind::PolarAddressPath | ProjectionKind::HelicalAddressPath => {
                let hop = format!("{} B", parameters.hop_bytes);
                rail_slider_row(
                    ui,
                    "Path hop",
                    hop,
                    egui::Slider::new(&mut parameters.hop_bytes, 1..=4096).logarithmic(true),
                );
                ui.weak("Address-path submode; Path geometry is recommended.");
            }
        }
        if projection == ProjectionKind::AlignmentLattice && !self.alignment_candidates.is_empty() {
            rail_group_label(ui, "RANKED STRIDE HYPOTHESES");
            let candidates = self.alignment_candidates.clone();
            for candidate in candidates.into_iter().take(6) {
                if rail_selectable(
                    ui,
                    parameters.alignment_stride == candidate.stride,
                    format!("{:>4} B   score {:.3}", candidate.stride, candidate.score),
                )
                .clicked()
                {
                    parameters.alignment_stride = candidate.stride;
                    self.projection_sample_key = None;
                }
            }
        }
    }

    pub(super) fn show_projection_lens_controls(&mut self, ui: &mut egui::Ui) {
        rail_group_label(ui, "COLOUR");
        rail_segmented(
            ui,
            &mut self.projection_composition.channels.color,
            &[
                (ProjectionColorFeature::Address, "Address"),
                (ProjectionColorFeature::Entropy, "Entropy"),
                (ProjectionColorFeature::Value, "Value"),
            ],
        );
        rail_group_label(ui, "HEIGHT");
        rail_segmented(
            ui,
            &mut self.projection_composition.channels.height,
            &[
                (ProjectionHeightFeature::None, "None"),
                (ProjectionHeightFeature::Entropy, "Entropy"),
                (ProjectionHeightFeature::ChangeRate, "Change"),
            ],
        );
        ui.add_enabled_ui(
            self.projection_composition.channels.height != ProjectionHeightFeature::None,
            |ui| {
                let relief = format!("{:>3.0}%", self.projection_relief * 100.0);
                rail_slider_row(
                    ui,
                    "Height amount",
                    relief,
                    egui::Slider::new(&mut self.projection_relief, 0.0..=1.0),
                );
            },
        );
        rail_group_label(ui, "SIZE");
        rail_segmented(
            ui,
            &mut self.projection_composition.channels.size,
            &[
                (ProjectionSizeFeature::Uniform, "Uniform"),
                (ProjectionSizeFeature::Entropy, "Entropy"),
                (ProjectionSizeFeature::ChangeRate, "Change"),
            ],
        );
        rail_group_label(ui, "OPACITY");
        rail_segmented(
            ui,
            &mut self.projection_composition.channels.opacity,
            &[
                (ProjectionOpacityFeature::Uniform, "Uniform"),
                (ProjectionOpacityFeature::SelectionContext, "Selection"),
            ],
        );
        ui.add_enabled_ui(
            self.projection_composition.channels.opacity
                == ProjectionOpacityFeature::SelectionContext,
            |ui| {
                let context = format!("{:>3.0}%", self.projection_context_light * 100.0);
                rail_slider_row(
                    ui,
                    "Unselected",
                    context,
                    egui::Slider::new(&mut self.projection_context_light, 0.05..=1.0),
                );
            },
        );
        rail_group_label(ui, "OVERLAYS");
        ui.columns(3, |columns| {
            columns[0].checkbox(
                &mut self.projection_composition.overlays.selection,
                "Selection",
            );
            columns[1].checkbox(&mut self.projection_composition.overlays.regions, "Regions");
            columns[2].checkbox(
                &mut self.projection_composition.overlays.signatures,
                "Signatures",
            );
        });
        ui.weak(
            "Signature outlines preserve voxel colour; strings, symbols, and pointer edges join this layer as analyzers land.",
        );
        if rail_action(ui, "Reset channels") {
            self.projection_composition.channels = ProjectionChannels::default();
            self.projection_composition.overlays = ProjectionOverlays::default();
            self.projection_relief = 1.0;
            self.projection_context_light = 0.72;
        }
    }

    pub(super) fn show_projection_field_controls(&mut self, ui: &mut egui::Ui) {
        rail_group_label(ui, "SURFACE PARAMETERS");
        let field_enabled = self.projection_composition.geometry.uses_field();
        ui.add_enabled_ui(field_enabled, |ui| {
            let radius = format!("{:.0}", self.projection_field_radius);
            rail_slider_row(
                ui,
                "Density reach",
                radius,
                egui::Slider::new(&mut self.projection_field_radius, 8.0..=64.0),
            );
            let gain = format!("{:.2}", self.projection_field_exposure);
            rail_slider_row(
                ui,
                "Density gain",
                gain,
                egui::Slider::new(&mut self.projection_field_exposure, 0.5..=6.0),
            );
            let contours_enabled = self.projection_contour_mode.enabled();
            if rail_selectable(ui, contours_enabled, "Contours").clicked() {
                self.projection_contour_mode = if contours_enabled {
                    ProjectionContourMode::Off
                } else {
                    ProjectionContourMode::Isolines
                };
            }
        });
        if !field_enabled {
            ui.weak("Choose Surface geometry to enable density reach, gain, and contours.");
        }
    }

    pub(super) fn show_projection_render_controls(&mut self, ui: &mut egui::Ui) {
        let points = self.projection_point_budget.to_string();
        rail_slider_row(
            ui,
            "Points",
            points,
            egui::Slider::new(&mut self.projection_point_budget, 512..=40_000).logarithmic(true),
        );
        let point_size = format!("{:.2}", self.projection_point_size);
        rail_slider_row(
            ui,
            "Point size",
            point_size,
            egui::Slider::new(&mut self.projection_point_size, 0.6..=4.5),
        );
        let brightness = format!("{:.2}", self.projection_brightness);
        rail_slider_row(
            ui,
            "Brightness",
            brightness,
            egui::Slider::new(&mut self.projection_brightness, 0.35..=2.5),
        );
        let perspective = format!("{:.2}", self.projection_perspective);
        rail_slider_row(
            ui,
            "Perspective",
            perspective,
            egui::Slider::new(&mut self.projection_perspective, 0.0..=1.0),
        );
    }

    pub(super) fn animation_program(&self) -> AnimationProgram {
        let middle_zoom = (self.projection_zoom * 1.12).clamp(0.2, 5.0);
        let mut composition = self.projection_composition;
        composition.compare_mode = ProjectionCompareMode::Morph;
        AnimationProgram {
            version: 1,
            source: self.source_name().to_owned(),
            source_range: None,
            output: self.video_output_path.trim().to_owned(),
            width: self.video_width,
            height: self.video_height,
            fps: self.video_fps,
            duration_seconds: self.video_duration_seconds,
            composition: Some(composition),
            stride: self.projection_composition.parameters.lag,
            point_budget: self.projection_point_budget,
            point_size: self.projection_point_size,
            brightness: self.projection_brightness,
            perspective: self.projection_perspective,
            background: [0, 0, 0],
            show_guides: true,
            look: AnimationLook {
                primitive: AnimationPrimitive::Voxel,
                vignette: 0.12,
                guide_opacity: 0.22,
                ..AnimationLook::default()
            },
            easing: AnimationEasing::SmoothStep,
            overwrite: self.video_overwrite,
            keyframes: vec![
                AnimationKeyframe {
                    at: 0.0,
                    morph: 0.0,
                    yaw: self.projection_yaw,
                    pitch: self.projection_pitch,
                    zoom: self.projection_zoom,
                    focus_offset: None,
                },
                AnimationKeyframe {
                    at: 0.5,
                    morph: 3.0,
                    yaw: self.projection_yaw + std::f32::consts::PI,
                    pitch: -self.projection_pitch * 0.5,
                    zoom: middle_zoom,
                    focus_offset: None,
                },
                AnimationKeyframe {
                    at: 1.0,
                    morph: 0.0,
                    yaw: self.projection_yaw + std::f32::consts::TAU,
                    pitch: self.projection_pitch,
                    zoom: self.projection_zoom,
                    focus_offset: None,
                },
            ],
        }
    }

    pub(super) fn show_video_export_controls(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.strong("Programmable video");
        ui.small("Deterministic keyframes -> PNG frames -> H.264 MP4 + JSON provenance.");
        ui.add_sized(
            [ui.available_width(), RAIL_CONTROL_HEIGHT],
            egui::TextEdit::singleline(&mut self.video_output_path),
        );
        let duration = format!("{:.1} s", self.video_duration_seconds);
        rail_slider_row(
            ui,
            "Duration",
            duration,
            egui::Slider::new(&mut self.video_duration_seconds, 1.0..=20.0),
        );
        let fps = format!("{} fps", self.video_fps);
        rail_slider_row(
            ui,
            "Frame rate",
            fps,
            egui::Slider::new(&mut self.video_fps, 12..=60),
        );
        ui.columns(2, |columns| {
            columns[0].add_sized(
                [columns[0].available_width(), RAIL_CONTROL_HEIGHT],
                egui::DragValue::new(&mut self.video_width)
                    .range(64..=4_096)
                    .speed(2.0)
                    .prefix("W "),
            );
            columns[1].add_sized(
                [columns[1].available_width(), RAIL_CONTROL_HEIGHT],
                egui::DragValue::new(&mut self.video_height)
                    .range(64..=4_096)
                    .speed(2.0)
                    .prefix("H "),
            );
        });
        ui.checkbox(&mut self.video_overwrite, "Replace existing");

        if self.video_export_receiver.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Rendering and encoding…");
            });
            return;
        }

        ui.columns(2, |columns| {
            if rail_action(&mut columns[0], "Export MP4") {
                self.start_video_export();
            }
            if rail_action(&mut columns[1], "Program JSON") {
                self.write_video_program();
            }
        });
    }

    pub(super) fn start_video_export(&mut self) {
        let program = self.animation_program();
        if let Err(error) = program.validate() {
            self.status = format!("Video program invalid: {error}");
            return;
        }
        let bytes = self.source_bytes().to_vec();
        let (sender, receiver) = std::sync::mpsc::channel();
        let spawn_result = std::thread::Builder::new()
            .name("strata-poc-video-export".to_owned())
            .spawn(move || {
                let result = export_animation(&program, &bytes);
                let _send_result = sender.send(result);
            });
        match spawn_result {
            Ok(_handle) => {
                self.video_export_receiver = Some(receiver);
                "Rendering deterministic video frames…".clone_into(&mut self.status);
            }
            Err(error) => {
                self.status = format!("Cannot start video export: {error}");
            }
        }
    }

    pub(super) fn write_video_program(&mut self) {
        let program = self.animation_program();
        let output = Path::new(&program.output);
        let Some(stem) = output.file_stem().and_then(|stem| stem.to_str()) else {
            "Video output needs a valid file name".clone_into(&mut self.status);
            return;
        };
        let path = output.with_file_name(format!("{stem}.program.json"));
        match save_animation_program(&path, &program, self.video_overwrite) {
            Ok(()) => self.status = format!("Wrote animation program {}", path.display()),
            Err(error) => self.status = format!("Cannot write animation program: {error}"),
        }
    }

    pub(super) fn poll_video_export(&mut self) {
        let result =
            self.video_export_receiver
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => Some(Err(
                        "video export worker disconnected before reporting a result".to_owned(),
                    )),
                });
        let Some(result) = result else {
            return;
        };
        self.video_export_receiver = None;
        self.status = match result {
            Ok(report) => {
                let digest_prefix: String = report.source_sha256.chars().take(12).collect();
                format!(
                    "Exported {} frames to {} (source {digest_prefix}…)",
                    report.frame_count,
                    report.output.display()
                )
            }
            Err(error) => format!("Video export failed: {error}"),
        };
    }
}
