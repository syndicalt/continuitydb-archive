//! Revision links and revision service.

use std::collections::HashMap;

use continuitydb_core::{StateCell, StateCellId, UtilityFeedback};
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, UtilityFeedback,
        ValidTimeRange,
    };

    use super::{revise_utility_feedback, RevisionGraph, RevisionLinkKind};

    fn sample_cell() -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new("project:continuitydb:feedback")],
            ValidTimeRange::new(valid_from, None)?,
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
            CellPayload::Text("Feedback revision target.".to_string()),
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
}
