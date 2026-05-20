//! Deterministic checkout and audit.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, CoreError, RevisionLinkRecord,
    Scope, SemanticAnchor, StateCell, StateCellId, TrustSignal,
};
use continuitydb_kernel::{CellLookup, KernelError, RevisionLinkLookup, StorageKernel};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by checkout.
#[derive(Debug, Error, PartialEq)]
pub enum CheckoutError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout metadata could not be derived from selected cells.
    #[error(transparent)]
    Core(#[from] CoreError),
}

/// Request constraints for deterministic checkout.
#[derive(Clone, Debug)]
pub struct CheckoutRequest {
    /// Optional semantic anchor filter.
    pub semantic_anchor: Option<SemanticAnchor>,
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Optional system transaction-time filter.
    pub system_at: Option<DateTime<Utc>>,
    /// Optional database commit identifier filter.
    pub commit_id: Option<CommitId>,
    /// Optional activation-state filter.
    pub activation: Option<ActivationState>,
    /// Optional exact answerability question filter.
    pub answerability_question: Option<String>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
    /// Optional dependency target filter.
    pub dependency_target: Option<StateCellId>,
    /// Optional dependency kind filter.
    pub dependency_kind: Option<CellDependencyKind>,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

/// Materialized continuity slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutSlice {
    /// Selected cells.
    pub cells: Vec<StateCell>,
    /// Total estimated tokens.
    pub total_tokens: i64,
    /// Citation traces for selected cells.
    pub audit_traces: Vec<AuditTrace>,
    /// Per-cell uncertainty metadata for selected cells.
    pub uncertainty: Vec<UncertaintyEntry>,
    /// Selected frontier cells that should remain monitored.
    pub frontier_recommendations: Vec<FrontierRecommendation>,
    /// Candidate cells that matched constraints but were not selected.
    pub alternatives: Vec<CheckoutAlternative>,
}

/// Deterministic uncertainty metadata for a selected StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyEntry {
    /// Selected cell identifier.
    pub cell_id: StateCellId,
    /// Maximum confidence across the selected cell's evidence.
    pub max_confidence: Confidence,
}

/// Recommendation to keep a selected frontier StateCell under monitoring.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrontierRecommendation {
    /// Frontier cell identifier.
    pub cell_id: StateCellId,
    /// Citation locators supporting the frontier recommendation.
    pub citations: Vec<String>,
}

/// Deterministic reason a matching candidate was not selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckoutAlternativeReason {
    /// Candidate could not fit inside the remaining token budget.
    TokenBudgetExceeded,
}

/// Metadata for a matching candidate omitted from the selected slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutAlternative {
    /// Omitted candidate cell identifier.
    pub cell_id: StateCellId,
    /// Deterministic omission reason.
    pub reason: CheckoutAlternativeReason,
    /// Citation locators supporting the omitted candidate.
    pub citations: Vec<String>,
    /// Maximum confidence across the omitted candidate's evidence.
    pub max_confidence: Confidence,
}

/// Audit trace for a StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditTrace {
    /// Audited cell identifier.
    pub cell_id: StateCellId,
    /// Database commit boundary that wrote the audited cell.
    pub commit_id: CommitId,
    /// Citation locators supporting the cell.
    pub citations: Vec<String>,
    /// Structured evidence provenance supporting the cell.
    pub evidence: Vec<AuditEvidence>,
    /// Dependency and causality links referenced by the cell.
    pub dependencies: Vec<AuditDependency>,
    /// Native revision-link records where this cell participates.
    pub revision_links: Vec<RevisionLinkRecord>,
}

/// Evidence metadata included in an audit trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditEvidence {
    /// Stable evidence source identifier.
    pub source: String,
    /// Source-local citation URI, path, or durable locator.
    pub locator: String,
    /// Confidence assigned to this evidence.
    pub confidence: Confidence,
    /// Trust signals associated with this evidence.
    pub trust: Vec<TrustSignal>,
}

/// Dependency metadata included in an audit trace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditDependency {
    /// Target StateCell identifier.
    pub target: StateCellId,
    /// Dependency edge meaning.
    pub kind: CellDependencyKind,
    /// Human-readable rationale for the dependency.
    pub rationale: String,
}

/// Materializes a deterministic continuity slice.
pub fn checkout<K: StorageKernel>(
    kernel: &K,
    request: CheckoutRequest,
) -> Result<CheckoutSlice, CheckoutError> {
    let mut candidates = kernel.lookup_cells(CellLookup {
        semantic_anchor: request
            .semantic_anchor
            .as_ref()
            .map(|anchor| anchor.as_str().to_string()),
        scope: request.scope,
        valid_at: request.valid_at,
        system_at: request.system_at,
        commit_id: request.commit_id,
        activation: request.activation,
        answerability_question: request.answerability_question,
        evidence_source: request.evidence_source,
        dependency_target: request.dependency_target,
        dependency_kind: request.dependency_kind,
        minimum_confidence: Some(request.minimum_confidence),
        ..CellLookup::default()
    })?;

    candidates.retain(|cell| {
        cell.evidence
            .iter()
            .any(|evidence| evidence.confidence.value() >= request.minimum_confidence.value())
    });

    candidates.sort_by(|left, right| {
        checkout_score(right)
            .total_cmp(&checkout_score(left))
            .then_with(|| max_confidence(right).total_cmp(&max_confidence(left)))
            .then_with(|| first_anchor(left).cmp(first_anchor(right)))
    });

    let mut total_tokens = 0;
    let mut cells = Vec::new();
    let mut alternatives = Vec::new();
    for cell in candidates {
        let next_total = total_tokens + cell.cost.token_count;
        if next_total <= request.token_budget {
            total_tokens = next_total;
            cells.push(cell);
        } else {
            alternatives.push(CheckoutAlternative {
                cell_id: cell.id,
                reason: CheckoutAlternativeReason::TokenBudgetExceeded,
                citations: citations(&cell),
                max_confidence: Confidence::new(max_confidence(&cell))?,
            });
        }
    }

    let audit_traces = cells
        .iter()
        .map(|cell| {
            let mut trace = audit(cell);
            trace.revision_links = revision_links_for_cell(kernel, cell.id)?;
            Ok(trace)
        })
        .collect::<Result<Vec<_>, CheckoutError>>()?;
    let uncertainty = cells
        .iter()
        .map(|cell| {
            Ok(UncertaintyEntry {
                cell_id: cell.id,
                max_confidence: Confidence::new(max_confidence(cell))?,
            })
        })
        .collect::<Result<Vec<_>, CheckoutError>>()?;
    let frontier_recommendations = cells
        .iter()
        .filter(|cell| cell.activation == ActivationState::Frontier)
        .map(|cell| FrontierRecommendation {
            cell_id: cell.id,
            citations: citations(cell),
        })
        .collect();

    Ok(CheckoutSlice {
        cells,
        total_tokens,
        audit_traces,
        uncertainty,
        frontier_recommendations,
        alternatives,
    })
}

/// Produces a basic audit trace for a StateCell.
pub fn audit(cell: &StateCell) -> AuditTrace {
    AuditTrace {
        cell_id: cell.id,
        commit_id: cell.commit_id,
        citations: citations(cell),
        evidence: cell
            .evidence
            .iter()
            .map(|evidence| AuditEvidence {
                source: evidence.source.as_str().to_string(),
                locator: evidence.citation.locator.clone(),
                confidence: evidence.confidence,
                trust: evidence.trust.clone(),
            })
            .collect(),
        dependencies: cell
            .dependencies
            .iter()
            .map(|dependency| AuditDependency {
                target: dependency.target,
                kind: dependency.kind,
                rationale: dependency.rationale.clone(),
            })
            .collect(),
        revision_links: Vec::new(),
    }
}

fn revision_links_for_cell<K: StorageKernel>(
    kernel: &K,
    cell_id: StateCellId,
) -> Result<Vec<RevisionLinkRecord>, CheckoutError> {
    let mut links = kernel.list_revision_links(RevisionLinkLookup {
        source: Some(cell_id),
        ..RevisionLinkLookup::default()
    })?;
    for link in kernel.list_revision_links(RevisionLinkLookup {
        target: Some(cell_id),
        ..RevisionLinkLookup::default()
    })? {
        if !links.contains(&link) {
            links.push(link);
        }
    }
    Ok(links)
}

fn citations(cell: &StateCell) -> Vec<String> {
    cell.evidence
        .iter()
        .map(|evidence| evidence.citation.locator.clone())
        .collect()
}

fn max_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}

fn checkout_score(cell: &StateCell) -> f32 {
    (max_confidence(cell) + cell.utility_feedback.utility_score()) / 2.0
}

fn first_anchor(cell: &StateCell) -> &str {
    cell.anchors
        .first()
        .map(SemanticAnchor::as_str)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use chrono::{DateTime, TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
        Citation, CommitId, Confidence, Evidence, RevisionLinkKind, RevisionLinkRecord, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, SystemTimeRange, TrustSignal,
        UtilityFeedback, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
    use continuitydb_memory::MemoryKernel;

    use super::{audit, checkout, CheckoutRequest};

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_question_and_source(
            anchor,
            "what should the agent know?",
            "test",
            confidence,
            tokens,
        )
    }

    fn sample_cell_with_question_and_source(
        anchor: &str,
        question: &str,
        source: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec![question.to_string()])?,
            vec![Evidence {
                source: SourceId::new(source),
                citation: Citation {
                    locator: format!("test://{anchor}"),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[derive(Default)]
    struct RecordingKernel {
        lookup: RefCell<Option<CellLookup>>,
    }

    impl StorageKernel for RecordingKernel {
        fn append_cells_at_with_commit_id<I>(
            &mut self,
            _cells: I,
            _committed_at: DateTime<Utc>,
            _commit_id: CommitId,
        ) -> Result<(), KernelError>
        where
            I: IntoIterator<Item = StateCell>,
        {
            Ok(())
        }

        fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
            *self.lookup.borrow_mut() = Some(lookup);
            Ok(Vec::new())
        }

        fn lookup_commit_manifest(
            &self,
            _commit_id: CommitId,
        ) -> Result<Option<continuitydb_core::CommitManifest>, KernelError> {
            Ok(None)
        }

        fn list_commit_manifests(
            &self,
        ) -> Result<Vec<continuitydb_core::CommitManifest>, KernelError> {
            Ok(Vec::new())
        }

        fn list_commit_manifests_matching(
            &self,
            _lookup: continuitydb_kernel::CommitManifestLookup,
        ) -> Result<Vec<continuitydb_core::CommitManifest>, KernelError> {
            Ok(Vec::new())
        }

        fn append_revision_link(
            &mut self,
            _revision_link: continuitydb_core::RevisionLinkRecord,
        ) -> Result<(), KernelError> {
            Ok(())
        }

        fn list_revision_links(
            &self,
            _lookup: continuitydb_kernel::RevisionLinkLookup,
        ) -> Result<Vec<continuitydb_core::RevisionLinkRecord>, KernelError> {
            Ok(Vec::new())
        }
    }

    fn test_commit_time() -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn append_committed(
        kernel: &mut MemoryKernel,
        mut cell: StateCell,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        kernel.append_cell_at_with_commit_id(cell.clone(), committed_at, commit_id)?;
        cell.system_time = SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        Ok(cell)
    }

    #[test]
    fn checkout_pushes_semantic_constraints_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: Some("what is frontier?".to_string()),
                evidence_source: Some("human".to_string()),
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(
            lookup.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(
            lookup.answerability_question,
            Some("what is frontier?".to_string())
        );
        assert_eq!(lookup.evidence_source, Some("human".to_string()));
        assert_eq!(lookup.minimum_confidence, Some(Confidence::new(0.8)?));
        Ok(())
    }

    #[test]
    fn checkout_pushes_semantic_anchor_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:release-status")),
                scope: None,
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(
            lookup.semantic_anchor.as_deref(),
            Some("project:continuitydb:release-status")
        );
        Ok(())
    }

    #[test]
    fn checkout_pushes_dependency_constraints_to_kernel() -> Result<(), Box<dyn std::error::Error>>
    {
        let kernel = RecordingKernel::default();
        let target = StateCellId::new();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: Some(target),
                dependency_kind: Some(CellDependencyKind::DependsOn),
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.dependency_target, Some(target));
        assert_eq!(lookup.dependency_kind, Some(CellDependencyKind::DependsOn));
        Ok(())
    }

    #[test]
    fn checkout_pushes_activation_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: None,
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: Some(ActivationState::Frontier),
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.activation, Some(ActivationState::Frontier));
        Ok(())
    }

    #[test]
    fn checkout_pushes_system_time_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        let system_at = test_commit_time()?;
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: None,
                valid_at: None,
                system_at: Some(system_at),
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.system_at, Some(system_at));
        Ok(())
    }

    #[test]
    fn checkout_pushes_commit_id_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        let commit_id = CommitId::new();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: None,
                valid_at: None,
                system_at: None,
                commit_id: Some(commit_id),
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.commit_id, Some(commit_id));
        Ok(())
    }

    #[test]
    fn checkout_respects_token_budget_and_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let high = sample_cell("project:continuitydb:high", 0.95, 10)?;
        let low = sample_cell("project:continuitydb:low", 0.40, 10)?;
        let high = append_committed(&mut kernel, high)?;
        append_committed(&mut kernel, low)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![high]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_system_time() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let before_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 59, 59)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:system-checkout", 0.95, 10)?,
        )?;

        let historical = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: Some(before_commit),
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;
        let current = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: Some(committed_at),
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert!(historical.cells.is_empty());
        assert_eq!(current.cells, vec![cell]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_commit_id() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let selected_commit_id = CommitId::new();
        let omitted_commit_id = CommitId::new();
        let mut selected = sample_cell("project:continuitydb:selected-commit", 0.95, 10)?;
        let omitted = sample_cell("project:continuitydb:omitted-commit", 0.95, 10)?;
        kernel.append_cell_at_with_commit_id(selected.clone(), committed_at, selected_commit_id)?;
        kernel.append_cell_at_with_commit_id(omitted, committed_at, omitted_commit_id)?;
        selected.system_time = SystemTimeRange::open_from(committed_at);
        selected.commit_id = selected_commit_id;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: Some(selected_commit_id),
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_prefers_higher_utility_candidate_under_token_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut high_confidence_low_utility =
            sample_cell("project:continuitydb:confidence-only", 0.95, 10)?;
        high_confidence_low_utility.utility_feedback = UtilityFeedback::new(
            Confidence::new(0.1)?,
            Confidence::new(0.1)?,
            Confidence::new(0.1)?,
        );
        let mut lower_confidence_high_utility =
            sample_cell("project:continuitydb:useful", 0.80, 10)?;
        lower_confidence_high_utility.utility_feedback = UtilityFeedback::new(
            Confidence::new(1.0)?,
            Confidence::new(1.0)?,
            Confidence::new(1.0)?,
        );
        let high_confidence_low_utility =
            append_committed(&mut kernel, high_confidence_low_utility)?;
        let lower_confidence_high_utility =
            append_committed(&mut kernel, lower_confidence_high_utility)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![lower_confidence_high_utility]);
        assert_eq!(slice.alternatives.len(), 1);
        assert_eq!(
            slice.alternatives[0].cell_id,
            high_confidence_low_utility.id
        );
        Ok(())
    }

    #[test]
    fn checkout_filters_by_dependency_target_and_kind() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:dependent", 0.95, 10)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut unrelated = sample_cell("project:continuitydb:unrelated", 0.90, 10)?;
        unrelated.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on another target",
        ));
        let mut support = sample_cell("project:continuitydb:support", 0.85, 10)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        let dependent = append_committed(&mut kernel, dependent)?;
        append_committed(&mut kernel, unrelated)?;
        append_committed(&mut kernel, support)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: Some(target),
                dependency_kind: Some(CellDependencyKind::DependsOn),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![dependent]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_semantic_anchor() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = sample_cell("project:continuitydb:anchor-selected", 0.91, 10)?;
        let unrelated = sample_cell("project:continuitydb:anchor-unrelated", 0.9, 10)?;
        let selected = append_committed(&mut kernel, selected)?;
        append_committed(&mut kernel, unrelated)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:anchor-selected")),
                scope: None,
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_answerability_question() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let status = sample_cell_with_question_and_source(
            "project:continuitydb:status",
            "what is status?",
            "test",
            0.95,
            10,
        )?;
        let frontier = sample_cell_with_question_and_source(
            "project:continuitydb:frontier",
            "what is frontier?",
            "test",
            0.90,
            10,
        )?;
        append_committed(&mut kernel, status)?;
        let frontier = append_committed(&mut kernel, frontier)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: Some("what is frontier?".to_string()),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![frontier]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_evidence_source() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let observed = sample_cell_with_question_and_source(
            "project:continuitydb:observed",
            "what is status?",
            "sensor",
            0.95,
            10,
        )?;
        let reviewed = sample_cell_with_question_and_source(
            "project:continuitydb:reviewed",
            "what is status?",
            "human",
            0.90,
            10,
        )?;
        append_committed(&mut kernel, observed)?;
        let reviewed = append_committed(&mut kernel, reviewed)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: Some("human".to_string()),
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![reviewed]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_activation_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let active = sample_cell("project:continuitydb:activation-active", 0.91, 10)?;
        let mut frontier = sample_cell("project:continuitydb:activation-frontier", 0.9, 10)?;
        frontier.activation = ActivationState::Frontier;
        append_committed(&mut kernel, active)?;
        let frontier = append_committed(&mut kernel, frontier)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: Some(ActivationState::Frontier),
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![frontier]);
        Ok(())
    }

    #[test]
    fn checkout_slice_includes_citations_uncertainty_and_frontier_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let active = sample_cell("project:continuitydb:active", 0.95, 10)?;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.72, 10)?;
        frontier.activation = ActivationState::Frontier;
        let active = append_committed(&mut kernel, active)?;
        let frontier = append_committed(&mut kernel, frontier)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![active.clone(), frontier.clone()]);
        assert_eq!(slice.audit_traces[0].cell_id, active.id);
        assert_eq!(
            slice.audit_traces[0].citations,
            vec!["test://project:continuitydb:active".to_string()]
        );
        assert_eq!(slice.audit_traces[1].cell_id, frontier.id);
        assert_eq!(
            slice.audit_traces[1].citations,
            vec!["test://project:continuitydb:frontier".to_string()]
        );
        assert_eq!(slice.uncertainty[0].cell_id, active.id);
        assert_eq!(slice.uncertainty[0].max_confidence, Confidence::new(0.95)?);
        assert_eq!(slice.uncertainty[1].cell_id, frontier.id);
        assert_eq!(slice.uncertainty[1].max_confidence, Confidence::new(0.72)?);
        assert_eq!(slice.frontier_recommendations.len(), 1);
        assert_eq!(slice.frontier_recommendations[0].cell_id, frontier.id);
        assert_eq!(
            slice.frontier_recommendations[0].citations,
            vec!["test://project:continuitydb:frontier".to_string()]
        );
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_include_native_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-selected", 0.95, 10)?,
        )?;
        let superseded = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-superseded", 0.83, 10)?,
        )?;
        let predecessor = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-predecessor", 0.72, 10)?,
        )?;
        let committed_at = test_commit_time()?;
        let source_link = RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::Supersedes,
            superseded.id,
            committed_at,
        );
        let target_link = RevisionLinkRecord::new(
            predecessor.id,
            RevisionLinkKind::Predecessor,
            selected.id,
            committed_at,
        );
        kernel.append_revision_link(source_link.clone())?;
        kernel.append_revision_link(target_link.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:checkout-link-selected",
                )),
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(
            slice.audit_traces[0].revision_links,
            vec![source_link, target_link]
        );
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_deduplicate_self_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-self", 0.95, 10)?,
        )?;
        let committed_at = test_commit_time()?;
        let self_link = RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::DerivesFrom,
            selected.id,
            committed_at,
        );
        kernel.append_revision_link(self_link.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:checkout-link-self",
                )),
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.audit_traces[0].revision_links, vec![self_link]);
        Ok(())
    }

    #[test]
    fn checkout_records_token_budget_alternatives() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = sample_cell("project:continuitydb:selected", 0.95, 10)?;
        let omitted = sample_cell("project:continuitydb:omitted", 0.90, 10)?;
        let selected = append_committed(&mut kernel, selected)?;
        let omitted = append_committed(&mut kernel, omitted)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.alternatives.len(), 1);
        assert_eq!(slice.alternatives[0].cell_id, omitted.id);
        assert_eq!(
            slice.alternatives[0].reason,
            super::CheckoutAlternativeReason::TokenBudgetExceeded
        );
        assert_eq!(
            slice.alternatives[0].citations,
            vec!["test://project:continuitydb:omitted".to_string()]
        );
        assert_eq!(slice.alternatives[0].max_confidence, Confidence::new(0.90)?);
        Ok(())
    }

    #[test]
    fn audit_includes_cell_id_and_citation_locator() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_cell("project:continuitydb:audit", 0.95, 10)?;
        let trace = audit(&cell);

        assert_eq!(trace.cell_id, cell.id);
        assert_eq!(
            trace.citations,
            vec!["test://project:continuitydb:audit".to_string()]
        );
        assert!(trace.dependencies.is_empty());
        Ok(())
    }

    #[test]
    fn audit_includes_commit_id() -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_cell("project:continuitydb:audit-commit", 0.95, 10)?;
        cell.commit_id = CommitId::new();

        let trace = audit(&cell);

        assert_eq!(trace.commit_id, cell.commit_id);
        Ok(())
    }

    #[test]
    fn audit_includes_structured_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_cell("project:continuitydb:audit-evidence", 0.95, 10)?;
        let trace = audit(&cell);

        assert_eq!(trace.evidence.len(), 1);
        assert_eq!(trace.evidence[0].source, "test");
        assert_eq!(
            trace.evidence[0].locator,
            "test://project:continuitydb:audit-evidence"
        );
        assert_eq!(trace.evidence[0].confidence, Confidence::new(0.95)?);
        assert_eq!(
            trace.evidence[0].trust,
            vec![TrustSignal::DirectObservation]
        );
        Ok(())
    }

    #[test]
    fn audit_includes_dependency_links() -> Result<(), Box<dyn std::error::Error>> {
        let target = StateCellId::new();
        let mut cell = sample_cell("project:continuitydb:audit-dependency", 0.95, 10)?;
        cell.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DerivedFrom,
            "derived from source evidence",
        ));

        let trace = audit(&cell);

        assert_eq!(trace.dependencies.len(), 1);
        assert_eq!(trace.dependencies[0].target, target);
        assert_eq!(trace.dependencies[0].kind, CellDependencyKind::DerivedFrom);
        assert_eq!(
            trace.dependencies[0].rationale,
            "derived from source evidence"
        );
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_include_dependency_links() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:audit-dependent", 0.95, 10)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target state",
        ));
        append_committed(&mut kernel, dependent)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.audit_traces.len(), 1);
        assert_eq!(slice.audit_traces[0].dependencies.len(), 1);
        assert_eq!(slice.audit_traces[0].dependencies[0].target, target);
        assert_eq!(
            slice.audit_traces[0].dependencies[0].kind,
            CellDependencyKind::DependsOn
        );
        assert_eq!(
            slice.audit_traces[0].dependencies[0].rationale,
            "depends on target state"
        );
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_include_commit_ids() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:audit-commit-checkout", 0.95, 10)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                answerability_question: None,
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.audit_traces.len(), 1);
        assert_eq!(slice.audit_traces[0].cell_id, selected.id);
        assert_eq!(slice.audit_traces[0].commit_id, selected.commit_id);
        Ok(())
    }
}
