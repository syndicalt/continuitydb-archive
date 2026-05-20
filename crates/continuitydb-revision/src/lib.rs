//! Revision links and revision service.

use std::collections::HashMap;

use continuitydb_core::{
    ActivationState, Answerability, Confidence, SemanticAnchor, StateCell, StateCellId,
    UtilityFeedback,
};

pub use continuitydb_core::RevisionLinkKind;
use serde::{Deserialize, Serialize};

/// Append-only revision graph for StateCell version relationships.
#[derive(Default)]
pub struct RevisionGraph {
    links: HashMap<(StateCellId, RevisionLinkKind), Vec<StateCellId>>,
}

impl RevisionGraph {
    /// Records a directed revision link.
    pub fn link(&mut self, source: StateCellId, kind: RevisionLinkKind, target: StateCellId) {
        self.links.entry((source, kind)).or_default().push(target);
    }

    /// Returns all targets for a source and link kind.
    pub fn targets(&self, source: StateCellId, kind: RevisionLinkKind) -> Vec<StateCellId> {
        self.links.get(&(source, kind)).cloned().unwrap_or_default()
    }
}

/// Result of applying utility feedback as an append-only StateCell revision.
pub struct UtilityFeedbackRevision {
    /// New StateCell version carrying the revised utility feedback.
    pub cell: StateCell,
    /// Revision links connecting the new version to the prior version.
    pub revision: RevisionGraph,
}

/// Creates a successor StateCell version with updated utility feedback.
pub fn revise_utility_feedback(
    previous: &StateCell,
    feedback: UtilityFeedback,
) -> UtilityFeedbackRevision {
    let mut cell = previous.clone();
    cell.id = StateCellId::new();
    cell.utility_feedback = feedback;

    let mut revision = RevisionGraph::default();
    revision.link(cell.id, RevisionLinkKind::Supersedes, previous.id);
    revision.link(cell.id, RevisionLinkKind::Predecessor, previous.id);

    UtilityFeedbackRevision { cell, revision }
}

/// Result of applying an activation-state change as an append-only StateCell revision.
pub struct ActivationStateRevision {
    /// New StateCell version carrying the revised activation state.
    pub cell: StateCell,
    /// Revision links connecting the new version to the prior version.
    pub revision: RevisionGraph,
}

/// Creates a successor StateCell version with updated activation state.
pub fn revise_activation_state(
    previous: &StateCell,
    activation: ActivationState,
) -> ActivationStateRevision {
    let mut cell = previous.clone();
    cell.id = StateCellId::new();
    cell.activation = activation;

    let mut revision = RevisionGraph::default();
    revision.link(cell.id, RevisionLinkKind::Supersedes, previous.id);
    revision.link(cell.id, RevisionLinkKind::Predecessor, previous.id);

    ActivationStateRevision { cell, revision }
}

/// Result of applying answerability labels as an append-only StateCell revision.
pub struct AnswerabilityRevision {
    /// New StateCell version carrying the revised answerability labels.
    pub cell: StateCell,
    /// Revision links connecting the new version to the prior version.
    pub revision: RevisionGraph,
}

/// Creates a successor StateCell version with updated answerability labels.
pub fn revise_answerability(
    previous: &StateCell,
    answerability: Answerability,
) -> AnswerabilityRevision {
    let mut cell = previous.clone();
    cell.id = StateCellId::new();
    cell.answerability = answerability;

    let mut revision = RevisionGraph::default();
    revision.link(cell.id, RevisionLinkKind::Supersedes, previous.id);
    revision.link(cell.id, RevisionLinkKind::Predecessor, previous.id);

    AnswerabilityRevision { cell, revision }
}

/// Result of applying evidence confidence as an append-only StateCell revision.
pub struct EvidenceConfidenceRevision {
    /// New StateCell version carrying revised evidence confidence.
    pub cell: StateCell,
    /// Revision links connecting the new version to the prior version.
    pub revision: RevisionGraph,
}

/// Creates a successor StateCell version with all evidence confidence replaced.
pub fn revise_evidence_confidence(
    previous: &StateCell,
    confidence: Confidence,
) -> EvidenceConfidenceRevision {
    let mut cell = previous.clone();
    cell.id = StateCellId::new();
    for evidence in &mut cell.evidence {
        evidence.confidence = confidence;
    }

    let mut revision = RevisionGraph::default();
    revision.link(cell.id, RevisionLinkKind::Supersedes, previous.id);
    revision.link(cell.id, RevisionLinkKind::Predecessor, previous.id);

    EvidenceConfidenceRevision { cell, revision }
}

/// Deterministic reason two StateCell versions conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CellConflictKind {
    /// Cells share meaning and valid time but carry incompatible payloads.
    PayloadMismatch,
}

/// Deterministic conflict metadata between two StateCell versions.
pub struct CellConflict {
    /// Left conflicting cell identifier.
    pub left: StateCellId,
    /// Right conflicting cell identifier.
    pub right: StateCellId,
    /// Conflict kind.
    pub kind: CellConflictKind,
    /// Shared semantic anchor that caused the conflict check to apply.
    pub shared_anchor: SemanticAnchor,
    /// Revision links recording the conflict.
    pub revision: RevisionGraph,
}

/// Detects the first deterministic conflict between two StateCell versions.
pub fn detect_cell_conflict(left: &StateCell, right: &StateCell) -> Option<CellConflict> {
    let shared_anchor = left
        .anchors
        .iter()
        .find(|left_anchor| {
            right
                .anchors
                .iter()
                .any(|right_anchor| right_anchor == *left_anchor)
        })?
        .clone();

    if !left.valid_time.overlaps(&right.valid_time) || left.payload == right.payload {
        return None;
    }

    let mut revision = RevisionGraph::default();
    revision.link(left.id, RevisionLinkKind::ConflictsWith, right.id);
    revision.link(right.id, RevisionLinkKind::ConflictsWith, left.id);

    Some(CellConflict {
        left: left.id,
        right: right.id,
        kind: CellConflictKind::PayloadMismatch,
        shared_anchor,
        revision,
    })
}

/// Deterministic conflict scan over a candidate StateCell set.
pub struct CellConflictScan {
    /// Pairwise conflicts found in deterministic input-pair order.
    pub conflicts: Vec<CellConflict>,
    /// Aggregate revision links for all detected conflicts.
    pub revision: RevisionGraph,
}

/// Finds all deterministic conflicts across unordered pairs in input order.
pub fn scan_cell_conflicts(cells: &[StateCell]) -> CellConflictScan {
    let mut conflicts = Vec::new();
    let mut revision = RevisionGraph::default();

    for (left_index, left) in cells.iter().enumerate() {
        for right in cells.iter().skip(left_index + 1) {
            if let Some(conflict) = detect_cell_conflict(left, right) {
                revision.link(
                    conflict.left,
                    RevisionLinkKind::ConflictsWith,
                    conflict.right,
                );
                revision.link(
                    conflict.right,
                    RevisionLinkKind::ConflictsWith,
                    conflict.left,
                );
                conflicts.push(conflict);
            }
        }
    }

    CellConflictScan {
        conflicts,
        revision,
    }
}

/// Deterministic recommendation for resolving a detected StateCell conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConflictResolutionKind {
    /// Evidence strongly favors one cell superseding the other.
    CandidateSupersession,
    /// Evidence confidence is close enough that the later valid-time start wins.
    LatestEvidenceWins,
    /// Deterministic policy cannot safely pick a winner.
    NeedsHumanReview,
}

/// Non-mutating conflict resolution recommendation.
pub struct ConflictResolutionRecommendation {
    /// Conflict being evaluated.
    pub conflict: CellConflict,
    /// Recommended resolution class.
    pub kind: ConflictResolutionKind,
    /// Winning cell, when deterministic policy can choose one.
    pub winner: Option<StateCellId>,
    /// Losing cell, when deterministic policy can choose one.
    pub loser: Option<StateCellId>,
    /// Stable machine-readable reason for audit and policy decisions.
    pub reason: String,
    /// Proposed revision links for callers to accept or reject.
    pub revision: RevisionGraph,
}

/// Recommends a deterministic conflict resolution without mutating committed truth.
pub fn recommend_conflict_resolution(
    left: &StateCell,
    right: &StateCell,
) -> Option<ConflictResolutionRecommendation> {
    let conflict = detect_cell_conflict(left, right)?;
    let left_confidence = max_evidence_confidence(left);
    let right_confidence = max_evidence_confidence(right);
    let confidence_gap = (left_confidence - right_confidence).abs();

    if confidence_gap >= 0.20 {
        let (winner, loser) = if left_confidence > right_confidence {
            (left.id, right.id)
        } else {
            (right.id, left.id)
        };
        return Some(recommend_supersession(
            conflict,
            ConflictResolutionKind::CandidateSupersession,
            winner,
            loser,
            "resolution:confidence-gap",
        ));
    }

    if left.valid_time.from() != right.valid_time.from() {
        let (winner, loser) = if left.valid_time.from() > right.valid_time.from() {
            (left.id, right.id)
        } else {
            (right.id, left.id)
        };
        return Some(recommend_supersession(
            conflict,
            ConflictResolutionKind::LatestEvidenceWins,
            winner,
            loser,
            "resolution:latest-valid-time",
        ));
    }

    Some(ConflictResolutionRecommendation {
        conflict,
        kind: ConflictResolutionKind::NeedsHumanReview,
        winner: None,
        loser: None,
        reason: "resolution:human-review-required".to_string(),
        revision: RevisionGraph::default(),
    })
}

/// Deterministic conflict-resolution scan over a candidate StateCell set.
pub struct ConflictResolutionScan {
    /// Resolution recommendations in deterministic input-pair order.
    pub recommendations: Vec<ConflictResolutionRecommendation>,
    /// Aggregate reciprocal conflict links for all recommended conflicts.
    pub conflicts: RevisionGraph,
    /// Aggregate proposed revision links from recommendations.
    pub proposed_revisions: RevisionGraph,
}

/// Recommends deterministic resolutions for all conflicts across unordered pairs.
pub fn recommend_conflict_resolutions(cells: &[StateCell]) -> ConflictResolutionScan {
    let mut recommendations = Vec::new();
    let mut conflicts = RevisionGraph::default();
    let mut proposed_revisions = RevisionGraph::default();

    for (left_index, left) in cells.iter().enumerate() {
        for right in cells.iter().skip(left_index + 1) {
            if let Some(recommendation) = recommend_conflict_resolution(left, right) {
                conflicts.link(
                    recommendation.conflict.left,
                    RevisionLinkKind::ConflictsWith,
                    recommendation.conflict.right,
                );
                conflicts.link(
                    recommendation.conflict.right,
                    RevisionLinkKind::ConflictsWith,
                    recommendation.conflict.left,
                );

                if let (Some(winner), Some(loser)) = (recommendation.winner, recommendation.loser) {
                    proposed_revisions.link(winner, RevisionLinkKind::Supersedes, loser);
                }

                recommendations.push(recommendation);
            }
        }
    }

    ConflictResolutionScan {
        recommendations,
        conflicts,
        proposed_revisions,
    }
}

fn recommend_supersession(
    conflict: CellConflict,
    kind: ConflictResolutionKind,
    winner: StateCellId,
    loser: StateCellId,
    reason: &str,
) -> ConflictResolutionRecommendation {
    let mut revision = RevisionGraph::default();
    revision.link(winner, RevisionLinkKind::Supersedes, loser);

    ConflictResolutionRecommendation {
        conflict,
        kind,
        winner: Some(winner),
        loser: Some(loser),
        reason: reason.to_string(),
        revision,
    }
}

fn max_evidence_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellPayload, Citation, Confidence, Evidence,
        Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, UtilityFeedback,
        ValidTimeRange,
    };

    use super::{
        detect_cell_conflict, recommend_conflict_resolution, recommend_conflict_resolutions,
        revise_activation_state, revise_answerability, revise_evidence_confidence,
        revise_utility_feedback, scan_cell_conflicts, CellConflictKind, ConflictResolutionKind,
        RevisionGraph, RevisionLinkKind,
    };

    fn timestamp(day: u32) -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(2026, 5, day, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn sample_cell() -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:feedback",
            "Feedback revision target.",
            20,
            None,
        )
    }

    fn sample_cell_with_anchor_payload_and_time(
        anchor: &str,
        payload: &str,
        from_day: u32,
        to_day: Option<u32>,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_anchor_payload_time_and_confidence(anchor, payload, from_day, to_day, 0.8)
    }

    fn sample_cell_with_anchor_payload_time_and_confidence(
        anchor: &str,
        payload: &str,
        from_day: u32,
        to_day: Option<u32>,
        confidence: f32,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = timestamp(from_day)?;
        let valid_to = to_day.map(timestamp).transpose()?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, valid_to)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what feedback applies?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: "test://feedback".to_string(),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(payload.to_string()),
            CellCost::new(5, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn revision_graph_records_supersession_and_conflict_links() {
        let previous = StateCellId::new();
        let current = StateCellId::new();
        let conflict = StateCellId::new();
        let mut graph = RevisionGraph::default();

        graph.link(current, RevisionLinkKind::Supersedes, previous);
        graph.link(current, RevisionLinkKind::ConflictsWith, conflict);

        assert_eq!(
            graph.targets(current, RevisionLinkKind::Supersedes),
            vec![previous]
        );
        assert_eq!(
            graph.targets(current, RevisionLinkKind::ConflictsWith),
            vec![conflict]
        );
    }

    #[test]
    fn revision_link_kind_reexport_remains_available() {
        let kind: RevisionLinkKind = continuitydb_core::RevisionLinkKind::DerivesFrom;

        assert_eq!(kind, RevisionLinkKind::DerivesFrom);
    }

    #[test]
    fn utility_feedback_revision_creates_successor_with_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = sample_cell()?;
        let feedback = UtilityFeedback::new(
            Confidence::new(0.9)?,
            Confidence::new(0.8)?,
            Confidence::new(0.7)?,
        );

        let revised = revise_utility_feedback(&previous, feedback);

        assert_ne!(revised.cell.id, previous.id);
        assert_eq!(revised.cell.utility_feedback, feedback);
        assert_eq!(revised.cell.anchors, previous.anchors);
        assert_eq!(revised.cell.valid_time, previous.valid_time);
        assert_eq!(revised.cell.scope, previous.scope);
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Supersedes),
            vec![previous.id]
        );
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Predecessor),
            vec![previous.id]
        );
        Ok(())
    }

    #[test]
    fn activation_revision_creates_successor_with_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = sample_cell()?;

        let revised = revise_activation_state(&previous, ActivationState::Frontier);

        assert_ne!(revised.cell.id, previous.id);
        assert_eq!(previous.activation, ActivationState::Active);
        assert_eq!(revised.cell.activation, ActivationState::Frontier);
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Supersedes),
            vec![previous.id]
        );
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Predecessor),
            vec![previous.id]
        );
        Ok(())
    }

    #[test]
    fn answerability_revision_creates_successor_with_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = sample_cell()?;
        let answerability = Answerability::new(vec![
            "what changed?".to_string(),
            "what needs review?".to_string(),
        ])?;

        let revised = revise_answerability(&previous, answerability.clone());

        assert_ne!(revised.cell.id, previous.id);
        assert_eq!(
            previous.answerability.questions(),
            &["what feedback applies?".to_string()]
        );
        assert_eq!(revised.cell.answerability, answerability);
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Supersedes),
            vec![previous.id]
        );
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Predecessor),
            vec![previous.id]
        );
        Ok(())
    }

    #[test]
    fn evidence_confidence_revision_creates_successor_with_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:confidence-revision",
            "Confidence revision target.",
            20,
            None,
            0.4,
        )?;
        let confidence = Confidence::new(0.85)?;

        let revised = revise_evidence_confidence(&previous, confidence);

        assert_ne!(revised.cell.id, previous.id);
        assert_eq!(previous.evidence[0].confidence, Confidence::new(0.4)?);
        assert_eq!(revised.cell.evidence[0].confidence, confidence);
        assert_eq!(revised.cell.evidence[0].source, previous.evidence[0].source);
        assert_eq!(
            revised.cell.evidence[0].citation,
            previous.evidence[0].citation
        );
        assert_eq!(revised.cell.evidence[0].trust, previous.evidence[0].trust);
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Supersedes),
            vec![previous.id]
        );
        assert_eq!(
            revised
                .revision
                .targets(revised.cell.id, RevisionLinkKind::Predecessor),
            vec![previous.id]
        );
        Ok(())
    }

    #[test]
    fn conflict_detection_identifies_same_anchor_overlapping_payload_mismatch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let left = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is green.",
            20,
            Some(22),
        )?;
        let right = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is blocked.",
            21,
            Some(23),
        )?;

        let conflict = detect_cell_conflict(&left, &right)
            .ok_or_else(|| std::io::Error::other("expected conflict"))?;

        assert_eq!(conflict.left, left.id);
        assert_eq!(conflict.right, right.id);
        assert_eq!(conflict.kind, CellConflictKind::PayloadMismatch);
        assert_eq!(
            conflict.shared_anchor.as_str(),
            "project:continuitydb:release-status"
        );
        assert_eq!(
            conflict
                .revision
                .targets(left.id, RevisionLinkKind::ConflictsWith),
            vec![right.id]
        );
        assert_eq!(
            conflict
                .revision
                .targets(right.id, RevisionLinkKind::ConflictsWith),
            vec![left.id]
        );
        Ok(())
    }

    #[test]
    fn conflict_detection_ignores_adjacent_valid_time_ranges(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let left = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is green.",
            20,
            Some(21),
        )?;
        let right = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is blocked.",
            21,
            Some(22),
        )?;

        assert!(detect_cell_conflict(&left, &right).is_none());
        Ok(())
    }

    #[test]
    fn conflict_scan_reports_each_conflicting_pair_once() -> Result<(), Box<dyn std::error::Error>>
    {
        let left = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is green.",
            20,
            Some(22),
        )?;
        let right = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is blocked.",
            21,
            Some(23),
        )?;
        let adjacent = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:release-status",
            "Release is ready for handoff.",
            23,
            Some(24),
        )?;
        let unrelated = sample_cell_with_anchor_payload_and_time(
            "project:continuitydb:roadmap",
            "Roadmap is current.",
            20,
            Some(24),
        )?;

        let scan = scan_cell_conflicts(&[left.clone(), right.clone(), adjacent, unrelated]);

        assert_eq!(scan.conflicts.len(), 1);
        assert_eq!(scan.conflicts[0].left, left.id);
        assert_eq!(scan.conflicts[0].right, right.id);
        assert_eq!(
            scan.revision
                .targets(left.id, RevisionLinkKind::ConflictsWith),
            vec![right.id]
        );
        assert_eq!(
            scan.revision
                .targets(right.id, RevisionLinkKind::ConflictsWith),
            vec![left.id]
        );
        Ok(())
    }

    #[test]
    fn conflict_resolution_recommends_candidate_supersession_for_confidence_gap(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let low_confidence = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is blocked.",
            20,
            Some(23),
            0.55,
        )?;
        let high_confidence = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is green.",
            21,
            Some(23),
            0.9,
        )?;

        let recommendation = recommend_conflict_resolution(&low_confidence, &high_confidence)
            .ok_or_else(|| std::io::Error::other("expected recommendation"))?;

        assert_eq!(
            recommendation.kind,
            ConflictResolutionKind::CandidateSupersession
        );
        assert_eq!(recommendation.winner, Some(high_confidence.id));
        assert_eq!(recommendation.loser, Some(low_confidence.id));
        assert_eq!(
            recommendation
                .revision
                .targets(high_confidence.id, RevisionLinkKind::Supersedes),
            vec![low_confidence.id]
        );
        Ok(())
    }

    #[test]
    fn conflict_resolution_recommends_latest_evidence_when_confidence_is_close(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let older = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is blocked.",
            20,
            Some(23),
            0.82,
        )?;
        let newer = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is green.",
            21,
            Some(23),
            0.8,
        )?;

        let recommendation = recommend_conflict_resolution(&older, &newer)
            .ok_or_else(|| std::io::Error::other("expected recommendation"))?;

        assert_eq!(
            recommendation.kind,
            ConflictResolutionKind::LatestEvidenceWins
        );
        assert_eq!(recommendation.winner, Some(newer.id));
        assert_eq!(recommendation.loser, Some(older.id));
        Ok(())
    }

    #[test]
    fn conflict_resolution_requires_human_review_for_tied_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let left = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is blocked.",
            20,
            Some(23),
            0.8,
        )?;
        let right = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is green.",
            20,
            Some(23),
            0.8,
        )?;

        let recommendation = recommend_conflict_resolution(&left, &right)
            .ok_or_else(|| std::io::Error::other("expected recommendation"))?;

        assert_eq!(
            recommendation.kind,
            ConflictResolutionKind::NeedsHumanReview
        );
        assert_eq!(recommendation.winner, None);
        assert_eq!(recommendation.loser, None);
        assert!(recommendation
            .revision
            .targets(left.id, RevisionLinkKind::Supersedes)
            .is_empty());
        Ok(())
    }

    #[test]
    fn conflict_resolution_scan_recommends_each_detected_conflict(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let low_confidence = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is blocked.",
            20,
            Some(23),
            0.55,
        )?;
        let high_confidence = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:release-status",
            "Release is green.",
            21,
            Some(23),
            0.9,
        )?;
        let older = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:roadmap-status",
            "Roadmap is stale.",
            20,
            Some(23),
            0.8,
        )?;
        let newer = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:roadmap-status",
            "Roadmap is current.",
            21,
            Some(23),
            0.79,
        )?;
        let unrelated = sample_cell_with_anchor_payload_time_and_confidence(
            "project:continuitydb:storage",
            "Storage milestone is separate.",
            20,
            Some(23),
            0.95,
        )?;

        let scan = recommend_conflict_resolutions(&[
            low_confidence.clone(),
            high_confidence.clone(),
            older.clone(),
            newer.clone(),
            unrelated,
        ]);

        assert_eq!(scan.recommendations.len(), 2);
        assert_eq!(
            scan.recommendations[0].kind,
            ConflictResolutionKind::CandidateSupersession
        );
        assert_eq!(scan.recommendations[0].winner, Some(high_confidence.id));
        assert_eq!(
            scan.recommendations[1].kind,
            ConflictResolutionKind::LatestEvidenceWins
        );
        assert_eq!(scan.recommendations[1].winner, Some(newer.id));
        assert_eq!(
            scan.conflicts
                .targets(low_confidence.id, RevisionLinkKind::ConflictsWith),
            vec![high_confidence.id]
        );
        assert_eq!(
            scan.proposed_revisions
                .targets(high_confidence.id, RevisionLinkKind::Supersedes),
            vec![low_confidence.id]
        );
        assert_eq!(
            scan.proposed_revisions
                .targets(newer.id, RevisionLinkKind::Supersedes),
            vec![older.id]
        );
        Ok(())
    }
}
