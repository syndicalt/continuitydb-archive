# Native Steward LabelAnswerability Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic native API application for accepted Steward `LabelAnswerability` proposals as append-only `StateCell` successors.

**Architecture:** Add an answerability revision helper to `continuitydb-revision`, then expose a feature-gated `ContinuityDb<K>` method that applies only accepted `LabelAnswerability` audit records. Rejected records remain no-ops; accepted unsupported actions fail explicitly.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-revision`, `continuitydb-api`, optional `continuitydb-steward` feature, existing memory kernel tests.

---

### Task 1: Add Answerability Revision Helper

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`

- [x] **Step 1: Write failing answerability revision test**

Add a test near the activation revision test:

```rust
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
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-revision answerability_revision_creates_successor_with_revision_links`

Expected: FAIL because `revise_answerability` does not exist.

- [x] **Step 3: Implement answerability revision helper**

Update imports:

```rust
use continuitydb_core::{
    ActivationState, Answerability, SemanticAnchor, StateCell, StateCellId, UtilityFeedback,
};
```

Add:

```rust
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
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-revision answerability_revision_creates_successor_with_revision_links`

Expected: PASS.

### Task 2: Add Native API LabelAnswerability Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing accepted application test**

Add this feature-gated test near the existing Steward application tests:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_label_answerability_application_appends_successor(
) -> Result<(), Box<dyn std::error::Error>> {
    let initial_commit = test_steward_time()?;
    let apply_commit = Utc
        .with_ymd_and_hms(2026, 5, 20, 13, 15, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let original_id = db.ingest_cell_at(
        sample_cell("project:continuitydb:apply-answerability", 0.91, 12)?,
        initial_commit,
    )?;
    let questions = vec![
        "what changed?".to_string(),
        "what needs review?".to_string(),
    ];
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::LabelAnswerability {
            cell_id: original_id,
            questions: questions.clone(),
        },
        "The cell can answer updated review questions.",
        vec!["test://answerability-apply".to_string()],
        initial_commit,
    )?;
    let record =
        db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

    let successor_id = db
        .apply_accepted_label_answerability_proposal_at(&record, apply_commit)?
        .ok_or_else(|| std::io::Error::other("expected successor"))?;

    let original = db
        .kernel()
        .lookup_cells(CellLookup {
            cell_id: Some(original_id),
            ..CellLookup::default()
        })?
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::other("missing original"))?;
    let successor = db
        .kernel()
        .lookup_cells(CellLookup {
            cell_id: Some(successor_id),
            ..CellLookup::default()
        })?
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::other("missing successor"))?;

    assert_ne!(successor_id, original_id);
    assert_eq!(
        original.answerability.questions(),
        &["what should the agent know?".to_string()]
    );
    assert_eq!(successor.answerability.questions(), questions.as_slice());
    assert_eq!(successor.system_time.from(), apply_commit);
    Ok(())
}
```

- [x] **Step 2: Write failing no-op and error tests**

Add tests:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_label_answerability_application_ignores_rejected_record(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let original_id = db.ingest_cell_at(
        sample_cell("project:continuitydb:rejected-answerability", 0.91, 12)?,
        committed_at,
    )?;
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::LabelAnswerability {
            cell_id: original_id,
            questions: vec!["what changed?".to_string()],
        },
        "Policy rejected this answerability application.",
        vec!["test://answerability-rejected".to_string()],
        committed_at,
    )?;
    let decision = continuitydb_steward::ProposalDecision::new(
        proposal.id(),
        ProposalOutcome::Rejected,
        vec!["policy:test-rejected".to_string()],
        committed_at,
    );
    let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

    let applied = db.apply_accepted_label_answerability_proposal_at(&record, committed_at)?;

    assert_eq!(applied, None);
    assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
    Ok(())
}

#[cfg(feature = "steward")]
#[test]
fn api_label_answerability_application_rejects_unsupported_accepted_action(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::MarkFrontier {
            cell_id: StateCellId::new(),
        },
        "Frontier application belongs to a different method.",
        vec!["test://unsupported-answerability-apply".to_string()],
        committed_at,
    )?;
    let record =
        db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

    let result = db.apply_accepted_label_answerability_proposal_at(&record, committed_at);

    assert!(matches!(
        result,
        Err(ContinuityError::UnsupportedStewardProposalAction)
    ));
    Ok(())
}

#[cfg(feature = "steward")]
#[test]
fn api_label_answerability_application_reports_missing_cell(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let missing_id = StateCellId::new();
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::LabelAnswerability {
            cell_id: missing_id,
            questions: vec!["what changed?".to_string()],
        },
        "Missing target should be reported before application.",
        vec!["test://answerability-missing".to_string()],
        committed_at,
    )?;
    let record =
        db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

    let result = db.apply_accepted_label_answerability_proposal_at(&record, committed_at);

    assert!(matches!(
        result,
        Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
    ));
    Ok(())
}
```

- [x] **Step 3: Run tests to verify they fail**

Run: `cargo test -p continuitydb-api --features steward label_answerability_application`

Expected: FAIL because the application API does not exist.

### Task 3: Implement Native API Application

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add feature-gated imports**

Add `Answerability` and `revise_answerability` behind the `steward` feature.

- [x] **Step 2: Add application method**

Add:

```rust
#[cfg(feature = "steward")]
pub fn apply_accepted_label_answerability_proposal_at(
    &mut self,
    record: &ProposalAuditRecord,
    committed_at: DateTime<Utc>,
) -> Result<Option<StateCellId>, ContinuityError> {
    if record.decision().outcome() == ProposalOutcome::Rejected {
        return Ok(None);
    }

    let (cell_id, questions) = match record.proposal().action() {
        StewardAction::LabelAnswerability { cell_id, questions } => (*cell_id, questions),
        _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
    };

    let answerability = Answerability::new(questions.clone())?;
    let previous = self.lookup_one_cell(cell_id)?;
    let revision = revise_answerability(&previous, answerability);
    let successor_id = revision.cell.id;
    self.kernel.append_cell_at(revision.cell, committed_at)?;
    Ok(Some(successor_id))
}
```

If `Answerability::new` cannot be converted by `?`, add a `ContinuityError::Core(#[from] CoreError)` variant and import `CoreError`.

- [x] **Step 3: Run targeted tests**

Run: `cargo test -p continuitydb-api --features steward label_answerability_application`

Expected: PASS.

### Task 4: Update Docs and Roadmap

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- Native feature-gated Steward `LabelAnswerability` application API.
```

- [x] **Step 2: Update roadmap milestones**

Add Native API milestone 30:

```markdown
30. Add native accepted Steward `LabelAnswerability` application. Implemented optional `steward` feature method `apply_accepted_label_answerability_proposal_at` so embedders can deterministically apply accepted answerability-label proposals as append-only StateCell successors while rejected and unsupported proposals do not mutate committed truth.
```

Add Steward milestone 16:

```markdown
16. Add accepted `LabelAnswerability` proposal application. Implemented deterministic answerability-label proposal application through the native API, preserving the model-as-proposer boundary while allowing accepted labels to become committed append-only StateCell revisions.
```

### Task 5: Verify and Commit

**Files:**
- Modify: plan checklist as steps complete.

- [x] **Step 1: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 2: Commit**

Run:

```bash
git add crates/continuitydb-api/src/lib.rs crates/continuitydb-revision/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-steward-label-answerability-application-design.md docs/superpowers/plans/2026-05-20-native-steward-label-answerability-application.md
git commit -m "feat: apply accepted answerability steward proposals"
```
