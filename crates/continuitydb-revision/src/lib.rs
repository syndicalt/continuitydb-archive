//! Revision links and revision service.

use std::collections::HashMap;

use continuitydb_core::StateCellId;
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

#[cfg(test)]
mod tests {
    use continuitydb_core::StateCellId;

    use super::{RevisionGraph, RevisionLinkKind};

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
}
