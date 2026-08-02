//! Reusable desktop workbench for Strata's linked binary investigations.
#![forbid(unsafe_code)]

mod app_canvas;
mod app_controls;
mod app_discovery;
mod app_discovery_support;
mod app_image_support;
mod app_inspector;
mod app_projection_canvas;
mod app_projection_controls;
mod app_projection_support;
mod app_projects;
mod app_runtime;
mod app_session_support;
mod app_sessions;
mod app_shell_support;
mod app_signatures;
mod app_sources;
mod app_texture_tiles;
pub(crate) mod cohort;
pub(crate) mod potential;
pub(crate) mod project_models;
pub(crate) mod projection;
pub(crate) mod session_models;
pub(crate) mod video;
pub(crate) mod workbench_models;

#[cfg(test)]
mod app_tests;

// The app modules are cohesive implementation shards around one parent-owned state model.
#[allow(clippy::wildcard_imports)]
use app_discovery_support::*;
#[allow(clippy::wildcard_imports)]
use app_image_support::*;
#[allow(clippy::wildcard_imports)]
use app_projection_support::*;
#[allow(clippy::wildcard_imports)]
use app_session_support::*;
#[allow(clippy::wildcard_imports)]
use app_shell_support::*;
#[allow(clippy::wildcard_imports)]
use app_signatures::*;
use app_texture_tiles::{
    RasterTextureTile, paint_raster_texture_tiles, upload_raster_texture_tiles,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
    time::Duration,
};

use cohort::{
    CohortSelection as ScreenCohortSelection, ProjectedMember, SelectionRect, select_cohort,
};
use eframe::egui;
use potential::{PotentialPoint, PotentialSettings, render_potential_field};
use project_models::{
    LOCAL_PROJECT_SUFFIX, LOCAL_PROJECT_VERSION, LocalProjectFile, ProjectPreferences,
    absolutized_path, default_project_preferences_path, derived_session_path,
    is_local_project_path, load_local_project, load_project_preferences_file,
    normalized_project_path, save_local_project, save_project_preferences_file,
};
use projection::{
    ProjectionChannels, ProjectionColorFeature, ProjectionCompareMode, ProjectionComposition,
    ProjectionDimensions, ProjectionDomain, ProjectionGeometry, ProjectionHeightFeature,
    ProjectionKind, ProjectionOpacityFeature, ProjectionOverlays, ProjectionParameters,
    ProjectionRegionPlacement, ProjectionSample, ProjectionSamplingConfig, ProjectionSizeFeature,
    sample_projection_sample_at_source_offset, sample_projection_samples_in_source,
};
use session_models::{
    POC_WORKSPACE_VERSION, PocWorkspaceSnapshot, StoredBranchStatus, StoredCohort,
    StoredContourMode, StoredFindingDisposition, StoredFindingStatus, StoredProjectionState,
    StoredRange, StoredRenderStyle, StoredResonanceMetric, StoredView, StoredWorkbenchMode,
    StoredXorBranch,
};
use strata_analysis::{
    poc::{
        EntropyBlock, ResonanceMetric, ResonanceScan, byte_histogram, digram_counts,
        selection_resonance,
    },
    production::{
        ProductionRuntimeConfig, ProductionRuntimeEvent, StructureEntropyArtifact,
        StructureEntropyPreset,
    },
    projection_p1::{AlignmentCandidate, P1AnalysisConfig, P1FeatureRequest, analyze_p1_tile},
    signatures::{SignatureCatalog, SignatureMatchEvidence, SignatureScanConfig},
    tiles::{TileKey, TilePlanConfig, TilePrecision},
    workbench::{
        ReversibleTransform, TransformAssessment, TransformEvaluation, WorkbenchConfig,
        WorkbenchEvidence, WorkbenchLead, WorkbenchLeadId, WorkbenchLeadKind, analyze_workbench,
        apply_reversible_transform, catalog_signature_leads, evaluate_transform_candidate,
    },
};
use strata_core::{
    AnalysisRequestId, ByteRange, ByteRangeSet, EvidenceId, Priority, SourceGeneration, SourceId,
};
use strata_gpu::{P1GpuDatum, WgpuP1Backend, run_p1_gpu_self_test};
use strata_runtime::{
    AttachedSource, DigestRuntimeEvent, InvestigationRuntime, RuntimeDigestRequest,
    RuntimeStructureRequest, SourceDigestArtifact, SourceOverviewConfig, TiledDiffArtifact,
    TiledDiffConfig, build_source_overview, build_tiled_diff,
};
use strata_session::{
    Journal, JournalEvent, Reattachment, SessionBundle, SourceFingerprint, WorkspaceSnapshot,
};
use strata_test_support::poc_fixtures::{
    InterleavedSensorFixture, InvestigationFixture, RevisionPairFixture, aligned_revision_pair,
    composite_firmware, interleaved_sensor_image, investigation_binary,
};
use strata_views::dossier::{
    DossierActionKind, DossierContext, DossierLinkState, DossierLinkTarget, InvestigationDossier,
    build_investigation_dossier,
};
use strata_views::investigation::{
    Correlation, CorrelationId, CorrelationStrength, Evidence, ExactProvenance,
    Finding as InvestigationFinding, FindingId, FindingStatus, Hypothesis, HypothesisId,
    HypothesisStatus, InvestigationError, InvestigationModel,
};
use strata_views::poc::{
    BitPlaneConfig, BitPlaneLayout, RasterLayout, RgbaImage, render_bit_plane_stride_atlas,
    render_classified_byte_atlas, render_log_digram_matrix, render_revision_diff_atlas,
};
use strata_views::workbench::{
    BranchId, BranchModel, BranchStatus, CohortFactor, CohortModel, CohortSample,
    ComparisonArchaeology, ComparisonClassification, ComparisonRegionId, LivingRegion, RegionId,
    RegionKind, RegionModel, RegionRelationship, RegionRelationshipId, RegionRelationshipKind,
    SampledByteId, SourceSnapshot, WorkbenchError,
};
use video::{
    AnimationEasing, AnimationKeyframe, AnimationLook, AnimationPrimitive, AnimationProgram,
    VideoExportReport, animation_preset, animation_presets, export_animation,
    load_animation_program, save_animation_program,
};
use workbench_models::{
    build_branch_from_evaluation, build_bytewise_comparison, build_comparison_archaeology,
    build_region_model,
};

const MAX_CONTIGUOUS_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const LARGE_SOURCE_PREVIEW_BYTES: u64 = 1024 * 1024;
const INSPECTOR_SELECTION_PREVIEW_BYTES: usize = 64 * 1024;
const UI_HEADER_BG: egui::Color32 = egui::Color32::from_rgb(238, 241, 243);
const UI_HEADER_TEXT: egui::Color32 = egui::Color32::from_rgb(25, 31, 35);
const UI_SHELL_BG: egui::Color32 = egui::Color32::from_rgb(12, 16, 19);
const UI_RAIL_BG: egui::Color32 = egui::Color32::from_rgb(22, 28, 32);
const UI_RAIL_ALT: egui::Color32 = egui::Color32::from_rgb(27, 34, 39);
const UI_CARD_BG: egui::Color32 = egui::Color32::from_rgb(32, 40, 45);
const UI_CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(2, 5, 7);
const UI_BORDER: egui::Color32 = egui::Color32::from_rgb(55, 65, 71);
const UI_TEXT: egui::Color32 = egui::Color32::from_rgb(223, 229, 232);
const UI_MUTED: egui::Color32 = egui::Color32::from_rgb(145, 156, 163);
const UI_CYAN: egui::Color32 = egui::Color32::from_rgb(41, 166, 225);
const UI_TEAL: egui::Color32 = egui::Color32::from_rgb(74, 190, 168);
const UI_AMBER: egui::Color32 = egui::Color32::from_rgb(235, 179, 66);
const RAIL_CONTROL_HEIGHT: f32 = 28.0;
const RAIL_SEGMENT_GAP: f32 = 4.0;

/// Product identity supplied by a thin native composition root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplicationIdentity {
    /// Native window and help heading.
    pub title: &'static str,
    /// Executable name shown in command-line help.
    pub executable: &'static str,
}

/// Identity retained by the compatibility POC executable.
pub const POC_IDENTITY: ApplicationIdentity = ApplicationIdentity {
    title: "Strata POC",
    executable: "strata-poc",
};

/// Identity used by the promoted macOS application host.
pub const PRODUCTION_IDENTITY: ApplicationIdentity = ApplicationIdentity {
    title: "Strata",
    executable: "strata-app-macos",
};

/// Parses process arguments and runs the workbench or a headless utility mode.
///
/// # Errors
///
/// Returns a user-facing error when arguments, fixtures, GPU initialization,
/// video programs, or the native application runtime fail.
pub fn run(identity: ApplicationIdentity) -> Result<(), String> {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    if first.as_deref() == Some(OsStr::new("--render-program")) {
        let path = arguments
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| "--render-program requires a JSON path".to_owned())?;
        return render_program_file(&path);
    }
    if first.as_deref() == Some(OsStr::new("--validate-program")) {
        let path = arguments
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| "--validate-program requires a JSON path".to_owned())?;
        let program = load_animation_program(&path)?;
        let frame_count = program.validate()?;
        println!("Valid: {} ({frame_count} frames)", path.display());
        return Ok(());
    }
    if first.as_deref() == Some(OsStr::new("--write-example-program")) {
        let path = arguments
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| "--write-example-program requires a JSON path".to_owned())?;
        save_animation_program(&path, &AnimationProgram::example(), false)?;
        println!("Wrote {}", path.display());
        return Ok(());
    }
    if first.as_deref() == Some(OsStr::new("--list-video-presets")) {
        for preset in animation_presets() {
            println!(
                "{}\t{}\n  reveals: {}\n  source: {}",
                preset.id, preset.title, preset.reveals, preset.fixture
            );
        }
        return Ok(());
    }
    if first.as_deref() == Some(OsStr::new("--write-video-preset")) {
        let id = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(|| "--write-video-preset requires a preset id".to_owned())?;
        let path = arguments
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| "--write-video-preset requires an output JSON path".to_owned())?;
        let preset = animation_preset(&id).ok_or_else(|| {
            format!("unknown video preset {id}; run --list-video-presets for valid ids")
        })?;
        save_animation_program(&path, &preset.program, false)?;
        println!("Wrote {} ({})", path.display(), preset.title);
        return Ok(());
    }
    if first.as_deref() == Some(OsStr::new("--gpu-self-test")) {
        let report = run_p1_gpu_self_test().map_err(|error| error.to_string())?;
        println!(
            "GPU verified: {} / {} · {} records · max error {:.8}",
            report.backend,
            report.adapter_name,
            report.compared_records,
            report.maximum_component_error
        );
        return Ok(());
    }
    if first.as_deref() == Some(OsStr::new("--help")) {
        print_help(identity);
        return Ok(());
    }
    run_gui(first.map(PathBuf::from), identity)
}

fn run_gui(initial_path: Option<PathBuf>, identity: ApplicationIdentity) -> Result<(), String> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id(identity.executable)
            .with_inner_size([1_440.0, 900.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        identity.title,
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(StrataPoc::new(
                creation_context,
                initial_path.as_ref(),
            )))
        }),
    )
    .map_err(|error| error.to_string())
}

fn render_program_file(path: &Path) -> Result<(), String> {
    let program = load_animation_program(path)?;
    let bytes = load_program_source(&program)?;
    let report = export_animation(&program, &bytes)?;
    println!(
        "Rendered {} frames to {}\nManifest: {}\nSource SHA-256: {}",
        report.frame_count,
        report.output.display(),
        report.manifest.display(),
        report.source_sha256
    );
    Ok(())
}

fn load_program_source(program: &AnimationProgram) -> Result<Vec<u8>, String> {
    if program.source == "demo://composite-firmware" {
        return composite_firmware()
            .map(|fixture| fixture.bytes)
            .map_err(|error| format!("cannot create demo source: {error}"));
    }
    if program.source == "demo://investigation-binary" {
        return investigation_binary()
            .map(|fixture| fixture.bytes)
            .map_err(|error| format!("cannot create discovery source: {error}"));
    }
    let path = Path::new(&program.source);
    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "cannot inspect animation source {}: {error}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_CONTIGUOUS_SOURCE_BYTES {
        return Err(format!(
            "animation source exceeds the POC {} MiB limit: {}",
            MAX_CONTIGUOUS_SOURCE_BYTES / (1024 * 1024),
            path.display()
        ));
    }
    std::fs::read(path)
        .map_err(|error| format!("cannot read animation source {}: {error}", path.display()))
}

fn print_help(identity: ApplicationIdentity) {
    println!(
        "{}\n\
         \n\
         {} [SOURCE | PROJECT.strata-project | SESSION_DIRECTORY]\n\
         {} --render-program PROGRAM.json\n\
         {} --validate-program PROGRAM.json\n\
         {} --write-example-program PROGRAM.json\n\
         {} --list-video-presets\n\
         {} --write-video-preset PRESET_ID PROGRAM.json\n\
         {} --gpu-self-test",
        identity.title,
        identity.executable,
        identity.executable,
        identity.executable,
        identity.executable,
        identity.executable,
        identity.executable,
        identity.executable
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Discover,
    Projection3d,
    Resonance,
    Structure,
    Grammar,
    Interleave,
    RevisionDiff,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
enum WorkbenchMode {
    #[default]
    Leads,
    Regions,
    Compare,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ProjectionInteraction {
    #[default]
    Rotate,
    SelectCohort,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ProjectionContourMode {
    #[default]
    Off,
    Isolines,
}

impl ProjectionContourMode {
    const fn enabled(self) -> bool {
        matches!(self, Self::Isolines)
    }
}

impl ViewKind {
    const fn title(self) -> &'static str {
        match self {
            Self::Discover => "Investigation map",
            Self::Projection3d => "3D projection lab",
            Self::Resonance => "Selection resonance field",
            Self::Structure => "Structure atlas",
            Self::Grammar => "Transition grammar",
            Self::Interleave => "Record and interleave lab",
            Self::RevisionDiff => "Aligned revision diff",
        }
    }

    const fn note(self) -> &'static str {
        match self {
            Self::Discover => {
                "Ranked leads connect exact byte ranges to correlations, reversible transforms, and evidence."
            }
            Self::Projection3d => {
                "Compose domain, projection, geometry, channels, and A/B comparison without losing byte identity."
            }
            Self::Resonance => {
                "The current selection becomes a live query for echoes at five structural scales."
            }
            Self::Structure => "Byte classes and block entropy expose boundaries before parsing.",
            Self::Grammar => "Exact ordered-byte transitions recalculate for the shared selection.",
            Self::Interleave => "Stride, lane, and bit controls recover fixed-record structure.",
            Self::RevisionDiff => "Same-offset comparison separates unchanged and changed bytes.",
        }
    }
}

struct PocData {
    investigation: InvestigationFixture,
    sensor: InterleavedSensorFixture,
    revisions: RevisionPairFixture,
}

struct LoadedSource {
    display_name: String,
    path: PathBuf,
    bytes: Vec<u8>,
    source: AttachedSource,
    source_length: u64,
    tile_overview_level: u8,
    resident_tiles: Vec<ResidentSourceTile>,
    resident_bytes: u64,
    sampled_overview: bool,
}

struct ResidentSourceTile {
    key: TileKey,
    coverage: ByteRange,
    read_range: ByteRange,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileLoadSlot {
    Primary,
    Comparison,
    SessionReattachment,
    FocusRefinement,
}

struct FileLoadMessage {
    request_id: u64,
    slot: FileLoadSlot,
    result: Result<FileLoadOutcome, String>,
}

struct FileLoadOutcome {
    loaded: LoadedSource,
    tiled_diff: Option<Arc<TiledDiffArtifact>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceDigestPurpose {
    ActiveSource,
    SessionCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingSourceDigest {
    request_id: AnalysisRequestId,
    purpose: SourceDigestPurpose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderKey {
    view: ViewKind,
    generation: u64,
    selection: Range<usize>,
    width: usize,
    stride: usize,
    lane: usize,
    bit: u8,
    max_texture_side: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectionSampleKey {
    generation: u64,
    sampling: ProjectionSamplingConfig,
    parameters: ProjectionParameters,
    projection_a: ProjectionKind,
    projection_b: ProjectionKind,
    point_budget: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ProjectionFieldKey {
    sample: ProjectionSampleKey,
    composition: ProjectionComposition,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    perspective: f32,
    brightness: f32,
    relief: f32,
    field_radius: f32,
    field_exposure: f32,
    contour_mode: ProjectionContourMode,
    canvas_size: egui::Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResonanceKey {
    generation: u64,
    probe_offset: usize,
    base_window: usize,
    stride: usize,
    sample_budget: usize,
    metric: ResonanceMetric,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DossierKey {
    source_id: SourceId,
    generation: SourceGeneration,
    ranges: Vec<(u64, u64)>,
    epoch: u64,
    structure_artifact_digest: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct SelectedResonance {
    probe_offset: u64,
    candidate_offset: u64,
    window_size: u64,
    score: f64,
    metric: ResonanceMetric,
}

#[derive(Debug, Clone, Copy)]
enum ActiveMapping {
    Raster(RasterLayout),
    BitPlane(BitPlaneLayout),
    Digram,
}

#[allow(clippy::struct_excessive_bools)]
struct StrataPoc {
    data: Option<PocData>,
    initialization_error: Option<String>,
    loaded_source: Option<LoadedSource>,
    comparison_source: Option<LoadedSource>,
    comparison_artifact: Option<Arc<TiledDiffArtifact>>,
    pending_session_source: Option<LoadedSource>,
    comparison_path_input: String,
    comparison_status: String,
    file_load_sender: Sender<FileLoadMessage>,
    file_load_receiver: Receiver<FileLoadMessage>,
    next_file_load_request: u64,
    primary_file_load: Option<u64>,
    comparison_file_load: Option<u64>,
    session_file_load: Option<u64>,
    focus_file_load: Option<u64>,
    analysis_source: Option<AttachedSource>,
    analysis_runtime: Option<InvestigationRuntime>,
    source_digest_request: Option<PendingSourceDigest>,
    next_digest_request: u128,
    structure_artifact: Option<Arc<StructureEntropyArtifact>>,
    structure_request: Option<AnalysisRequestId>,
    next_analysis_request: u128,
    structure_status: String,
    active_view: ViewKind,
    selection: Range<usize>,
    drag_anchor: Option<usize>,
    selected_digram: Option<(u8, u8)>,
    selected_projection: Option<[usize; 3]>,
    path_input: String,
    status: String,
    source_generation: u64,
    discovery_findings: Vec<WorkbenchLead>,
    discovery_selected: Option<WorkbenchLeadId>,
    discovery_generation: Option<u64>,
    discovery_preview_transform: bool,
    discovery_error: Option<String>,
    signature_catalog: Option<Arc<SignatureCatalog>>,
    signature_pack_path_input: String,
    signature_pack_status: String,
    signature_scan_status: String,
    project_path_input: String,
    reopen_last_project: bool,
    show_project_preferences: bool,
    pending_project_save: Option<PathBuf>,
    project_preferences_path: PathBuf,
    investigation: InvestigationModel,
    workbench_mode: WorkbenchMode,
    regions: RegionModel,
    selected_region: Option<RegionId>,
    comparison: Option<ComparisonArchaeology>,
    selected_comparison: Option<ComparisonRegionId>,
    branches: BranchModel,
    selected_branch: Option<BranchId>,
    branch_key: u8,
    branch_assessments: BTreeMap<BranchId, TransformAssessment>,
    session_path_input: String,
    session_bundle: Option<SessionBundle>,
    session_journal: Journal,
    session_attached: bool,
    restored_session_selection: Vec<Range<usize>>,
    atlas_width: usize,
    digram_stride: usize,
    interleave_width: usize,
    interleave_stride: usize,
    interleave_lane: usize,
    bit_plane: u8,
    diff_width: usize,
    projection_point_budget: usize,
    projection_composition: ProjectionComposition,
    projection_relief: f32,
    projection_context_light: f32,
    projection_point_size: f32,
    projection_brightness: f32,
    projection_perspective: f32,
    projection_field_radius: f32,
    projection_field_exposure: f32,
    projection_contour_mode: ProjectionContourMode,
    projection_yaw: f32,
    projection_pitch: f32,
    projection_zoom: f32,
    projection_spin: bool,
    projection_auto_morph: bool,
    projection_speed: f32,
    projection_phase: f32,
    projection_interaction: ProjectionInteraction,
    projection_cohort_anchor: Option<egui::Pos2>,
    projection_cohort_cursor: Option<egui::Pos2>,
    projection_cohort_selection: Option<ScreenCohortSelection>,
    analytical_cohort: CohortModel,
    projection_samples: Vec<ProjectionSample>,
    alignment_candidates: Vec<AlignmentCandidate>,
    gpu_backend: Option<WgpuP1Backend>,
    gpu_status: String,
    projection_sample_key: Option<ProjectionSampleKey>,
    projection_field_texture: Option<egui::TextureHandle>,
    projection_field_key: Option<ProjectionFieldKey>,
    resonance_metric: ResonanceMetric,
    resonance_base_window: usize,
    resonance_stride: usize,
    resonance_sample_budget: usize,
    resonance_layers: Vec<ResonanceScan>,
    resonance_key: Option<ResonanceKey>,
    selected_resonance: Option<SelectedResonance>,
    dossier: Option<InvestigationDossier>,
    dossier_key: Option<DossierKey>,
    dossier_error: Option<String>,
    dossier_epoch: u64,
    video_output_path: String,
    video_duration_seconds: f32,
    video_fps: u32,
    video_width: u32,
    video_height: u32,
    video_overwrite: bool,
    video_export_receiver: Option<Receiver<Result<VideoExportReport, String>>>,
    entropy: Vec<EntropyBlock>,
    texture_tiles: Vec<RasterTextureTile>,
    texture_key: Option<RenderKey>,
    texture_dimensions: [usize; 2],
    active_mapping: Option<ActiveMapping>,
    render_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionSlot {
    A,
    B,
}

impl ProjectionSlot {
    const fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ScreenProjection {
    position: egui::Pos2,
    depth: f32,
    radius: f32,
    color: egui::Color32,
    point_id: u64,
    source_offsets: [usize; 3],
    analysis_range: [usize; 2],
    slot: ProjectionSlot,
    bit_plane: Option<u8>,
    region_slot: Option<usize>,
    p1: Option<strata_analysis::projection_p1::P1FeatureRecord>,
}

#[derive(Debug, Clone, Copy)]
struct ResonanceScreenPoint {
    position: egui::Pos2,
    probe_offset: u64,
    candidate_offset: u64,
    window_size: u64,
    score: f64,
    metric: ResonanceMetric,
}

#[derive(Debug, Clone, Copy)]
struct ProjectionRenderSettings {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    perspective: f32,
    point_size: f32,
    brightness: f32,
    relief: f32,
}

#[derive(Debug, Clone, Copy)]
struct ProjectionLabelState {
    point_count: usize,
    composition: ProjectionComposition,
    relief: f32,
    context_light: f32,
    field_radius: f32,
    field_exposure: f32,
    field_contours: bool,
}

impl eframe::App for StrataPoc {
    fn on_exit(&mut self) {
        let _ = self.persist_project_preferences();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.prepare_frame(ui);
        let compact_shell = ui.available_width() < 1_180.0;
        let control_width = if compact_shell { 300.0 } else { 350.0 };
        let (inspector_default, inspector_minimum, inspector_maximum) = if compact_shell {
            (280.0, 240.0, 300.0)
        } else {
            (330.0, 280.0, 380.0)
        };
        egui::Panel::top("poc_header")
            .exact_size(47.0)
            .frame(
                egui::Frame::new()
                    .fill(UI_HEADER_BG)
                    .inner_margin(egui::Margin::symmetric(9, 6))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(190, 196, 200),
                    )),
            )
            .show(ui, |ui| self.show_header(ui));
        egui::Panel::bottom("poc_status")
            .exact_size(28.0)
            .frame(rail_frame(UI_RAIL_BG, 6))
            .show(ui, |ui| {
                let available = ui.available_width();
                let gpu_width = 116.0;
                let metadata_width = (available * 0.32).clamp(190.0, 330.0);
                let status_width = (available - gpu_width - metadata_width - 28.0).max(72.0);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [status_width, 18.0],
                        egui::Label::new(
                            egui::RichText::new(&self.status).size(10.5).color(UI_MUTED),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&self.status);
                    ui.separator();
                    ui.add_sized(
                        [metadata_width, 18.0],
                        egui::Label::new(
                            egui::RichText::new(
                                "read-only  /  deterministic fixtures  /  no telemetry",
                            )
                            .monospace()
                            .size(10.0)
                            .color(UI_TEXT),
                        )
                        .truncate(),
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(gpu_width, 18.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new("●  WGPU / METAL")
                                    .monospace()
                                    .size(10.0)
                                    .color(UI_TEAL),
                            );
                        },
                    );
                });
            });
        egui::Panel::left("poc_controls")
            .exact_size(control_width)
            .frame(rail_frame(UI_RAIL_BG, 9))
            .show(ui, |ui| self.show_control_deck(ui));
        egui::Panel::right("poc_inspector")
            .default_size(inspector_default)
            .min_size(inspector_minimum)
            .max_size(inspector_maximum)
            .frame(rail_frame(UI_RAIL_BG, 10))
            .show(ui, |ui| self.show_inspector(ui));
        egui::CentralPanel::default()
            .frame(rail_frame(UI_CANVAS_BG, 10))
            .show(ui, |ui| self.show_central(ui));
        self.show_drop_overlay(ui.ctx());
        self.show_project_preferences_window(ui.ctx());
        if self.has_active_file_load() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
}
