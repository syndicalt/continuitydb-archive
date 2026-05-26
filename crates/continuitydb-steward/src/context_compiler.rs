//! Proposal-only orchestration for StateCell v2 context compilation.

use std::collections::HashSet;

use continuitydb_core::{
    Confidence, ContextAbstractionLevel, ContextCompilerProposal, ContextCompilerProposalLine,
    ContextPacketStrategy, StateCell, StateCellId, TrajectoryMemory,
};

use crate::StewardError;

/// Input supplied to a context compiler proposal source.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextCompilerOrchestratorInput {
    task_intent: String,
    candidate_cell_ids: Vec<StateCellId>,
    candidate_evidence_locators: Vec<ContextCompilerCandidateEvidence>,
    candidate_trajectory_memories: Vec<ContextCompilerCandidateTrajectoryMemory>,
}

/// Evidence locators available for a candidate StateCell during proposal generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCompilerCandidateEvidence {
    cell_id: StateCellId,
    evidence_locators: Vec<String>,
}

/// Distilled rollout experience available for a candidate StateCell during proposal generation.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextCompilerCandidateTrajectoryMemory {
    cell_id: StateCellId,
    hypothesis_tried: String,
    progress_made: String,
    failure_mode: String,
    trace_locator: String,
    confidence: Confidence,
    reusable_lesson: String,
    applicability_conditions: Vec<String>,
    invalidation_conditions: Vec<String>,
    checkout_strategy: ContextPacketStrategy,
}

impl ContextCompilerOrchestratorInput {
    /// Creates a context compiler orchestration request for selected candidate cells.
    pub fn new(task_intent: impl Into<String>, candidate_cell_ids: Vec<StateCellId>) -> Self {
        let mut deduplicated_candidate_cell_ids = Vec::new();
        for cell_id in candidate_cell_ids {
            if !deduplicated_candidate_cell_ids.contains(&cell_id) {
                deduplicated_candidate_cell_ids.push(cell_id);
            }
        }

        Self {
            task_intent: task_intent.into(),
            candidate_cell_ids: deduplicated_candidate_cell_ids,
            candidate_evidence_locators: Vec::new(),
            candidate_trajectory_memories: Vec::new(),
        }
    }

    /// Creates a context compiler orchestration request directly from selected StateCells.
    pub fn from_state_cells(task_intent: impl Into<String>, candidates: &[StateCell]) -> Self {
        let mut input = Self::new(
            task_intent,
            candidates.iter().map(|candidate| candidate.id).collect(),
        );
        for candidate in candidates {
            let mut locators = candidate
                .evidence
                .iter()
                .map(|evidence| evidence.citation.locator.clone())
                .filter(|locator| !locator.trim().is_empty())
                .collect::<Vec<_>>();
            if let Some(trajectory_memory) = &candidate.trajectory_memory {
                let trace_locator = trajectory_memory.trace_locator.trim();
                if !trace_locator.is_empty() {
                    locators.push(trace_locator.to_string());
                }
                input = input.with_candidate_trajectory_memory(candidate.id, trajectory_memory);
            }
            input = input.with_candidate_evidence_locators(candidate.id, locators);
        }
        input
    }

    /// Adds citation locators available for the candidate StateCell.
    pub fn with_candidate_evidence_locators(
        mut self,
        cell_id: StateCellId,
        evidence_locators: Vec<String>,
    ) -> Self {
        if !self.candidate_cell_ids.contains(&cell_id) {
            return self;
        }

        if let Some(candidate) = self
            .candidate_evidence_locators
            .iter_mut()
            .find(|candidate| candidate.cell_id == cell_id)
        {
            for locator in evidence_locators {
                if locator.trim().is_empty() {
                    continue;
                }
                if !candidate
                    .evidence_locators
                    .iter()
                    .any(|existing| existing == &locator)
                {
                    candidate.evidence_locators.push(locator);
                }
            }
            return self;
        }

        let evidence_locators = evidence_locators
            .into_iter()
            .filter(|locator| !locator.trim().is_empty())
            .collect();
        self.candidate_evidence_locators
            .push(ContextCompilerCandidateEvidence {
                cell_id,
                evidence_locators,
            });
        self
    }

    fn with_candidate_trajectory_memory(
        mut self,
        cell_id: StateCellId,
        trajectory_memory: &TrajectoryMemory,
    ) -> Self {
        self.candidate_trajectory_memories
            .push(ContextCompilerCandidateTrajectoryMemory {
                cell_id,
                hypothesis_tried: trajectory_memory.hypothesis_tried.clone(),
                progress_made: trajectory_memory.progress_made.clone(),
                failure_mode: trajectory_memory.failure_mode.clone(),
                trace_locator: trajectory_memory.trace_locator.clone(),
                confidence: trajectory_memory.confidence,
                reusable_lesson: trajectory_memory.reusable_lesson.clone(),
                applicability_conditions: trajectory_memory.applicability_conditions.clone(),
                invalidation_conditions: trajectory_memory.invalidation_conditions.clone(),
                checkout_strategy: trajectory_memory.checkout_strategy,
            });
        self
    }

    /// Returns the caller's task intent.
    pub fn task_intent(&self) -> &str {
        &self.task_intent
    }

    /// Returns StateCell IDs that the source is allowed to shape.
    pub fn candidate_cell_ids(&self) -> &[StateCellId] {
        &self.candidate_cell_ids
    }

    /// Returns evidence locators available for the candidate StateCell.
    pub fn candidate_evidence_locators(&self, cell_id: StateCellId) -> &[String] {
        self.candidate_evidence_locators
            .iter()
            .find(|candidate| candidate.cell_id == cell_id)
            .map(|candidate| candidate.evidence_locators.as_slice())
            .unwrap_or(&[])
    }

    /// Returns distilled trajectory memory available for the candidate StateCell, if any.
    pub fn candidate_trajectory_memory(
        &self,
        cell_id: StateCellId,
    ) -> Option<&ContextCompilerCandidateTrajectoryMemory> {
        self.candidate_trajectory_memories
            .iter()
            .find(|candidate| candidate.cell_id == cell_id)
    }
}

impl ContextCompilerCandidateTrajectoryMemory {
    /// Returns the hypothesis, plan, or approach attempted by the prior rollout.
    pub fn hypothesis_tried(&self) -> &str {
        &self.hypothesis_tried
    }

    /// Returns the useful progress made by the prior rollout.
    pub fn progress_made(&self) -> &str {
        &self.progress_made
    }

    /// Returns the observed failure mode or stopping condition.
    pub fn failure_mode(&self) -> &str {
        &self.failure_mode
    }

    /// Returns the retained trace locator supporting this trajectory memory.
    pub fn trace_locator(&self) -> &str {
        &self.trace_locator
    }

    /// Returns confidence that this distilled lesson applies as recorded.
    pub fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns the reusable lesson distilled from the prior rollout.
    pub fn reusable_lesson(&self) -> &str {
        &self.reusable_lesson
    }

    /// Returns conditions under which the trajectory lesson should be reused.
    pub fn applicability_conditions(&self) -> &[String] {
        &self.applicability_conditions
    }

    /// Returns conditions that invalidate or retire the trajectory lesson.
    pub fn invalidation_conditions(&self) -> &[String] {
        &self.invalidation_conditions
    }

    /// Returns the preferred checkout strategy for this trajectory lesson.
    pub fn checkout_strategy(&self) -> ContextPacketStrategy {
        self.checkout_strategy
    }
}

/// Untrusted packet-shape suggestion emitted by a model or adaptive policy source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCompilerProposalDraft {
    target_cell_id: StateCellId,
    strategy: ContextPacketStrategy,
    abstraction_level: ContextAbstractionLevel,
    reason_tags: Vec<String>,
    evidence_locators: Vec<String>,
    proposed_lines: Vec<ContextCompilerProposalLine>,
}

impl ContextCompilerProposalDraft {
    /// Creates an untrusted packet-shape suggestion.
    pub fn new(
        target_cell_id: StateCellId,
        strategy: ContextPacketStrategy,
        reason_tags: Vec<String>,
        evidence_locators: Vec<String>,
    ) -> Self {
        Self {
            target_cell_id,
            strategy,
            abstraction_level: strategy.default_abstraction_level(),
            reason_tags,
            evidence_locators,
            proposed_lines: Vec::new(),
        }
    }

    /// Overrides the default abstraction level for this packet-shape draft.
    pub fn with_abstraction_level(mut self, abstraction_level: ContextAbstractionLevel) -> Self {
        self.abstraction_level = abstraction_level;
        self
    }

    /// Attaches citation-backed proposed packet lines to this draft.
    pub fn with_proposed_lines(mut self, proposed_lines: Vec<ContextCompilerProposalLine>) -> Self {
        self.proposed_lines = proposed_lines;
        self
    }

    fn into_validated_proposal(self) -> Result<ContextCompilerProposal, StewardError> {
        ContextCompilerProposal::new(
            self.target_cell_id,
            self.strategy,
            self.abstraction_level,
            self.reason_tags,
            self.evidence_locators,
        )
        .map(|proposal| proposal.with_proposed_lines(self.proposed_lines))
        .map_err(|_| StewardError::InvalidContextCompilerProposal)
    }
}

/// Source of untrusted context compiler proposal drafts.
pub trait ContextCompilerProposalSource {
    /// Emits untrusted packet-shape suggestions for the given orchestration input.
    fn propose_context_compiler_drafts(
        &self,
        input: &ContextCompilerOrchestratorInput,
    ) -> Result<Vec<ContextCompilerProposalDraft>, StewardError>;
}

/// Deterministic proposal source useful for tests and fixed policy adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticContextCompilerProposalSource {
    drafts: Vec<ContextCompilerProposalDraft>,
}

impl StaticContextCompilerProposalSource {
    /// Creates a static source with precomputed proposal drafts.
    pub fn new(drafts: Vec<ContextCompilerProposalDraft>) -> Self {
        Self { drafts }
    }
}

impl ContextCompilerProposalSource for StaticContextCompilerProposalSource {
    fn propose_context_compiler_drafts(
        &self,
        _input: &ContextCompilerOrchestratorInput,
    ) -> Result<Vec<ContextCompilerProposalDraft>, StewardError> {
        Ok(self.drafts.clone())
    }
}

/// Validates adaptive or model-produced packet suggestions before checkout can use them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCompilerOrchestrator<S> {
    source: S,
}

impl<S> ContextCompilerOrchestrator<S>
where
    S: ContextCompilerProposalSource,
{
    /// Creates a context compiler orchestrator around an untrusted proposal source.
    pub fn new(source: S) -> Self {
        Self { source }
    }

    /// Produces validated core compiler proposals for deterministic checkout.
    pub fn propose(
        &self,
        input: ContextCompilerOrchestratorInput,
    ) -> Result<Vec<ContextCompilerProposal>, StewardError> {
        let allowed_targets = input
            .candidate_cell_ids()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut seen_targets = HashSet::new();
        let mut proposals = Vec::new();

        for draft in self.source.propose_context_compiler_drafts(&input)? {
            if !allowed_targets.contains(&draft.target_cell_id) {
                return Err(StewardError::InvalidContextCompilerProposalTarget);
            }
            if !seen_targets.insert(draft.target_cell_id) {
                return Err(StewardError::DuplicateContextCompilerProposalTarget);
            }
            let allowed_evidence = input
                .candidate_evidence_locators(draft.target_cell_id)
                .iter()
                .collect::<HashSet<_>>();
            if draft
                .evidence_locators
                .iter()
                .any(|locator| !allowed_evidence.contains(locator))
            {
                return Err(StewardError::InvalidContextCompilerProposal);
            }
            if draft.proposed_lines.iter().any(|line| {
                line.citations
                    .iter()
                    .any(|citation| !allowed_evidence.contains(citation))
            }) {
                return Err(StewardError::InvalidContextCompilerProposal);
            }
            proposals.push(draft.into_validated_proposal()?);
        }

        if seen_targets.len() != allowed_targets.len() {
            return Err(StewardError::MissingContextCompilerProposalTarget);
        }

        Ok(proposals)
    }
}
