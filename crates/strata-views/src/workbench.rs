//! UI-independent state for the remaining Strata workbench slices.
//!
//! The models here deliberately store caller-assigned identities and exact source provenance.
//! Rendering, sampling, and persistence layers can therefore replace each other without
//! changing analyst-visible region, branch, comparison, or cohort state.

use strata_core::{ByteRangeSet, SourceGeneration, SourceId, TransformGraphSpec};

use crate::investigation::ExactProvenance;

/// Stable identity for a living source region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionId(pub u128);

/// Stable identity for a typed relationship between living regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionRelationshipId(pub u128);

/// Stable identity for a reversible-hypothesis branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BranchId(pub u128);

/// Stable identity for one paired-source comparison workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComparisonId(pub u128);

/// Stable identity for one archaeological comparison region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComparisonRegionId(pub u128);

/// Exact identity of a sampled byte in a 3D cohort.
///
/// One source generation and byte offset produce exactly one identity in a cohort model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SampledByteId {
    /// Source that supplied the sampled byte.
    pub source_id: SourceId,
    /// Source generation that supplied the sampled byte.
    pub generation: SourceGeneration,
    /// Exact source offset of the sampled byte.
    pub offset: u64,
}

/// Typed semantic role for a living region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionKind {
    /// A candidate file or segment header.
    Header,
    /// A likely fixed-width or indexed table.
    Table,
    /// A likely executable or instruction-like span.
    Code,
    /// A text-like span.
    Text,
    /// Repeated fill, alignment, or erased storage.
    Padding,
    /// A statistically distinct but not yet semantically identified span.
    Structural,
    /// A caller-defined typed role.
    Custom(String),
}

/// One mutable analyst-facing region, always tied to exact source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivingRegion {
    /// Stable caller-assigned identity.
    pub id: RegionId,
    /// Short display label.
    pub label: String,
    /// Typed region role.
    pub kind: RegionKind,
    /// Exact source generation and ranges represented by this region.
    pub provenance: ExactProvenance,
    /// Parent region, when this region refines another known region.
    pub parent_id: Option<RegionId>,
}

/// Typed relationship semantics for regions that are not parent/child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRelationshipKind {
    /// One region references or points to another.
    References,
    /// Regions are adjacent in the same source layout.
    Adjacent,
    /// Regions share a measured structural signature.
    Similar,
    /// One region is an exact single-byte XOR encoding of another.
    XorEncoded,
    /// Regions are repeated instances of a common layout.
    Repeats,
}

/// A typed, provenance-bearing relationship between two living regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRelationship {
    /// Stable caller-assigned relationship identity.
    pub id: RegionRelationshipId,
    /// Region originating the relationship.
    pub from: RegionId,
    /// Region receiving the relationship.
    pub to: RegionId,
    /// Typed relationship semantics.
    pub kind: RegionRelationshipKind,
    /// Exact range where the relationship was observed.
    pub provenance: ExactProvenance,
    /// Concise analyst or analyzer rationale.
    pub rationale: String,
}

/// Deterministic region graph with parent/child and typed peer relationships.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionModel {
    regions: Vec<LivingRegion>,
    relationships: Vec<RegionRelationship>,
}

impl RegionModel {
    /// Creates an empty region graph.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
            relationships: Vec::new(),
        }
    }

    /// Returns living regions in deterministic insertion order.
    #[must_use]
    pub fn regions(&self) -> &[LivingRegion] {
        &self.regions
    }

    /// Returns typed region relationships in deterministic insertion order.
    #[must_use]
    pub fn relationships(&self) -> &[RegionRelationship] {
        &self.relationships
    }

    /// Looks up a region by its stable ID.
    #[must_use]
    pub fn region(&self, id: RegionId) -> Option<&LivingRegion> {
        self.regions.iter().find(|region| region.id == id)
    }

    /// Returns direct children of one region in insertion order.
    #[must_use]
    pub fn children(&self, parent_id: RegionId) -> Vec<&LivingRegion> {
        self.regions
            .iter()
            .filter(|region| region.parent_id == Some(parent_id))
            .collect()
    }

    /// Adds a region after validating its exact provenance and parent containment.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkbenchError`] for invalid provenance, duplicate IDs, unknown parents, or
    /// child ranges that are not contained by their parent snapshot.
    pub fn add_region(&mut self, region: LivingRegion) -> Result<(), WorkbenchError> {
        validate_provenance(&region.provenance)?;
        if self.regions.iter().any(|existing| existing.id == region.id) {
            return Err(WorkbenchError::DuplicateId);
        }
        if let Some(parent_id) = region.parent_id {
            let parent = self
                .region(parent_id)
                .ok_or(WorkbenchError::UnknownRegion(parent_id))?;
            if !same_snapshot(&parent.provenance, &region.provenance)
                || !ranges_contained_by(&region.provenance.ranges, &parent.provenance.ranges)
            {
                return Err(WorkbenchError::ProvenanceMismatch);
            }
        }
        self.regions.push(region);
        Ok(())
    }

    /// Adds a typed relationship between two known regions.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkbenchError`] for invalid provenance, duplicate IDs, unknown endpoints,
    /// self-links, or evidence that does not belong to the endpoints' source snapshot.
    pub fn add_relationship(
        &mut self,
        relationship: RegionRelationship,
    ) -> Result<(), WorkbenchError> {
        validate_provenance(&relationship.provenance)?;
        if self
            .relationships
            .iter()
            .any(|existing| existing.id == relationship.id)
        {
            return Err(WorkbenchError::DuplicateId);
        }
        if relationship.from == relationship.to {
            return Err(WorkbenchError::InvalidRelationship);
        }
        let from = self
            .region(relationship.from)
            .ok_or(WorkbenchError::UnknownRegion(relationship.from))?;
        let to = self
            .region(relationship.to)
            .ok_or(WorkbenchError::UnknownRegion(relationship.to))?;
        if !same_snapshot(&from.provenance, &relationship.provenance)
            || !same_snapshot(&to.provenance, &relationship.provenance)
        {
            return Err(WorkbenchError::ProvenanceMismatch);
        }
        self.relationships.push(relationship);
        Ok(())
    }
}

/// Fixed-point metric value; its unit and scale are part of `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricValue {
    /// Stable metric name, including the caller's chosen unit/scale.
    pub name: String,
    /// Exact fixed-point integer value.
    pub value: i64,
}

/// Reversibility and loss declaration for a hypothesis branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchReversibility {
    /// The branch can be inverted using the transform graph's declared inverse specifications.
    Reversible {
        /// Explicit loss model, including the no-loss case when applicable.
        loss_model: String,
    },
    /// The branch cannot be inverted without loss.
    Lossy {
        /// Explicit description of lost or non-recoverable information.
        loss_model: String,
    },
}

/// Lifecycle state for an analyst hypothesis branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchStatus {
    /// Created but not yet used as active workbench context.
    Draft,
    /// Available as active exploratory context.
    Active,
    /// Retained for comparison or evidence.
    Pinned,
    /// Removed from active work while preserving its record.
    Discarded,
}

/// Transform branch with ancestry, reversibility, and before/after measurements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HypothesisBranch {
    /// Stable caller-assigned branch identity.
    pub id: BranchId,
    /// Short display label.
    pub label: String,
    /// Parent branch when this branch refines an earlier hypothesis.
    pub parent_id: Option<BranchId>,
    /// Exact source bytes that the branch transforms.
    pub provenance: ExactProvenance,
    /// Reproducible transform graph specification.
    pub transform: TransformGraphSpec,
    /// Explicit reversibility and loss declaration.
    pub reversibility: BranchReversibility,
    /// Metrics captured immediately before applying the branch transform.
    pub before_metrics: Vec<MetricValue>,
    /// Metrics captured immediately after applying the branch transform.
    pub after_metrics: Vec<MetricValue>,
    /// Branch lifecycle status.
    pub status: BranchStatus,
}

/// One metric value compared across two branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchMetricComparison {
    /// Metric name from either compared branch.
    pub name: String,
    /// Value from the first branch when it reported this metric.
    pub first: Option<i64>,
    /// Value from the second branch when it reported this metric.
    pub second: Option<i64>,
}

/// Deterministic comparison result for two branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchComparison {
    /// First branch requested by the caller.
    pub first_id: BranchId,
    /// Second branch requested by the caller.
    pub second_id: BranchId,
    /// Union of after-metric names, in stable first-then-second order.
    pub after_metrics: Vec<BranchMetricComparison>,
}

/// State holder for reversible and lossy branch hypotheses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchModel {
    branches: Vec<HypothesisBranch>,
}

impl BranchModel {
    /// Creates an empty branch model.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            branches: Vec::new(),
        }
    }

    /// Returns branches in deterministic insertion order.
    #[must_use]
    pub fn branches(&self) -> &[HypothesisBranch] {
        &self.branches
    }

    /// Looks up a branch by stable ID.
    #[must_use]
    pub fn branch(&self, id: BranchId) -> Option<&HypothesisBranch> {
        self.branches.iter().find(|branch| branch.id == id)
    }

    /// Adds a branch after validating ancestry, provenance, and reversibility declarations.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkbenchError`] for duplicate IDs, invalid provenance or reversibility, an
    /// unknown parent, or a branch outside its parent's exact source ranges.
    pub fn add_branch(&mut self, branch: HypothesisBranch) -> Result<(), WorkbenchError> {
        validate_provenance(&branch.provenance)?;
        validate_branch_reversibility(&branch)?;
        if self
            .branches
            .iter()
            .any(|existing| existing.id == branch.id)
        {
            return Err(WorkbenchError::DuplicateId);
        }
        if let Some(parent_id) = branch.parent_id {
            let parent = self
                .branch(parent_id)
                .ok_or(WorkbenchError::UnknownBranch(parent_id))?;
            if !same_snapshot(&parent.provenance, &branch.provenance)
                || !ranges_contained_by(&branch.provenance.ranges, &parent.provenance.ranges)
            {
                return Err(WorkbenchError::ProvenanceMismatch);
            }
        }
        self.branches.push(branch);
        Ok(())
    }

    /// Pins a non-discarded branch for subsequent comparison or evidence.
    ///
    /// # Errors
    ///
    /// Returns [`WorkbenchError::UnknownBranch`] or
    /// [`WorkbenchError::InvalidBranchTransition`] for a discarded branch.
    pub fn pin(&mut self, id: BranchId) -> Result<(), WorkbenchError> {
        let branch = self.branch_mut(id)?;
        if branch.status == BranchStatus::Discarded {
            return Err(WorkbenchError::InvalidBranchTransition);
        }
        branch.status = BranchStatus::Pinned;
        Ok(())
    }

    /// Discards a branch from active work without erasing its provenance or metrics.
    ///
    /// # Errors
    ///
    /// Returns [`WorkbenchError::UnknownBranch`] when `id` is not present.
    pub fn discard(&mut self, id: BranchId) -> Result<(), WorkbenchError> {
        let branch = self.branch_mut(id)?;
        branch.status = BranchStatus::Discarded;
        Ok(())
    }

    /// Compares two non-discarded branches using their after-transform metrics.
    ///
    /// # Errors
    ///
    /// Returns [`WorkbenchError::UnknownBranch`] for an unknown ID or
    /// [`WorkbenchError::InvalidBranchTransition`] when either branch was discarded.
    pub fn compare(
        &self,
        first_id: BranchId,
        second_id: BranchId,
    ) -> Result<BranchComparison, WorkbenchError> {
        let first = self
            .branch(first_id)
            .ok_or(WorkbenchError::UnknownBranch(first_id))?;
        let second = self
            .branch(second_id)
            .ok_or(WorkbenchError::UnknownBranch(second_id))?;
        if first.status == BranchStatus::Discarded || second.status == BranchStatus::Discarded {
            return Err(WorkbenchError::InvalidBranchTransition);
        }
        let mut metrics = Vec::new();
        for metric in &first.after_metrics {
            metrics.push(BranchMetricComparison {
                name: metric.name.clone(),
                first: Some(metric.value),
                second: metric_value(&second.after_metrics, &metric.name),
            });
        }
        for metric in &second.after_metrics {
            if metric_value(&first.after_metrics, &metric.name).is_none() {
                metrics.push(BranchMetricComparison {
                    name: metric.name.clone(),
                    first: None,
                    second: Some(metric.value),
                });
            }
        }
        Ok(BranchComparison {
            first_id,
            second_id,
            after_metrics: metrics,
        })
    }

    fn branch_mut(&mut self, id: BranchId) -> Result<&mut HypothesisBranch, WorkbenchError> {
        self.branches
            .iter_mut()
            .find(|branch| branch.id == id)
            .ok_or(WorkbenchError::UnknownBranch(id))
    }
}

/// Exact identity of one side of a paired-source comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSnapshot {
    /// Source identity.
    pub source_id: SourceId,
    /// Immutable or live-source generation compared.
    pub generation: SourceGeneration,
}

/// Paired source context for comparison archaeology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparisonPair {
    /// Stable caller-assigned comparison identity.
    pub id: ComparisonId,
    /// Earlier or baseline source snapshot.
    pub left: SourceSnapshot,
    /// Later or candidate source snapshot.
    pub right: SourceSnapshot,
}

/// Classification for an exact paired comparison region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonClassification {
    /// Exact bytes occur at corresponding locations in both sources.
    Unchanged,
    /// Equivalent bytes appear at a different location in the paired source.
    Moved,
    /// Both sources contain the region but its bytes differ.
    Modified,
    /// The region exists only in the right/new source.
    New,
}

/// One exact archaeological region in a paired comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonRegion {
    /// Stable caller-assigned region identity.
    pub id: ComparisonRegionId,
    /// Classification derived by the selected comparison algorithm.
    pub classification: ComparisonClassification,
    /// Exact baseline ranges; absent only for a `New` region.
    pub left: Option<ExactProvenance>,
    /// Exact candidate ranges; required for all classifications in this POC.
    pub right: ExactProvenance,
    /// Human-readable explanation of the classification.
    pub explanation: String,
}

/// State for one source pair and its classified archaeological regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonArchaeology {
    /// Paired source snapshots.
    pub pair: ComparisonPair,
    regions: Vec<ComparisonRegion>,
}

impl ComparisonArchaeology {
    /// Creates a comparison workspace for an explicit pair of source snapshots.
    #[must_use]
    pub const fn new(pair: ComparisonPair) -> Self {
        Self {
            pair,
            regions: Vec::new(),
        }
    }

    /// Returns classified regions in deterministic insertion order.
    #[must_use]
    pub fn regions(&self) -> &[ComparisonRegion] {
        &self.regions
    }

    /// Looks up a classified comparison region by stable ID.
    #[must_use]
    pub fn region(&self, id: ComparisonRegionId) -> Option<&ComparisonRegion> {
        self.regions.iter().find(|region| region.id == id)
    }

    /// Adds a classified region after validating its left/right source snapshots and ranges.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkbenchError`] for duplicate IDs, invalid or mismatched provenance, missing
    /// sides, or range geometry inconsistent with the selected classification.
    pub fn add_region(&mut self, region: ComparisonRegion) -> Result<(), WorkbenchError> {
        if self.regions.iter().any(|existing| existing.id == region.id) {
            return Err(WorkbenchError::DuplicateId);
        }
        validate_provenance(&region.right)?;
        if region.right.source_id != self.pair.right.source_id
            || region.right.generation != self.pair.right.generation
        {
            return Err(WorkbenchError::ProvenanceMismatch);
        }
        match (&region.classification, &region.left) {
            (ComparisonClassification::New, None) => {}
            (ComparisonClassification::New, Some(_)) => {
                return Err(WorkbenchError::InvalidComparisonRegion);
            }
            (_, None) => return Err(WorkbenchError::InvalidComparisonRegion),
            (_, Some(left)) => {
                validate_provenance(left)?;
                if left.source_id != self.pair.left.source_id
                    || left.generation != self.pair.left.generation
                {
                    return Err(WorkbenchError::ProvenanceMismatch);
                }
                let same_locations = left.ranges == region.right.ranges;
                match region.classification {
                    ComparisonClassification::Unchanged | ComparisonClassification::Modified
                        if !same_locations =>
                    {
                        return Err(WorkbenchError::InvalidComparisonRegion);
                    }
                    ComparisonClassification::Moved if same_locations => {
                        return Err(WorkbenchError::InvalidComparisonRegion);
                    }
                    ComparisonClassification::Unchanged
                    | ComparisonClassification::Modified
                    | ComparisonClassification::Moved => {}
                    ComparisonClassification::New => {
                        return Err(WorkbenchError::InvalidComparisonRegion);
                    }
                }
            }
        }
        self.regions.push(region);
        Ok(())
    }
}

/// An explanatory factor for one sampled byte or lasso cohort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortFactor {
    /// Factor label, including any caller-defined normalization/unit.
    pub name: String,
    /// Signed fixed-point contribution in the factor's declared scale.
    pub contribution: i64,
}

/// A stable, render-independent identity and explanation for one sampled byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortSample {
    /// Exact stable byte identity.
    pub id: SampledByteId,
    /// Source byte value retained for local explanation.
    pub byte: u8,
    /// Fixed-point 3D location in caller-defined view coordinates.
    pub position: [i32; 3],
    /// Factors that placed or highlighted this byte.
    pub factors: Vec<CohortFactor>,
    /// Exact single-byte source provenance matching `id`.
    pub provenance: ExactProvenance,
}

/// Exact membership and explanation of a lasso-selected 3D cohort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortSelection {
    /// Stable byte identities selected by the lasso, in lasso input order.
    pub member_ids: Vec<SampledByteId>,
    /// Exact normalized source ranges for all selected bytes.
    pub provenance: ExactProvenance,
    /// Analyst-facing explanation of why this cohort matters.
    pub explanation: String,
    /// Aggregated factors presented alongside the selection.
    pub factors: Vec<CohortFactor>,
}

/// State for one source-generation 3D cohort projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortModel {
    source: SourceSnapshot,
    samples: Vec<CohortSample>,
    selection: Option<CohortSelection>,
}

impl CohortModel {
    /// Creates an empty 3D cohort for one exact source generation.
    #[must_use]
    pub const fn new(source: SourceSnapshot) -> Self {
        Self {
            source,
            samples: Vec::new(),
            selection: None,
        }
    }

    /// Returns the source generation represented by this cohort.
    #[must_use]
    pub const fn source(&self) -> SourceSnapshot {
        self.source
    }

    /// Returns samples in deterministic insertion order.
    #[must_use]
    pub fn samples(&self) -> &[CohortSample] {
        &self.samples
    }

    /// Returns current lasso membership and explanation.
    #[must_use]
    pub const fn selection(&self) -> Option<&CohortSelection> {
        self.selection.as_ref()
    }

    /// Looks up one sampled byte by exact stable identity.
    #[must_use]
    pub fn sample(&self, id: SampledByteId) -> Option<&CohortSample> {
        self.samples.iter().find(|sample| sample.id == id)
    }

    /// Adds one source-generation-consistent sampled byte.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkbenchError`] when the identity/provenance disagrees with the cohort source,
    /// its one-byte range is invalid, or the sample ID already exists.
    pub fn add_sample(&mut self, sample: CohortSample) -> Result<(), WorkbenchError> {
        validate_sample(&sample, self.source)?;
        if self.samples.iter().any(|existing| existing.id == sample.id) {
            return Err(WorkbenchError::DuplicateId);
        }
        self.samples.push(sample);
        Ok(())
    }

    /// Replaces lasso membership with known samples and materializes their exact source ranges.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkbenchError`] for empty or duplicate membership, unknown sample IDs, or
    /// invalid materialized provenance.
    pub fn select_lasso(
        &mut self,
        member_ids: Vec<SampledByteId>,
        explanation: String,
        factors: Vec<CohortFactor>,
    ) -> Result<(), WorkbenchError> {
        if member_ids.is_empty() {
            return Err(WorkbenchError::EmptySelection);
        }
        if has_duplicates(&member_ids) {
            return Err(WorkbenchError::DuplicateSelectionMember);
        }
        let mut ranges = Vec::with_capacity(member_ids.len());
        for id in &member_ids {
            let sample = self.sample(*id).ok_or(WorkbenchError::UnknownSample(*id))?;
            for range in &sample.provenance.ranges.ranges {
                ranges.push(*range);
            }
        }
        ranges.sort_by_key(|range| range.start);
        let provenance = ExactProvenance {
            source_id: self.source.source_id,
            generation: self.source.generation,
            ranges: ByteRangeSet { ranges },
        };
        validate_provenance(&provenance)?;
        self.selection = Some(CohortSelection {
            member_ids,
            provenance,
            explanation,
            factors,
        });
        Ok(())
    }

    /// Clears only lasso membership; sampled identities remain available for a new selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
}

/// Typed errors used by all workbench state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkbenchError {
    /// A caller attempted to add an already-known stable ID.
    DuplicateId,
    /// Exact provenance had no ranges.
    EmptyRanges,
    /// Exact provenance contains an empty, unordered, or overlapping range.
    InvalidRanges,
    /// A parent/relationship source snapshot did not match its child or link.
    ProvenanceMismatch,
    /// A required living region is absent.
    UnknownRegion(RegionId),
    /// A relationship linked a region to itself or violated its contract.
    InvalidRelationship,
    /// A required branch is absent.
    UnknownBranch(BranchId),
    /// A transform's reversibility declaration is inconsistent or incomplete.
    InvalidReversibility,
    /// The requested pin, discard, or comparison operation is invalid for branch state.
    InvalidBranchTransition,
    /// A comparison region had incompatible sides for its classification.
    InvalidComparisonRegion,
    /// A required sampled byte is absent.
    UnknownSample(SampledByteId),
    /// A sampled byte did not represent its exact ID or cohort source generation.
    InvalidSample,
    /// A lasso request contained no sample identities.
    EmptySelection,
    /// A lasso request repeated one sampled byte identity.
    DuplicateSelectionMember,
}

impl core::fmt::Display for WorkbenchError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WorkbenchError {}

fn validate_provenance(provenance: &ExactProvenance) -> Result<(), WorkbenchError> {
    let Some(first) = provenance.ranges.ranges.first() else {
        return Err(WorkbenchError::EmptyRanges);
    };
    if first.is_empty() {
        return Err(WorkbenchError::InvalidRanges);
    }
    let mut previous_end = first.end;
    for range in provenance.ranges.ranges.iter().skip(1) {
        if range.is_empty() || range.start < previous_end {
            return Err(WorkbenchError::InvalidRanges);
        }
        previous_end = range.end;
    }
    Ok(())
}

fn ranges_contained_by(child: &ByteRangeSet, parent: &ByteRangeSet) -> bool {
    child.ranges.iter().all(|child_range| {
        parent.ranges.iter().any(|parent_range| {
            parent_range.start <= child_range.start && child_range.end <= parent_range.end
        })
    })
}

fn same_snapshot(first: &ExactProvenance, second: &ExactProvenance) -> bool {
    first.source_id == second.source_id && first.generation == second.generation
}

fn validate_branch_reversibility(branch: &HypothesisBranch) -> Result<(), WorkbenchError> {
    let loss_model = match &branch.reversibility {
        BranchReversibility::Reversible { loss_model }
        | BranchReversibility::Lossy { loss_model } => loss_model,
    };
    if loss_model.trim().is_empty() {
        return Err(WorkbenchError::InvalidReversibility);
    }
    if matches!(
        &branch.reversibility,
        BranchReversibility::Reversible { .. }
    ) {
        for node in &branch.transform.nodes {
            if !node.reversible || node.inverse_spec_json.is_none() || node.loss_model.is_none() {
                return Err(WorkbenchError::InvalidReversibility);
            }
        }
    }
    Ok(())
}

fn metric_value(metrics: &[MetricValue], name: &str) -> Option<i64> {
    metrics
        .iter()
        .find(|metric| metric.name == name)
        .map(|metric| metric.value)
}

fn validate_sample(sample: &CohortSample, source: SourceSnapshot) -> Result<(), WorkbenchError> {
    validate_provenance(&sample.provenance)?;
    if sample.id.source_id != source.source_id
        || sample.id.generation != source.generation
        || sample.provenance.source_id != source.source_id
        || sample.provenance.generation != source.generation
        || sample.provenance.ranges.ranges.len() != 1
    {
        return Err(WorkbenchError::InvalidSample);
    }
    let Some(range) = sample.provenance.ranges.ranges.first() else {
        return Err(WorkbenchError::InvalidSample);
    };
    let Some(expected_end) = sample.id.offset.checked_add(1) else {
        return Err(WorkbenchError::InvalidSample);
    };
    if range.start != sample.id.offset || range.end != expected_end {
        return Err(WorkbenchError::InvalidSample);
    }
    Ok(())
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

#[cfg(test)]
mod tests {
    use strata_core::{ByteRange, ByteRangeSet, SourceGeneration, SourceId, TransformGraphSpec};

    use super::{
        BranchId, BranchModel, BranchReversibility, BranchStatus, CohortFactor, CohortModel,
        CohortSample, ComparisonArchaeology, ComparisonClassification, ComparisonId,
        ComparisonPair, ComparisonRegion, ComparisonRegionId, ExactProvenance, HypothesisBranch,
        LivingRegion, MetricValue, RegionId, RegionKind, RegionModel, RegionRelationship,
        RegionRelationshipId, RegionRelationshipKind, SampledByteId, SourceSnapshot,
        WorkbenchError,
    };

    fn provenance(start: u64, end: u64) -> Result<ExactProvenance, WorkbenchError> {
        let range = ByteRange::new(start, end).map_err(|_| WorkbenchError::InvalidRanges)?;
        Ok(ExactProvenance {
            source_id: SourceId(1),
            generation: SourceGeneration(2),
            ranges: ByteRangeSet {
                ranges: vec![range],
            },
        })
    }

    fn region(
        id: u128,
        start: u64,
        end: u64,
        parent_id: Option<RegionId>,
    ) -> Result<LivingRegion, WorkbenchError> {
        Ok(LivingRegion {
            id: RegionId(id),
            label: "region".to_owned(),
            kind: RegionKind::Structural,
            provenance: provenance(start, end)?,
            parent_id,
        })
    }

    fn branch(id: u128, parent_id: Option<BranchId>) -> Result<HypothesisBranch, WorkbenchError> {
        Ok(HypothesisBranch {
            id: BranchId(id),
            label: "stride 4".to_owned(),
            parent_id,
            provenance: provenance(0, 16)?,
            transform: TransformGraphSpec::default(),
            reversibility: BranchReversibility::Reversible {
                loss_model: "none".to_owned(),
            },
            before_metrics: vec![MetricValue {
                name: "entropy_milli_bits".to_owned(),
                value: 1_000,
            }],
            after_metrics: vec![MetricValue {
                name: "entropy_milli_bits".to_owned(),
                value: 800,
            }],
            status: BranchStatus::Active,
        })
    }

    #[test]
    fn living_regions_validate_parentage_and_typed_relationships() -> Result<(), WorkbenchError> {
        let mut model = RegionModel::new();
        model.add_region(region(1, 0, 32, None)?)?;
        model.add_region(region(2, 8, 16, Some(RegionId(1)))?)?;
        assert_eq!(model.children(RegionId(1)).len(), 1);
        let relationship = RegionRelationship {
            id: RegionRelationshipId(1),
            from: RegionId(1),
            to: RegionId(2),
            kind: RegionRelationshipKind::References,
            provenance: provenance(8, 16)?,
            rationale: "header directs to its table".to_owned(),
        };
        model.add_relationship(relationship)?;
        assert_eq!(model.relationships().len(), 1);
        assert_eq!(
            model.add_region(region(3, 40, 48, Some(RegionId(1)))?),
            Err(WorkbenchError::ProvenanceMismatch)
        );
        Ok(())
    }

    #[test]
    fn branches_pin_discard_and_compare_metrics() -> Result<(), WorkbenchError> {
        let mut model = BranchModel::new();
        model.add_branch(branch(1, None)?)?;
        let mut second = branch(2, Some(BranchId(1)))?;
        second.after_metrics.push(MetricValue {
            name: "periodicity_milli".to_owned(),
            value: 900,
        });
        model.add_branch(second)?;
        model.pin(BranchId(1))?;
        let comparison = model.compare(BranchId(1), BranchId(2))?;
        assert_eq!(comparison.after_metrics.len(), 2);
        model.discard(BranchId(2))?;
        assert_eq!(
            model.compare(BranchId(1), BranchId(2)),
            Err(WorkbenchError::InvalidBranchTransition)
        );
        Ok(())
    }

    #[test]
    fn comparison_archaeology_requires_classified_exact_sides() -> Result<(), WorkbenchError> {
        let pair = ComparisonPair {
            id: ComparisonId(1),
            left: SourceSnapshot {
                source_id: SourceId(1),
                generation: SourceGeneration(2),
            },
            right: SourceSnapshot {
                source_id: SourceId(3),
                generation: SourceGeneration(4),
            },
        };
        let mut archaeology = ComparisonArchaeology::new(pair);
        let right = ExactProvenance {
            source_id: SourceId(3),
            generation: SourceGeneration(4),
            ranges: ByteRangeSet {
                ranges: vec![ByteRange::new(40, 48).map_err(|_| WorkbenchError::InvalidRanges)?],
            },
        };
        archaeology.add_region(ComparisonRegion {
            id: ComparisonRegionId(1),
            classification: ComparisonClassification::New,
            left: None,
            right,
            explanation: "new trailer".to_owned(),
        })?;
        assert_eq!(archaeology.regions().len(), 1);

        let left = ExactProvenance {
            source_id: SourceId(1),
            generation: SourceGeneration(2),
            ranges: ByteRangeSet {
                ranges: vec![ByteRange::new(8, 16).map_err(|_| WorkbenchError::InvalidRanges)?],
            },
        };
        let different_location = ExactProvenance {
            source_id: SourceId(3),
            generation: SourceGeneration(4),
            ranges: ByteRangeSet {
                ranges: vec![ByteRange::new(16, 24).map_err(|_| WorkbenchError::InvalidRanges)?],
            },
        };
        assert_eq!(
            archaeology.add_region(ComparisonRegion {
                id: ComparisonRegionId(2),
                classification: ComparisonClassification::Unchanged,
                left: Some(left.clone()),
                right: different_location,
                explanation: "invalid unchanged move".to_owned(),
            }),
            Err(WorkbenchError::InvalidComparisonRegion)
        );
        let same_location = ExactProvenance {
            source_id: SourceId(3),
            generation: SourceGeneration(4),
            ranges: left.ranges.clone(),
        };
        assert_eq!(
            archaeology.add_region(ComparisonRegion {
                id: ComparisonRegionId(3),
                classification: ComparisonClassification::Moved,
                left: Some(left),
                right: same_location,
                explanation: "invalid same-offset move".to_owned(),
            }),
            Err(WorkbenchError::InvalidComparisonRegion)
        );
        Ok(())
    }

    #[test]
    fn cohort_lasso_materializes_exact_sample_membership() -> Result<(), WorkbenchError> {
        let source = SourceSnapshot {
            source_id: SourceId(1),
            generation: SourceGeneration(2),
        };
        let mut cohort = CohortModel::new(source);
        for offset in [3_u64, 9_u64] {
            cohort.add_sample(CohortSample {
                id: SampledByteId {
                    source_id: source.source_id,
                    generation: source.generation,
                    offset,
                },
                byte: 0x41,
                position: [0, 1, 2],
                factors: vec![CohortFactor {
                    name: "printable".to_owned(),
                    contribution: 1_000,
                }],
                provenance: provenance(offset, offset + 1)?,
            })?;
        }
        let members = vec![
            SampledByteId {
                source_id: source.source_id,
                generation: source.generation,
                offset: 9,
            },
            SampledByteId {
                source_id: source.source_id,
                generation: source.generation,
                offset: 3,
            },
        ];
        cohort.select_lasso(
            members,
            "two printable outliers".to_owned(),
            vec![CohortFactor {
                name: "lasso density".to_owned(),
                contribution: 500,
            }],
        )?;
        let selection = cohort.selection().ok_or(WorkbenchError::EmptySelection)?;
        assert_eq!(selection.member_ids.len(), 2);
        assert_eq!(selection.provenance.ranges.ranges[0].start, 3);
        assert_eq!(selection.provenance.ranges.ranges[1].start, 9);
        Ok(())
    }
}
