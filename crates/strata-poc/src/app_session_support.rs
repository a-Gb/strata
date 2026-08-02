//! Stateless session conversion, validation, and local-source helpers.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]
// Split inherent implementations intentionally share the parent-owned app state.

use super::*;

pub(super) fn stored_range_from_usize(range: &Range<usize>) -> Result<StoredRange, String> {
    Ok(StoredRange {
        start: u64::try_from(range.start)
            .map_err(|_| "range start cannot fit the session contract".to_owned())?,
        end: u64::try_from(range.end)
            .map_err(|_| "range end cannot fit the session contract".to_owned())?,
    })
}

pub(super) fn range_from_stored(range: &StoredRange) -> Option<Range<usize>> {
    Some(usize::try_from(range.start).ok()?..usize::try_from(range.end).ok()?)
}

pub(super) fn byte_range_from_stored(range: StoredRange) -> Result<ByteRange, String> {
    ByteRange::new(range.start, range.end).map_err(|error| error.to_string())
}

pub(super) const fn split_u128(value: u128) -> [u64; 2] {
    let bytes = value.to_be_bytes();
    [
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        u64::from_be_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
    ]
}

pub(super) const fn join_u128(value: [u64; 2]) -> u128 {
    (value[0] as u128) << 64 | value[1] as u128
}

pub(super) fn stored_cohort_from_selection(
    selection: &ScreenCohortSelection,
) -> Result<StoredCohort, String> {
    let members = selection
        .members
        .iter()
        .map(|member| {
            Ok([
                u64::try_from(member.source_offsets[0])
                    .map_err(|_| "cohort offset cannot fit the session contract".to_owned())?,
                u64::try_from(member.source_offsets[1])
                    .map_err(|_| "cohort offset cannot fit the session contract".to_owned())?,
                u64::try_from(member.source_offsets[2])
                    .map_err(|_| "cohort offset cannot fit the session contract".to_owned())?,
            ])
        })
        .collect::<Result<Vec<_>, String>>()?;
    let exact_ranges = selection
        .source_ranges
        .iter()
        .map(stored_range_from_usize)
        .collect::<Result<Vec<_>, _>>()?;
    let member_ranges = selection
        .members
        .iter()
        .map(|member| stored_range_from_usize(&(member.source_range[0]..member.source_range[1])))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StoredCohort {
        members,
        member_ranges,
        exact_ranges,
    })
}

pub(super) fn stored_xor_branch(
    branch: &strata_views::workbench::HypothesisBranch,
) -> Result<StoredXorBranch, String> {
    let [range] = branch.provenance.ranges.ranges.as_slice() else {
        return Err("POC XOR branches must retain exactly one source range".to_owned());
    };
    let [node] = branch.transform.nodes.as_slice() else {
        return Err("POC XOR branches must retain exactly one transform node".to_owned());
    };
    if node.kind != "xor-byte" {
        return Err("the POC session only persists explicit XOR branches".to_owned());
    }
    let parameters: serde_json::Value = serde_json::from_str(&node.parameter_json)
        .map_err(|error| format!("invalid XOR branch parameters: {error}"))?;
    let object = parameters
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(|| "XOR branch parameters are not an exact key object".to_owned())?;
    let key = object
        .get("key")
        .and_then(serde_json::Value::as_u64)
        .and_then(|key| u8::try_from(key).ok())
        .ok_or_else(|| "XOR branch key is outside the byte domain".to_owned())?;
    Ok(StoredXorBranch {
        id: split_u128(branch.id.0),
        label: branch.label.clone(),
        range: StoredRange {
            start: range.start,
            end: range.end,
        },
        key,
        status: stored_branch_status(branch.status),
    })
}

pub(super) fn validate_bundle_workspace_event(bundle: &SessionBundle) -> Result<(), String> {
    for entry in bundle.journal().entries() {
        let JournalEvent::WorkspaceChanged(workspace) = &entry.event else {
            return Err(format!(
                "POC session event #{} is outside the source-free workspace contract",
                entry.sequence
            ));
        };
        let snapshot: PocWorkspaceSnapshot = serde_json::from_value(workspace.value().clone())
            .map_err(|error| {
                format!(
                    "invalid source-free workspace event #{}: {error}",
                    entry.sequence
                )
            })?;
        snapshot.validate(bundle.manifest().source().byte_length())?;
    }
    let last_workspace = bundle
        .journal()
        .entries()
        .iter()
        .rev()
        .find_map(|entry| {
            if let JournalEvent::WorkspaceChanged(snapshot) = &entry.event {
                Some(snapshot)
            } else {
                None
            }
        })
        .ok_or_else(|| "session journal has no workspace checkpoint".to_owned())?;
    if last_workspace != bundle.manifest().workspace() {
        return Err("session manifest does not match its final workspace event".to_owned());
    }
    Ok(())
}

pub(super) fn poc_workspace_equivalent(
    first: &WorkspaceSnapshot,
    second: &WorkspaceSnapshot,
) -> bool {
    let first = serde_json::from_value::<PocWorkspaceSnapshot>(first.value().clone());
    let second = serde_json::from_value::<PocWorkspaceSnapshot>(second.value().clone());
    matches!((first, second), (Ok(first), Ok(second)) if first == second)
}

pub(super) fn open_local_source(
    path: &Path,
    source_id: SourceId,
    generation: SourceGeneration,
) -> Result<LoadedSource, String> {
    open_local_source_with_focus(path, source_id, generation, None)
}

pub(super) fn open_local_source_with_focus(
    path: &Path,
    source_id: SourceId,
    generation: SourceGeneration,
    focus: Option<ByteRange>,
) -> Result<LoadedSource, String> {
    let source = AttachedSource::open_local(path, source_id, generation)
        .map_err(|error| format!("cannot open candidate {}: {error}", path.display()))?;
    let descriptor = source.descriptor();
    let length = descriptor
        .length
        .ok_or_else(|| "local source did not report a length".to_owned())?;
    let preview_length = if length <= MAX_CONTIGUOUS_SOURCE_BYTES {
        length
    } else {
        length.min(LARGE_SOURCE_PREVIEW_BYTES)
    };
    let plan_config = TilePlanConfig {
        focus,
        ..TilePlanConfig::default()
    };
    let overview = build_source_overview(
        source,
        SourceOverviewConfig {
            preview_bytes: preview_length.max(1),
            tile_plan: plan_config,
        },
        b"strata-poc/p1-default",
    )
    .map_err(|error| {
        format!(
            "cannot build candidate overview {}: {error}",
            path.display()
        )
    })?;
    let source = overview.source;
    let bytes = overview.preview_bytes;
    let plan = overview.plan;
    let sampled_overview = plan.is_sampled();
    let tile_overview_level = plan.overview_level;
    let resident_bytes = plan.resident_bytes;
    let resident_tiles = plan
        .tiles
        .into_iter()
        .zip(overview.resident_tiles)
        .map(|(planned, resident)| ResidentSourceTile {
            key: planned.key,
            coverage: resident.coverage,
            read_range: resident.read_range,
            bytes: resident.bytes,
        })
        .collect();
    Ok(LoadedSource {
        display_name: descriptor.display_name,
        path: path.to_path_buf(),
        bytes,
        source,
        source_length: length,
        tile_overview_level,
        resident_tiles,
        resident_bytes,
        sampled_overview,
    })
}

pub(super) fn retained_source(
    bytes: &[u8],
    generation: SourceGeneration,
    display_name: &str,
) -> Result<AttachedSource, strata_core::DomainError> {
    AttachedSource::retained(
        SourceId(1),
        generation,
        display_name,
        Arc::<[u8]>::from(bytes.to_vec()),
    )
}

pub(super) fn record_initialization_error(slot: &mut Option<String>, error: String) {
    if let Some(existing) = slot {
        existing.push_str("; ");
        existing.push_str(&error);
    } else {
        *slot = Some(error);
    }
}

pub(super) fn digest_prefix(digest: &str) -> &str {
    digest.get(..12).unwrap_or(digest)
}

pub(super) const fn journal_event_label(event: &JournalEvent) -> &'static str {
    match event {
        JournalEvent::WorkspaceChanged(_) => "workspace checkpoint",
        JournalEvent::ViewChanged(_) => "view changed",
        JournalEvent::SelectionChanged(_) => "selection changed",
        JournalEvent::HypothesisApplied(_) => "hypothesis applied",
        JournalEvent::AnnotationAdded(_) => "annotation added",
    }
}

pub(super) const fn stored_view(view: ViewKind) -> StoredView {
    match view {
        ViewKind::Discover => StoredView::Discover,
        ViewKind::Projection3d => StoredView::Projection3d,
        ViewKind::Resonance => StoredView::Resonance,
        ViewKind::Structure => StoredView::Structure,
        ViewKind::Grammar => StoredView::Grammar,
        ViewKind::Interleave => StoredView::Interleave,
        ViewKind::RevisionDiff => StoredView::RevisionDiff,
    }
}

pub(super) const fn view_from_stored(view: StoredView) -> ViewKind {
    match view {
        StoredView::Discover => ViewKind::Discover,
        StoredView::Projection3d => ViewKind::Projection3d,
        StoredView::Resonance => ViewKind::Resonance,
        StoredView::Structure => ViewKind::Structure,
        StoredView::Grammar => ViewKind::Grammar,
        StoredView::Interleave => ViewKind::Interleave,
        StoredView::RevisionDiff => ViewKind::RevisionDiff,
    }
}

pub(super) const fn stored_workbench_mode(mode: WorkbenchMode) -> StoredWorkbenchMode {
    match mode {
        WorkbenchMode::Leads => StoredWorkbenchMode::Leads,
        WorkbenchMode::Regions => StoredWorkbenchMode::Regions,
        WorkbenchMode::Compare => StoredWorkbenchMode::Compare,
    }
}

pub(super) const fn workbench_mode_from_stored(mode: StoredWorkbenchMode) -> WorkbenchMode {
    match mode {
        StoredWorkbenchMode::Leads => WorkbenchMode::Leads,
        StoredWorkbenchMode::Regions => WorkbenchMode::Regions,
        StoredWorkbenchMode::Compare => WorkbenchMode::Compare,
    }
}

pub(super) const fn stored_render_style(geometry: ProjectionGeometry) -> StoredRenderStyle {
    match geometry {
        ProjectionGeometry::Surface => StoredRenderStyle::Density,
        ProjectionGeometry::Points | ProjectionGeometry::Path | ProjectionGeometry::Voxels => {
            StoredRenderStyle::Voxels
        }
    }
}

pub(super) const fn geometry_from_stored(style: StoredRenderStyle) -> ProjectionGeometry {
    match style {
        StoredRenderStyle::Voxels => ProjectionGeometry::Voxels,
        StoredRenderStyle::Density | StoredRenderStyle::Combined => ProjectionGeometry::Surface,
    }
}

pub(super) fn legacy_projection_composition(
    morph: f32,
    render_style: StoredRenderStyle,
    stride: usize,
    color_mix: f32,
) -> ProjectionComposition {
    let projection_a = if morph < 0.5 {
        ProjectionKind::Transitions
    } else if morph < 1.5 {
        ProjectionKind::PolarAddressPath
    } else if morph < 2.5 {
        ProjectionKind::HelicalAddressPath
    } else {
        ProjectionKind::Hilbert
    };
    let mut composition = ProjectionComposition {
        projection_a,
        projection_b: projection_a,
        geometry: geometry_from_stored(render_style),
        compare_mode: ProjectionCompareMode::Single,
        ..ProjectionComposition::default()
    };
    composition.parameters.lag = stride.clamp(1, 1024);
    composition.channels.color = if color_mix >= 0.5 {
        ProjectionColorFeature::Entropy
    } else {
        ProjectionColorFeature::Address
    };
    composition
}

pub(super) const fn legacy_color_mix(feature: ProjectionColorFeature) -> f32 {
    match feature {
        ProjectionColorFeature::Address => 0.0,
        ProjectionColorFeature::Entropy => 1.0,
        ProjectionColorFeature::Value => 0.5,
    }
}

pub(super) const fn legacy_morph_for_projection(projection: ProjectionKind) -> f32 {
    match projection {
        ProjectionKind::Transitions => 0.0,
        ProjectionKind::PolarAddressPath => 1.0,
        ProjectionKind::HelicalAddressPath => 2.0,
        ProjectionKind::AddressRaster
        | ProjectionKind::Hilbert
        | ProjectionKind::Bitplanes
        | ProjectionKind::Complexity
        | ProjectionKind::Sections
        | ProjectionKind::AlignmentLattice
        | ProjectionKind::RecurrencePlane
        | ProjectionKind::RepetitionSkyline
        | ProjectionKind::SpectralWaterfall
        | ProjectionKind::HammingHypercube
        | ProjectionKind::HierarchicalBlockVolume => 3.0,
    }
}

pub(super) const fn stored_contour_mode(mode: ProjectionContourMode) -> StoredContourMode {
    match mode {
        ProjectionContourMode::Off => StoredContourMode::Off,
        ProjectionContourMode::Isolines => StoredContourMode::Isolines,
    }
}

pub(super) const fn contour_mode_from_stored(mode: StoredContourMode) -> ProjectionContourMode {
    match mode {
        StoredContourMode::Off => ProjectionContourMode::Off,
        StoredContourMode::Isolines => ProjectionContourMode::Isolines,
    }
}

pub(super) const fn stored_resonance_metric(metric: ResonanceMetric) -> StoredResonanceMetric {
    match metric {
        ResonanceMetric::ExactBytes => StoredResonanceMetric::ExactBytes,
        ResonanceMetric::ByteShape => StoredResonanceMetric::ByteShape,
        ResonanceMetric::Texture => StoredResonanceMetric::Texture,
    }
}

pub(super) const fn resonance_metric_from_stored(metric: StoredResonanceMetric) -> ResonanceMetric {
    match metric {
        StoredResonanceMetric::ExactBytes => ResonanceMetric::ExactBytes,
        StoredResonanceMetric::ByteShape => ResonanceMetric::ByteShape,
        StoredResonanceMetric::Texture => ResonanceMetric::Texture,
    }
}

pub(super) const fn stored_branch_status(status: BranchStatus) -> StoredBranchStatus {
    match status {
        BranchStatus::Draft => StoredBranchStatus::Draft,
        BranchStatus::Active => StoredBranchStatus::Active,
        BranchStatus::Pinned => StoredBranchStatus::Pinned,
        BranchStatus::Discarded => StoredBranchStatus::Discarded,
    }
}

pub(super) const fn branch_status_from_stored(status: StoredBranchStatus) -> BranchStatus {
    match status {
        StoredBranchStatus::Draft => BranchStatus::Draft,
        StoredBranchStatus::Active => BranchStatus::Active,
        StoredBranchStatus::Pinned => BranchStatus::Pinned,
        StoredBranchStatus::Discarded => BranchStatus::Discarded,
    }
}
