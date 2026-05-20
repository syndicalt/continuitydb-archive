//! Revision links and revision service.

use std::collections::HashMap;

use continuitydb_core::{SemanticAnchor, StateCell, StateCellId, UtilityFeedback};
use serde::{Deserialize, Serialize};

/// Relationship between two StateCell versions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RevisionLinkKind {
    /// Current cell directly follows a previous version.
    Predecessor,
    /// Current cell supersedes the target.
    Supersedes,
    /// Current cell conflicts with the target.
    ConflictsWith,
    /// Current cell was derived from the target.
    DerivesFrom,
}

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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, UtilityFeedback,
        ValidTimeRange,
    };

    use super::{
        detect_cell_conflict, revise_utility_feedback, CellConflictKind, RevisionGraph,
        RevisionLinkKind,
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
                confidence: Confidence::new(0.8)?,
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
}
