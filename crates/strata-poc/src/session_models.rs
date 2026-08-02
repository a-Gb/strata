//! Source-free, serializable POC workspace intent.
#![allow(clippy::redundant_pub_crate)] // Parent-only helpers live in a separate binary module.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::projection::ProjectionComposition;

pub(crate) const POC_WORKSPACE_VERSION: u32 = 1;
const MAX_STORED_RANGES: usize = 4_096;
const MAX_STORED_COHORT_MEMBERS: usize = 4_096;
const MAX_STORED_BRANCHES: usize = 256;
const MAX_STORED_DISPOSITIONS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredView {
    Discover,
    Projection3d,
    Resonance,
    Structure,
    Grammar,
    Interleave,
    RevisionDiff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredWorkbenchMode {
    Leads,
    Regions,
    Compare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredFindingStatus {
    Promoted,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredBranchStatus {
    Draft,
    Active,
    Pinned,
    Discarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredRenderStyle {
    Voxels,
    Density,
    Combined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredContourMode {
    Off,
    Isolines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoredResonanceMetric {
    ExactBytes,
    ByteShape,
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredRange {
    pub(crate) start: u64,
    pub(crate) end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredFindingDisposition {
    pub(crate) lead_id: u64,
    pub(crate) status: StoredFindingStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredXorBranch {
    pub(crate) id: [u64; 2],
    pub(crate) label: String,
    pub(crate) range: StoredRange,
    pub(crate) key: u8,
    pub(crate) status: StoredBranchStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredCohort {
    pub(crate) members: Vec<[u64; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) member_ranges: Vec<StoredRange>,
    pub(crate) exact_ranges: Vec<StoredRange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredProjectionState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) composition: Option<ProjectionComposition>,
    pub(crate) stride: usize,
    pub(crate) point_budget: usize,
    pub(crate) morph: f32,
    pub(crate) color_mix: f32,
    pub(crate) relief: f32,
    pub(crate) context_light: f32,
    pub(crate) point_size: f32,
    pub(crate) brightness: f32,
    pub(crate) perspective: f32,
    pub(crate) render_style: StoredRenderStyle,
    pub(crate) field_radius: f32,
    pub(crate) field_exposure: f32,
    pub(crate) contour_mode: StoredContourMode,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) zoom: f32,
    pub(crate) spin: bool,
    pub(crate) auto_morph: bool,
    pub(crate) speed: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PocWorkspaceSnapshot {
    pub(crate) version: u32,
    pub(crate) source_generation: u64,
    pub(crate) active_view: StoredView,
    pub(crate) workbench_mode: StoredWorkbenchMode,
    pub(crate) exact_selection: Vec<StoredRange>,
    pub(crate) selected_lead: Option<u64>,
    #[serde(
        default,
        serialize_with = "serialize_optional_u128",
        deserialize_with = "deserialize_optional_u128"
    )]
    pub(crate) selected_region: Option<u128>,
    #[serde(
        default,
        serialize_with = "serialize_optional_u128",
        deserialize_with = "deserialize_optional_u128"
    )]
    pub(crate) selected_comparison: Option<u128>,
    pub(crate) finding_dispositions: Vec<StoredFindingDisposition>,
    pub(crate) branches: Vec<StoredXorBranch>,
    pub(crate) selected_branch: Option<[u64; 2]>,
    pub(crate) branch_key: u8,
    pub(crate) cohort: Option<StoredCohort>,
    pub(crate) atlas_width: usize,
    pub(crate) digram_stride: usize,
    pub(crate) interleave_width: usize,
    pub(crate) interleave_stride: usize,
    pub(crate) interleave_lane: usize,
    pub(crate) bit_plane: u8,
    pub(crate) diff_width: usize,
    pub(crate) resonance_metric: StoredResonanceMetric,
    pub(crate) resonance_base_window: usize,
    pub(crate) resonance_stride: usize,
    pub(crate) resonance_sample_budget: usize,
    pub(crate) projection: StoredProjectionState,
}

impl PocWorkspaceSnapshot {
    pub(crate) fn validate(&self, source_length: u64) -> Result<(), String> {
        if self.version != POC_WORKSPACE_VERSION {
            return Err(format!(
                "unsupported POC workspace version {}",
                self.version
            ));
        }
        validate_ranges(&self.exact_selection, source_length, "selection")?;
        if self.finding_dispositions.len() > MAX_STORED_DISPOSITIONS {
            return Err("too many stored finding dispositions".to_owned());
        }
        let mut lead_ids = BTreeSet::new();
        for disposition in &self.finding_dispositions {
            if !lead_ids.insert(disposition.lead_id) {
                return Err("duplicate stored finding disposition".to_owned());
            }
        }
        if self.branches.len() > MAX_STORED_BRANCHES {
            return Err("too many stored branches".to_owned());
        }
        let mut branch_ids = BTreeSet::new();
        for branch in &self.branches {
            validate_ranges(&[branch.range], source_length, "branch")?;
            if branch.label.trim().is_empty()
                || branch.label.len() > 256
                || branch.label.contains(['/', '\\'])
                || branch.label.contains("://")
            {
                return Err("stored branch label is empty, oversized, or locator-like".to_owned());
            }
            if !branch_ids.insert(branch.id) {
                return Err("duplicate stored branch".to_owned());
            }
        }
        if let Some(selected) = self.selected_branch
            && !branch_ids.contains(&selected)
        {
            return Err("selected branch is absent from stored branches".to_owned());
        }
        if let Some(cohort) = &self.cohort {
            validate_cohort(cohort, source_length)?;
            if cohort.exact_ranges != self.exact_selection {
                return Err("cohort ranges must equal the exact session selection".to_owned());
            }
        }
        validate_view_controls(self)?;
        Ok(())
    }
}

#[allow(clippy::ref_option)] // serde's `serialize_with` contract passes the field by reference.
fn serialize_optional_u128<S>(value: &Option<u128>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(&format!("0x{value:032x}")),
        None => serializer.serialize_none(),
    }
}

fn deserialize_optional_u128<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StoredU128 {
        Legacy(u64),
        Hex(String),
    }

    let stored = Option::<StoredU128>::deserialize(deserializer)?;
    stored
        .map(|stored| match stored {
            StoredU128::Legacy(value) => Ok(u128::from(value)),
            StoredU128::Hex(value) => value
                .strip_prefix("0x")
                .ok_or_else(|| "stored 128-bit ID is missing its 0x prefix".to_owned())
                .and_then(|hex| {
                    u128::from_str_radix(hex, 16)
                        .map_err(|error| format!("invalid stored 128-bit ID: {error}"))
                }),
        })
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn validate_view_controls(snapshot: &PocWorkspaceSnapshot) -> Result<(), String> {
    if snapshot.atlas_width == 0
        || snapshot.digram_stride == 0
        || snapshot.interleave_width == 0
        || snapshot.interleave_stride == 0
        || snapshot.interleave_lane >= snapshot.interleave_stride
        || snapshot.bit_plane > 7
        || snapshot.diff_width == 0
        || snapshot.resonance_base_window == 0
        || snapshot.resonance_stride == 0
        || snapshot.resonance_sample_budget == 0
    {
        return Err("stored view controls are outside their exact domains".to_owned());
    }
    let projection = &snapshot.projection;
    if let Some(composition) = projection.composition {
        composition.validate().map_err(str::to_owned)?;
    }
    if projection.stride == 0
        || !(3..=250_000).contains(&projection.point_budget)
        || !bounded(projection.morph, 0.0, 3.0)
        || !bounded(projection.color_mix, 0.0, 1.0)
        || !bounded(projection.relief, 0.0, 1.0)
        || !bounded(projection.context_light, 0.0, 1.0)
        || !bounded(projection.point_size, 0.25, 8.0)
        || !bounded(projection.brightness, 0.1, 8.0)
        || !bounded(projection.perspective, 0.0, 1.0)
        || !bounded(projection.field_radius, 1.0, 128.0)
        || !bounded(projection.field_exposure, 0.1, 8.0)
        || !projection.yaw.is_finite()
        || !projection.pitch.is_finite()
        || !bounded(projection.zoom, 0.05, 20.0)
        || !bounded(projection.speed, 0.01, 4.0)
    {
        return Err("stored projection controls are outside their bounded domains".to_owned());
    }
    Ok(())
}

fn bounded(value: f32, minimum: f32, maximum: f32) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

fn validate_cohort(cohort: &StoredCohort, source_length: u64) -> Result<(), String> {
    if cohort.members.is_empty() || cohort.members.len() > MAX_STORED_COHORT_MEMBERS {
        return Err("stored cohort membership is empty or exceeds its bound".to_owned());
    }
    let mut members = BTreeSet::new();
    let mut offsets = BTreeSet::new();
    for member in &cohort.members {
        if !members.insert(*member) {
            return Err("stored cohort repeats a sampled identity".to_owned());
        }
        for offset in member {
            if *offset >= source_length {
                return Err("stored cohort offset exceeds source length".to_owned());
            }
            offsets.insert(*offset);
        }
    }
    if !cohort.member_ranges.is_empty() {
        if cohort.member_ranges.len() != cohort.members.len() {
            return Err("stored cohort member ranges do not align with membership".to_owned());
        }
        for range in &cohort.member_ranges {
            validate_ranges(&[*range], source_length, "cohort member")?;
        }
    }
    validate_ranges(&cohort.exact_ranges, source_length, "cohort")?;
    let derived = if cohort.member_ranges.is_empty() {
        ranges_from_offsets(&offsets)?
    } else {
        normalized_ranges(&cohort.member_ranges)
    };
    if derived != cohort.exact_ranges {
        return Err("stored cohort ranges do not match its exact members".to_owned());
    }
    Ok(())
}

fn normalized_ranges(ranges: &[StoredRange]) -> Vec<StoredRange> {
    let mut candidates = ranges.to_vec();
    candidates.sort_by_key(|range| (range.start, range.end));
    let mut normalized: Vec<StoredRange> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        match normalized.last_mut() {
            Some(last) if candidate.start <= last.end => last.end = last.end.max(candidate.end),
            _ => normalized.push(candidate),
        }
    }
    normalized
}

fn validate_ranges(ranges: &[StoredRange], source_length: u64, label: &str) -> Result<(), String> {
    if ranges.len() > MAX_STORED_RANGES {
        return Err(format!("too many stored {label} ranges"));
    }
    let mut previous_end = None;
    for range in ranges {
        if range.start >= range.end || range.end > source_length {
            return Err(format!("stored {label} range is empty or out of bounds"));
        }
        if previous_end.is_some_and(|end| range.start <= end) {
            return Err(format!(
                "stored {label} ranges overlap or are not normalized"
            ));
        }
        previous_end = Some(range.end);
    }
    Ok(())
}

fn ranges_from_offsets(offsets: &BTreeSet<u64>) -> Result<Vec<StoredRange>, String> {
    let mut ranges = Vec::new();
    let mut start = None;
    let mut previous = None;
    for &offset in offsets {
        match (start, previous) {
            (None, None) => {
                start = Some(offset);
                previous = Some(offset);
            }
            (Some(_), Some(last)) if offset == last.saturating_add(1) => {
                previous = Some(offset);
            }
            (Some(range_start), Some(last)) => {
                ranges.push(StoredRange {
                    start: range_start,
                    end: last.checked_add(1).ok_or("stored cohort range overflow")?,
                });
                start = Some(offset);
                previous = Some(offset);
            }
            _ => return Err("stored cohort offset state is inconsistent".to_owned()),
        }
    }
    if let (Some(range_start), Some(last)) = (start, previous) {
        ranges.push(StoredRange {
            start: range_start,
            end: last.checked_add(1).ok_or("stored cohort range overflow")?,
        });
    }
    Ok(ranges)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use strata_session::{
        Journal, JournalEvent, Reattachment, SessionBundle, SourceFingerprint, WorkspaceSnapshot,
    };

    use super::{
        POC_WORKSPACE_VERSION, PocWorkspaceSnapshot, ProjectionComposition, StoredBranchStatus,
        StoredCohort, StoredContourMode, StoredFindingDisposition, StoredFindingStatus,
        StoredProjectionState, StoredRange, StoredRenderStyle, StoredResonanceMetric, StoredView,
        StoredWorkbenchMode, StoredXorBranch,
    };

    fn snapshot() -> PocWorkspaceSnapshot {
        PocWorkspaceSnapshot {
            version: POC_WORKSPACE_VERSION,
            source_generation: 4,
            active_view: StoredView::Projection3d,
            workbench_mode: StoredWorkbenchMode::Leads,
            exact_selection: vec![
                StoredRange { start: 4, end: 7 },
                StoredRange { start: 10, end: 12 },
            ],
            selected_lead: Some(9),
            selected_region: None,
            selected_comparison: None,
            finding_dispositions: vec![StoredFindingDisposition {
                lead_id: 9,
                status: StoredFindingStatus::Promoted,
            }],
            branches: vec![StoredXorBranch {
                id: [9, 44],
                label: "XOR 0xa7".to_owned(),
                range: StoredRange { start: 4, end: 12 },
                key: 0xa7,
                status: StoredBranchStatus::Pinned,
            }],
            selected_branch: Some([9, 44]),
            branch_key: 0xa7,
            cohort: Some(StoredCohort {
                members: vec![[4, 5, 6], [10, 10, 11]],
                member_ranges: Vec::new(),
                exact_ranges: vec![
                    StoredRange { start: 4, end: 7 },
                    StoredRange { start: 10, end: 12 },
                ],
            }),
            atlas_width: 32,
            digram_stride: 1,
            interleave_width: 24,
            interleave_stride: 6,
            interleave_lane: 5,
            bit_plane: 3,
            diff_width: 32,
            resonance_metric: StoredResonanceMetric::ByteShape,
            resonance_base_window: 8,
            resonance_stride: 1,
            resonance_sample_budget: 1_024,
            projection: StoredProjectionState {
                composition: Some(ProjectionComposition::default()),
                stride: 1,
                point_budget: 12_000,
                morph: 2.0,
                color_mix: 0.0,
                relief: 1.0,
                context_light: 0.72,
                point_size: 1.8,
                brightness: 1.25,
                perspective: 0.72,
                render_style: StoredRenderStyle::Voxels,
                field_radius: 20.0,
                field_exposure: 1.4,
                contour_mode: StoredContourMode::Off,
                yaw: -0.72,
                pitch: 0.38,
                zoom: 0.92,
                spin: false,
                auto_morph: false,
                speed: 0.32,
            },
        }
    }

    #[test]
    fn snapshot_round_trip_is_deterministic_and_source_free() -> Result<(), String> {
        let snapshot = snapshot();
        snapshot.validate(64)?;
        let first = serde_json::to_vec_pretty(&snapshot).map_err(|error| error.to_string())?;
        let decoded: PocWorkspaceSnapshot =
            serde_json::from_slice(&first).map_err(|error| error.to_string())?;
        decoded.validate(64)?;
        let second = serde_json::to_vec_pretty(&decoded).map_err(|error| error.to_string())?;
        assert_eq!(first, second);
        let text = String::from_utf8(first).map_err(|error| error.to_string())?;
        assert!(!text.contains("/Users/example/private.bin"));
        assert!(!text.contains("RAW_SOURCE_SENTINEL"));
        Ok(())
    }

    #[test]
    fn full_width_region_ids_use_backward_compatible_hex() -> Result<(), String> {
        let mut snapshot = snapshot();
        snapshot.selected_region = Some(u128::MAX);
        snapshot.selected_comparison = Some(u128::MAX - 1);
        let encoded = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
        assert!(encoded.contains("0xffffffffffffffffffffffffffffffff"));
        let decoded: PocWorkspaceSnapshot =
            serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
        assert_eq!(decoded.selected_region, snapshot.selected_region);
        assert_eq!(decoded.selected_comparison, snapshot.selected_comparison);

        let mut legacy = serde_json::to_value(snapshot).map_err(|error| error.to_string())?;
        legacy["selected_region"] = serde_json::Value::from(42_u64);
        let decoded: PocWorkspaceSnapshot =
            serde_json::from_value(legacy).map_err(|error| error.to_string())?;
        assert_eq!(decoded.selected_region, Some(42));
        Ok(())
    }

    #[test]
    fn snapshot_rejects_overlapping_ranges() {
        let mut snapshot = snapshot();
        snapshot.cohort = None;
        snapshot.exact_selection = vec![
            StoredRange { start: 4, end: 9 },
            StoredRange { start: 8, end: 12 },
        ];
        assert!(snapshot.validate(64).is_err());
    }

    #[test]
    fn cohort_ranges_must_match_exact_member_offsets() {
        let mut snapshot = snapshot();
        if let Some(cohort) = &mut snapshot.cohort {
            cohort.exact_ranges = vec![StoredRange { start: 4, end: 12 }];
        }
        assert!(snapshot.validate(64).is_err());
    }

    #[test]
    fn snapshot_rejects_injected_source_or_path_fields() -> Result<(), String> {
        let mut value = serde_json::to_value(snapshot()).map_err(|error| error.to_string())?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "snapshot must encode as an object".to_owned())?;
        object.insert(
            "source_bytes".to_owned(),
            serde_json::json!([82, 65, 87, 95, 83, 79, 85, 82, 67, 69]),
        );
        object.insert(
            "source_path".to_owned(),
            serde_json::json!("/Users/example/private.bin"),
        );
        assert!(serde_json::from_value::<PocWorkspaceSnapshot>(value).is_err());
        Ok(())
    }

    #[test]
    fn source_free_bundle_preserves_exact_state_and_requires_digest_match() -> Result<(), String> {
        let directory = unique_bundle_directory();
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let _cleanup = TestDirectory(directory.clone());
        let snapshot = snapshot();
        snapshot.validate(64)?;
        let workspace = WorkspaceSnapshot::from_value(
            serde_json::to_value(&snapshot).map_err(|error| error.to_string())?,
        );
        let mut journal = Journal::new();
        journal
            .append(JournalEvent::WorkspaceChanged(workspace.clone()))
            .map_err(|error| error.to_string())?;
        let mut source = vec![0_u8; 64];
        let sentinel = b"RAW_SOURCE_SENTINEL";
        let prefix = source
            .get_mut(..sentinel.len())
            .ok_or_else(|| "source fixture is too small".to_owned())?;
        prefix.copy_from_slice(sentinel);
        let bundle = SessionBundle::new(
            SourceFingerprint::from_bytes("redacted-primary-source", &source)
                .map_err(|error| error.to_string())?,
            workspace,
            journal,
        )
        .map_err(|error| error.to_string())?;
        bundle
            .save_to_directory(&directory)
            .map_err(|error| error.to_string())?;

        let manifest =
            fs::read(directory.join("manifest.json")).map_err(|error| error.to_string())?;
        let journal =
            fs::read(directory.join("journal.ndjson")).map_err(|error| error.to_string())?;
        assert!(
            !manifest
                .windows(sentinel.len())
                .any(|window| window == sentinel)
        );
        assert!(
            !journal
                .windows(sentinel.len())
                .any(|window| window == sentinel)
        );
        assert!(!String::from_utf8_lossy(&manifest).contains("/Users/example/private.bin"));

        let loaded =
            SessionBundle::load_from_directory(&directory).map_err(|error| error.to_string())?;
        let decoded: PocWorkspaceSnapshot =
            serde_json::from_value(loaded.manifest().workspace().value().clone())
                .map_err(|error| error.to_string())?;
        assert_eq!(decoded, snapshot);
        assert_eq!(decoded.exact_selection.len(), 2);
        let workspace_before_mismatch = loaded.manifest().workspace().value().clone();
        let mut mismatch = source.clone();
        let first = mismatch
            .first_mut()
            .ok_or_else(|| "source fixture is empty".to_owned())?;
        *first ^= 0xff;
        assert!(matches!(
            loaded.reattach(&mismatch),
            Reattachment::Mismatch { .. }
        ));
        assert_eq!(
            loaded.manifest().workspace().value(),
            &workspace_before_mismatch
        );
        assert_eq!(loaded.reattach(&source), Reattachment::Match);
        Ok(())
    }

    struct TestDirectory(PathBuf);

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _result = fs::remove_dir_all(&self.0);
        }
    }

    fn unique_bundle_directory() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "strata-poc-session-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
