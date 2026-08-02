//! Pure, UI-independent investigation state for the Strata POC.
//!
//! Callers provide stable IDs rather than asking this module to generate them. All records carry
//! exact source, generation, and half-open byte-range provenance; a view cannot promote an
//! aggregate-only visual observation into this model without first materializing its ranges.

use strata_core::{ByteRangeSet, EvidenceId, SourceGeneration, SourceId, ViewId};

/// Stable identity assigned by the caller to one machine- or analyst-originated finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FindingId(pub u128);

/// Stable identity assigned by the caller to one explicit relationship between findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CorrelationId(pub u128);

/// Stable identity assigned by the caller to one analyst hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HypothesisId(pub u128);

/// Exact immutable source context for a finding, evidence record, selection, or navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactProvenance {
    /// Identity of the source snapshot that supplied the bytes.
    pub source_id: SourceId,
    /// Generation of the source snapshot that supplied the bytes.
    pub generation: SourceGeneration,
    /// Non-empty, ordered, non-overlapping exact byte ranges.
    pub ranges: ByteRangeSet,
}

/// Origin and confidence state of a finding before it is promoted to evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingStatus {
    /// An analyzer or analyst has observed a candidate pattern.
    Candidate,
    /// The analyst has promoted this finding into an explicit evidence record.
    Promoted,
    /// The analyst has inspected and dismissed this finding.
    Dismissed,
}

/// A listable observation tied to exact source ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Stable caller-assigned finding identity.
    pub id: FindingId,
    /// Short display title.
    pub title: String,
    /// Concise human-readable detail or analyzer output.
    pub detail: String,
    /// Current analyst disposition.
    pub status: FindingStatus,
    /// Exact source provenance for this observation.
    pub provenance: ExactProvenance,
}

/// Analyst-owned evidence with a stable core evidence ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// Stable evidence identity, compatible with the domain/session model.
    pub id: EvidenceId,
    /// Short analyst-authored claim.
    pub claim: String,
    /// Supporting exact source ranges.
    pub provenance: ExactProvenance,
    /// Finding that was promoted, if this evidence originated from a finding.
    pub finding_id: Option<FindingId>,
}

/// Correlation strength as stated by the analyst or deterministic analyzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelationStrength {
    /// A potentially useful relationship that needs confirmation.
    Candidate,
    /// A repeatable relationship supported by the linked findings.
    Corroborated,
    /// A relationship that was investigated and rejected.
    Rejected,
}

/// A relationship between at least two known findings over an exact source range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correlation {
    /// Stable caller-assigned correlation identity.
    pub id: CorrelationId,
    /// Findings participating in this relationship, in display order.
    pub finding_ids: Vec<FindingId>,
    /// Exact source ranges in which the relationship was observed.
    pub provenance: ExactProvenance,
    /// Current assessment of the relationship.
    pub strength: CorrelationStrength,
    /// Analyst or analyzer rationale.
    pub rationale: String,
}

/// Current state of a reversible or investigatory hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisStatus {
    /// Formulated but not yet tested.
    Draft,
    /// A test has been run but is not yet conclusive.
    Tested,
    /// Evidence supports the hypothesis without making it a fact beyond scope.
    Supported,
    /// Evidence contradicts the hypothesis.
    Rejected,
}

/// An analyst hypothesis tied to exact bytes and optional supporting evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hypothesis {
    /// Stable caller-assigned hypothesis identity.
    pub id: HypothesisId,
    /// Testable statement such as "lane 2 contains a six-byte record field".
    pub statement: String,
    /// Exact source ranges to which this hypothesis applies.
    pub provenance: ExactProvenance,
    /// Analyst-maintained state; this is never inferred from a visual alone.
    pub status: HypothesisStatus,
    /// Evidence records explicitly cited in support of or against this hypothesis.
    pub evidence_ids: Vec<EvidenceId>,
}

/// Exact current selection, optionally originating from an evidence record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionState {
    /// Selected evidence record when the selection originated from the evidence list.
    pub evidence_id: Option<EvidenceId>,
    /// Exact selected source ranges.
    pub provenance: ExactProvenance,
}

/// State required to navigate an exact selection from one view to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossViewNavigation {
    /// View that initiated the navigation, if any.
    pub origin_view: Option<ViewId>,
    /// View that should receive focus.
    pub target_view: ViewId,
    /// Exact source ranges to focus in the target view.
    pub provenance: ExactProvenance,
    /// Evidence record that supplied this navigation, if any.
    pub evidence_id: Option<EvidenceId>,
}

/// Errors returned when a state transition would lose identity or exact provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvestigationError {
    /// A caller attempted to add a record whose stable ID is already present.
    DuplicateId,
    /// A record has no exact byte ranges.
    EmptyRanges,
    /// A record contains an empty, unordered, or overlapping range set.
    InvalidRanges,
    /// A correlation referenced fewer than two findings or repeated a finding.
    InvalidCorrelationMembers,
    /// A referenced finding is absent from this model.
    UnknownFinding(FindingId),
    /// A referenced evidence record is absent from this model.
    UnknownEvidence(EvidenceId),
    /// A referenced correlation is absent from this model.
    UnknownCorrelation(CorrelationId),
    /// A referenced hypothesis is absent from this model.
    UnknownHypothesis(HypothesisId),
    /// A link claimed one source snapshot while referring to another.
    ProvenanceMismatch,
    /// Promotion requires evidence to identify the finding it is promoting.
    PromotionRequiresFinding,
}

impl core::fmt::Display for InvestigationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InvestigationError {}

/// Pure investigation state intended for a GUI, CLI, or test harness to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvestigationModel {
    findings: Vec<Finding>,
    evidence: Vec<Evidence>,
    correlations: Vec<Correlation>,
    hypotheses: Vec<Hypothesis>,
    selection: Option<SelectionState>,
    navigation: Option<CrossViewNavigation>,
}

impl InvestigationModel {
    /// Creates empty investigation state with no implicit source or UI dependencies.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            findings: Vec::new(),
            evidence: Vec::new(),
            correlations: Vec::new(),
            hypotheses: Vec::new(),
            selection: None,
            navigation: None,
        }
    }

    /// Returns findings in their insertion order for a deterministic findings list.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Looks up one finding by its stable caller-assigned ID.
    #[must_use]
    pub fn finding(&self, id: FindingId) -> Option<&Finding> {
        self.findings.iter().find(|finding| finding.id == id)
    }

    /// Returns evidence records in their insertion order.
    #[must_use]
    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    /// Returns correlations in their insertion order.
    #[must_use]
    pub fn correlations(&self) -> &[Correlation] {
        &self.correlations
    }

    /// Looks up one correlation by its stable caller-assigned ID.
    #[must_use]
    pub fn correlation(&self, id: CorrelationId) -> Option<&Correlation> {
        self.correlations
            .iter()
            .find(|correlation| correlation.id == id)
    }

    /// Returns hypotheses in their insertion order.
    #[must_use]
    pub fn hypotheses(&self) -> &[Hypothesis] {
        &self.hypotheses
    }

    /// Looks up one hypothesis by its stable caller-assigned ID.
    #[must_use]
    pub fn hypothesis(&self, id: HypothesisId) -> Option<&Hypothesis> {
        self.hypotheses
            .iter()
            .find(|hypothesis| hypothesis.id == id)
    }

    /// Returns the current exact selection, if one is active.
    #[must_use]
    pub const fn selection(&self) -> Option<&SelectionState> {
        self.selection.as_ref()
    }

    /// Returns the current cross-view navigation request, if one is active.
    #[must_use]
    pub const fn navigation(&self) -> Option<&CrossViewNavigation> {
        self.navigation.as_ref()
    }

    /// Adds a finding after validating its stable identity and exact provenance.
    ///
    /// # Errors
    ///
    /// Returns an error for a duplicate identity or empty, unordered, or overlapping ranges.
    pub fn add_finding(&mut self, finding: Finding) -> Result<(), InvestigationError> {
        validate_provenance(&finding.provenance)?;
        if self
            .findings
            .iter()
            .any(|existing| existing.id == finding.id)
        {
            return Err(InvestigationError::DuplicateId);
        }
        self.findings.push(finding);
        Ok(())
    }

    /// Adds analyst-owned evidence, optionally linked to a known source finding.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid provenance, a duplicate identity, an unknown finding, or a
    /// source-generation mismatch with the linked finding.
    pub fn add_evidence(&mut self, evidence: Evidence) -> Result<(), InvestigationError> {
        validate_provenance(&evidence.provenance)?;
        if self
            .evidence
            .iter()
            .any(|existing| existing.id == evidence.id)
        {
            return Err(InvestigationError::DuplicateId);
        }
        if let Some(finding_id) = evidence.finding_id {
            let finding = self
                .finding(finding_id)
                .ok_or(InvestigationError::UnknownFinding(finding_id))?;
            if !same_snapshot(&finding.provenance, &evidence.provenance) {
                return Err(InvestigationError::ProvenanceMismatch);
            }
        }
        self.evidence.push(evidence);
        Ok(())
    }

    /// Adds linked evidence and marks its source finding as promoted in one state transition.
    ///
    /// No model state changes when the evidence has an invalid range, duplicate ID, missing
    /// finding link, unknown finding, or a different source snapshot from its finding.
    ///
    /// # Errors
    ///
    /// Returns an error for any of those validation failures.
    pub fn promote_finding(&mut self, evidence: Evidence) -> Result<(), InvestigationError> {
        validate_provenance(&evidence.provenance)?;
        if self
            .evidence
            .iter()
            .any(|existing| existing.id == evidence.id)
        {
            return Err(InvestigationError::DuplicateId);
        }
        let finding_id = evidence
            .finding_id
            .ok_or(InvestigationError::PromotionRequiresFinding)?;
        let finding_index = self
            .findings
            .iter()
            .position(|finding| finding.id == finding_id)
            .ok_or(InvestigationError::UnknownFinding(finding_id))?;
        let Some(finding) = self.findings.get(finding_index) else {
            return Err(InvestigationError::UnknownFinding(finding_id));
        };
        if !same_snapshot(&finding.provenance, &evidence.provenance) {
            return Err(InvestigationError::ProvenanceMismatch);
        }
        let Some(finding) = self.findings.get_mut(finding_index) else {
            return Err(InvestigationError::UnknownFinding(finding_id));
        };
        finding.status = FindingStatus::Promoted;
        self.evidence.push(evidence);
        Ok(())
    }

    /// Changes a finding disposition without changing its exact provenance or stable ID.
    ///
    /// # Errors
    ///
    /// Returns [`InvestigationError::UnknownFinding`] when `id` is not present.
    pub fn set_finding_status(
        &mut self,
        id: FindingId,
        status: FindingStatus,
    ) -> Result<(), InvestigationError> {
        let finding = self
            .findings
            .iter_mut()
            .find(|finding| finding.id == id)
            .ok_or(InvestigationError::UnknownFinding(id))?;
        finding.status = status;
        Ok(())
    }

    /// Adds a correlation between at least two existing findings.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid provenance, duplicate correlation identity, fewer than two
    /// unique members, or a reference to an unknown finding.
    pub fn add_correlation(&mut self, correlation: Correlation) -> Result<(), InvestigationError> {
        validate_provenance(&correlation.provenance)?;
        if self
            .correlations
            .iter()
            .any(|existing| existing.id == correlation.id)
        {
            return Err(InvestigationError::DuplicateId);
        }
        if correlation.finding_ids.len() < 2 || has_duplicate_findings(&correlation.finding_ids) {
            return Err(InvestigationError::InvalidCorrelationMembers);
        }
        for finding_id in &correlation.finding_ids {
            self.finding(*finding_id)
                .ok_or(InvestigationError::UnknownFinding(*finding_id))?;
        }
        self.correlations.push(correlation);
        Ok(())
    }

    /// Adds a hypothesis and validates every cited evidence record.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid provenance, a duplicate identity, or unknown cited evidence.
    pub fn add_hypothesis(&mut self, hypothesis: Hypothesis) -> Result<(), InvestigationError> {
        validate_provenance(&hypothesis.provenance)?;
        if self
            .hypotheses
            .iter()
            .any(|existing| existing.id == hypothesis.id)
        {
            return Err(InvestigationError::DuplicateId);
        }
        for evidence_id in &hypothesis.evidence_ids {
            self.evidence_by_id(*evidence_id)
                .ok_or(InvestigationError::UnknownEvidence(*evidence_id))?;
        }
        self.hypotheses.push(hypothesis);
        Ok(())
    }

    /// Changes a hypothesis state without changing its claim, evidence references, or provenance.
    ///
    /// # Errors
    ///
    /// Returns [`InvestigationError::UnknownHypothesis`] when `id` is not present.
    pub fn set_hypothesis_status(
        &mut self,
        id: HypothesisId,
        status: HypothesisStatus,
    ) -> Result<(), InvestigationError> {
        let hypothesis = self
            .hypotheses
            .iter_mut()
            .find(|hypothesis| hypothesis.id == id)
            .ok_or(InvestigationError::UnknownHypothesis(id))?;
        hypothesis.status = status;
        Ok(())
    }

    /// Changes a correlation assessment without changing its findings or exact provenance.
    ///
    /// # Errors
    ///
    /// Returns [`InvestigationError::UnknownCorrelation`] when `id` is not present.
    pub fn set_correlation_strength(
        &mut self,
        id: CorrelationId,
        strength: CorrelationStrength,
    ) -> Result<(), InvestigationError> {
        let correlation = self
            .correlations
            .iter_mut()
            .find(|correlation| correlation.id == id)
            .ok_or(InvestigationError::UnknownCorrelation(id))?;
        correlation.strength = strength;
        Ok(())
    }

    /// Selects an evidence record and copies its exact provenance into the shared selection.
    ///
    /// # Errors
    ///
    /// Returns [`InvestigationError::UnknownEvidence`] when `evidence_id` is not present.
    pub fn select_evidence(&mut self, evidence_id: EvidenceId) -> Result<(), InvestigationError> {
        let evidence = self
            .evidence_by_id(evidence_id)
            .ok_or(InvestigationError::UnknownEvidence(evidence_id))?;
        self.selection = Some(SelectionState {
            evidence_id: Some(evidence.id),
            provenance: evidence.provenance.clone(),
        });
        Ok(())
    }

    /// Selects exact ranges not yet promoted to evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied provenance has empty, unordered, or overlapping ranges.
    pub fn select_ranges(&mut self, provenance: ExactProvenance) -> Result<(), InvestigationError> {
        validate_provenance(&provenance)?;
        self.selection = Some(SelectionState {
            evidence_id: None,
            provenance,
        });
        Ok(())
    }

    /// Publishes an exact cross-view navigation request and makes it the active selection.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid provenance, unknown linked evidence, or a source-generation
    /// mismatch between the evidence and navigation target.
    pub fn navigate(&mut self, navigation: CrossViewNavigation) -> Result<(), InvestigationError> {
        validate_provenance(&navigation.provenance)?;
        if let Some(evidence_id) = navigation.evidence_id {
            let evidence = self
                .evidence_by_id(evidence_id)
                .ok_or(InvestigationError::UnknownEvidence(evidence_id))?;
            if !same_snapshot(&evidence.provenance, &navigation.provenance) {
                return Err(InvestigationError::ProvenanceMismatch);
            }
        }
        self.selection = Some(SelectionState {
            evidence_id: navigation.evidence_id,
            provenance: navigation.provenance.clone(),
        });
        self.navigation = Some(navigation);
        Ok(())
    }

    /// Clears only transient focus state; findings, evidence, and hypotheses remain intact.
    pub fn clear_navigation(&mut self) {
        self.navigation = None;
    }

    fn evidence_by_id(&self, id: EvidenceId) -> Option<&Evidence> {
        self.evidence.iter().find(|evidence| evidence.id == id)
    }
}

fn validate_provenance(provenance: &ExactProvenance) -> Result<(), InvestigationError> {
    let Some(first) = provenance.ranges.ranges.first() else {
        return Err(InvestigationError::EmptyRanges);
    };
    if first.is_empty() {
        return Err(InvestigationError::InvalidRanges);
    }
    let mut previous_end = first.end;
    for range in provenance.ranges.ranges.iter().skip(1) {
        if range.is_empty() || range.start < previous_end {
            return Err(InvestigationError::InvalidRanges);
        }
        previous_end = range.end;
    }
    Ok(())
}

fn has_duplicate_findings(finding_ids: &[FindingId]) -> bool {
    finding_ids
        .iter()
        .enumerate()
        .any(|(index, finding_id)| finding_ids[..index].contains(finding_id))
}

fn same_snapshot(first: &ExactProvenance, second: &ExactProvenance) -> bool {
    first.source_id == second.source_id && first.generation == second.generation
}

#[cfg(test)]
mod tests {
    use strata_core::{ByteRange, ByteRangeSet, EvidenceId, SourceGeneration, SourceId, ViewId};

    use super::{
        Correlation, CorrelationId, CorrelationStrength, CrossViewNavigation, Evidence,
        ExactProvenance, Finding, FindingId, FindingStatus, Hypothesis, HypothesisId,
        HypothesisStatus, InvestigationError, InvestigationModel,
    };

    fn provenance(start: u64, end: u64) -> Result<ExactProvenance, InvestigationError> {
        let range = ByteRange::new(start, end).map_err(|_| InvestigationError::InvalidRanges)?;
        Ok(ExactProvenance {
            source_id: SourceId(7),
            generation: SourceGeneration(3),
            ranges: ByteRangeSet {
                ranges: vec![range],
            },
        })
    }

    fn finding(id: u128, start: u64, end: u64) -> Result<Finding, InvestigationError> {
        Ok(Finding {
            id: FindingId(id),
            title: "Boundary".to_owned(),
            detail: "Entropy changes at this exact range".to_owned(),
            status: FindingStatus::Candidate,
            provenance: provenance(start, end)?,
        })
    }

    #[test]
    fn model_keeps_stable_findings_and_rejects_duplicates() -> Result<(), InvestigationError> {
        let mut model = InvestigationModel::new();
        model.add_finding(finding(1, 16, 32)?)?;
        assert_eq!(model.findings().len(), 1);
        assert_eq!(
            model.add_finding(finding(1, 48, 64)?),
            Err(InvestigationError::DuplicateId)
        );
        Ok(())
    }

    #[test]
    fn evidence_selection_and_navigation_keep_exact_provenance() -> Result<(), InvestigationError> {
        let mut model = InvestigationModel::new();
        model.add_finding(finding(1, 16, 32)?)?;
        let evidence = Evidence {
            id: EvidenceId(11),
            claim: "Candidate header boundary".to_owned(),
            provenance: provenance(16, 32)?,
            finding_id: Some(FindingId(1)),
        };
        model.add_evidence(evidence)?;
        model.select_evidence(EvidenceId(11))?;
        let selection = model.selection().ok_or(InvestigationError::EmptyRanges)?;
        assert_eq!(selection.evidence_id, Some(EvidenceId(11)));
        assert_eq!(selection.provenance.ranges.ranges[0].start, 16);

        let destination = CrossViewNavigation {
            origin_view: Some(ViewId(2)),
            target_view: ViewId(9),
            provenance: provenance(16, 32)?,
            evidence_id: Some(EvidenceId(11)),
        };
        model.navigate(destination)?;
        let navigation = model.navigation().ok_or(InvestigationError::EmptyRanges)?;
        assert_eq!(navigation.target_view, ViewId(9));
        assert_eq!(navigation.provenance.generation, SourceGeneration(3));
        assert_eq!(navigation.provenance.ranges.ranges[0].end, 32);
        Ok(())
    }

    #[test]
    fn correlations_and_hypotheses_only_reference_known_records() -> Result<(), InvestigationError>
    {
        let mut model = InvestigationModel::new();
        model.add_finding(finding(1, 0, 8)?)?;
        model.add_finding(finding(2, 8, 16)?)?;
        let correlation = Correlation {
            id: CorrelationId(5),
            finding_ids: vec![FindingId(1), FindingId(2)],
            provenance: provenance(0, 16)?,
            strength: CorrelationStrength::Candidate,
            rationale: "Two views report the same transition".to_owned(),
        };
        model.add_correlation(correlation)?;
        assert_eq!(model.correlations().len(), 1);

        let hypothesis = Hypothesis {
            id: HypothesisId(4),
            statement: "Records have an eight-byte period".to_owned(),
            provenance: provenance(0, 16)?,
            status: HypothesisStatus::Draft,
            evidence_ids: vec![EvidenceId(99)],
        };
        assert_eq!(
            model.add_hypothesis(hypothesis),
            Err(InvestigationError::UnknownEvidence(EvidenceId(99)))
        );
        Ok(())
    }

    #[test]
    fn model_rejects_empty_or_overlapping_exact_ranges() -> Result<(), InvestigationError> {
        let mut model = InvestigationModel::new();
        let empty = ExactProvenance {
            source_id: SourceId(7),
            generation: SourceGeneration(3),
            ranges: ByteRangeSet::default(),
        };
        assert_eq!(
            model.select_ranges(empty),
            Err(InvestigationError::EmptyRanges)
        );

        let first = ByteRange::new(0, 8).map_err(|_| InvestigationError::InvalidRanges)?;
        let second = ByteRange::new(4, 12).map_err(|_| InvestigationError::InvalidRanges)?;
        let overlapping = ExactProvenance {
            source_id: SourceId(7),
            generation: SourceGeneration(3),
            ranges: ByteRangeSet {
                ranges: vec![first, second],
            },
        };
        assert_eq!(
            model.select_ranges(overlapping),
            Err(InvestigationError::InvalidRanges)
        );
        Ok(())
    }

    #[test]
    fn analyst_transitions_preserve_ids_and_promote_findings_atomically()
    -> Result<(), InvestigationError> {
        let mut model = InvestigationModel::new();
        model.add_finding(finding(1, 0, 8)?)?;
        model.add_finding(finding(2, 8, 16)?)?;

        let unlinked_evidence = Evidence {
            id: EvidenceId(10),
            claim: "Cannot promote without a finding link".to_owned(),
            provenance: provenance(0, 8)?,
            finding_id: None,
        };
        assert_eq!(
            model.promote_finding(unlinked_evidence),
            Err(InvestigationError::PromotionRequiresFinding)
        );
        assert_eq!(model.evidence().len(), 0);
        assert_eq!(
            model.finding(FindingId(1)).map(|item| item.status),
            Some(FindingStatus::Candidate)
        );

        let evidence = Evidence {
            id: EvidenceId(11),
            claim: "Header boundary corroborated".to_owned(),
            provenance: provenance(0, 8)?,
            finding_id: Some(FindingId(1)),
        };
        model.promote_finding(evidence)?;
        assert_eq!(model.evidence().len(), 1);
        assert_eq!(
            model.finding(FindingId(1)).map(|item| item.status),
            Some(FindingStatus::Promoted)
        );

        let correlation = Correlation {
            id: CorrelationId(5),
            finding_ids: vec![FindingId(1), FindingId(2)],
            provenance: provenance(0, 16)?,
            strength: CorrelationStrength::Candidate,
            rationale: "Two exact findings meet at the boundary".to_owned(),
        };
        model.add_correlation(correlation)?;
        model.set_correlation_strength(CorrelationId(5), CorrelationStrength::Corroborated)?;
        assert_eq!(
            model
                .correlation(CorrelationId(5))
                .map(|item| item.strength),
            Some(CorrelationStrength::Corroborated)
        );

        let hypothesis = Hypothesis {
            id: HypothesisId(4),
            statement: "The first eight bytes are a header".to_owned(),
            provenance: provenance(0, 8)?,
            status: HypothesisStatus::Draft,
            evidence_ids: vec![EvidenceId(11)],
        };
        model.add_hypothesis(hypothesis)?;
        model.set_hypothesis_status(HypothesisId(4), HypothesisStatus::Supported)?;
        assert_eq!(
            model.hypothesis(HypothesisId(4)).map(|item| item.status),
            Some(HypothesisStatus::Supported)
        );

        model.set_finding_status(FindingId(2), FindingStatus::Dismissed)?;
        assert_eq!(
            model.finding(FindingId(2)).map(|item| item.status),
            Some(FindingStatus::Dismissed)
        );
        Ok(())
    }
}
