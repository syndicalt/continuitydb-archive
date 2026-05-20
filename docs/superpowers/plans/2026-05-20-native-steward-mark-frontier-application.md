# Native Steward MarkFrontier Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first deterministic native API path that applies an accepted Steward `MarkFrontier` proposal as an append-only `StateCell` successor.

**Architecture:** Add a revision helper in `continuitydb-revision` for activation-state changes, then expose a feature-gated `ContinuityDb<K>` method that consumes an audited proposal record and commits only accepted `MarkFrontier` actions. Unsupported accepted actions fail explicitly; rejected actions are no-ops.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-revision`, `continuitydb-api`, optional `continuitydb-steward` feature, existing memory kernel tests.

---

### Task 1: Add Activation Revision Helper

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`

- [x] **Step 1: Write failing activation revision test**

Add a test near the existing utility feedback revision test:

```rust
#[test]
fn activation_revision_creates_successor_with_revision_links(
) -> Result<(), Box<dyn std::error::Error>> {
    let previous = sample_cell("project:continuitydb:activation-revision", 0.91)?;

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
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-revision activation_revision_creates_successor_with_revision_links`

Expected: FAIL because `ActivationState` and `revise_activation_state` are not imported or implemented.

- [x] **Step 3: Implement activation revision helper**

Update imports:

```rust
use continuitydb_core::{
    ActivationState, SemanticAnchor, StateCell, StateCellId, UtilityFeedback,
};
```

Add:

```rust
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
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-revision activation_revision_creates_successor_with_revision_links`

Expected: PASS.

### Task 2: Add Native API Application Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing accepted application test**

Add this feature-gated test near the Steward API tests:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_applies_accepted_mark_frontier_proposal_as_successor(
) -> Result<(), Box<dyn std::error::Error>> {
    let initial_commit = test_steward_time()?;
    let apply_commit = Utc
        .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let original_id = db.ingest_cell_at(
        sample_cell("project:continuitydb:apply-frontier", 0.91, 12)?,
        initial_commit,
    )?;
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::MarkFrontier { cell_id: original_id },
        "Stale evidence should move this cell to frontier monitoring.",
        vec!["test://frontier-apply".to_string()],
        initial_commit,
    )?;
    let record = db.record_steward_proposal(
        proposal,
        &ProposalPolicy::strict(),
        initial_commit,
    )?;

    let successor_id = db
        .apply_accepted_mark_frontier_proposal_at(&record, apply_commit)?
        .ok_or_else(|| std::io::Error::other("expected successor"))?;

    let original = db.audit_cell(original_id)?;
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
    assert_eq!(original.activation, ActivationState::Active);
    assert_eq!(successor.activation, ActivationState::Frontier);
    assert_eq!(successor.system_time.from(), apply_commit);
    Ok(())
}
```

- [x] **Step 2: Write failing no-op and error tests**

Add tests for rejected no-op, unsupported accepted action, and missing target:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_ignores_rejected_mark_frontier_application(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let original_id = db.ingest_cell_at(
        sample_cell("project:continuitydb:rejected-frontier", 0.91, 12)?,
        committed_at,
    )?;
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::MarkFrontier { cell_id: original_id },
        "Policy rejected this frontier application.",
        vec!["test://frontier-rejected".to_string()],
        committed_at,
    )?;
    let decision = continuitydb_steward::ProposalDecision::new(
        proposal.id(),
        ProposalOutcome::Rejected,
        vec!["policy:test-rejected".to_string()],
        committed_at,
    );
    let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

    let applied = db.apply_accepted_mark_frontier_proposal_at(&record, committed_at)?;

    assert_eq!(applied, None);
    assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
    Ok(())
}

#[cfg(feature = "steward")]
#[test]
fn api_rejects_unsupported_accepted_steward_application(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let source = StateCellId::new();
    let target = StateCellId::new();
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::LinkRevision {
            source,
            kind: RevisionLinkKind::Supersedes,
            target,
        },
        "Supersession application is not implemented in this slice.",
        vec!["test://unsupported-apply".to_string()],
        committed_at,
    )?;
    let record = db.record_steward_proposal(
        proposal,
        &ProposalPolicy::strict(),
        committed_at,
    )?;

    let result = db.apply_accepted_mark_frontier_proposal_at(&record, committed_at);

    assert!(matches!(
        result,
        Err(ContinuityError::UnsupportedStewardProposalAction)
    ));
    Ok(())
}

#[cfg(feature = "steward")]
#[test]
fn api_mark_frontier_application_reports_missing_cell(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let missing_id = StateCellId::new();
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::MarkFrontier { cell_id: missing_id },
        "Missing target should be reported before application.",
        vec!["test://frontier-missing".to_string()],
        committed_at,
    )?;
    let record = db.record_steward_proposal(
        proposal,
        &ProposalPolicy::strict(),
        committed_at,
    )?;

    let result = db.apply_accepted_mark_frontier_proposal_at(&record, committed_at);

    assert!(matches!(
        result,
        Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
    ));
    Ok(())
}
```

- [x] **Step 3: Run tests to verify they fail**

Run: `cargo test -p continuitydb-api --features steward mark_frontier_application`

Expected: FAIL because the application API and unsupported-action error do not exist.

### Task 3: Implement Native API Application

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add imports and error**

Import `ActivationState`, `ProposalOutcome`, and `StewardAction`, and add:

```rust
#[cfg(feature = "steward")]
#[error("steward proposal action is not supported by this application API")]
UnsupportedStewardProposalAction,
```

- [x] **Step 2: Add application method**

Add the method in the `impl<K: StorageKernel> ContinuityDb<K>` block near other Steward methods:

```rust
#[cfg(feature = "steward")]
pub fn apply_accepted_mark_frontier_proposal_at(
    &mut self,
    record: &ProposalAuditRecord,
    committed_at: DateTime<Utc>,
) -> Result<Option<StateCellId>, ContinuityError> {
    if record.decision().outcome() == ProposalOutcome::Rejected {
        return Ok(None);
    }

    let cell_id = match record.proposal().action() {
        StewardAction::MarkFrontier { cell_id } => *cell_id,
        _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
    };

    let previous = self.lookup_one_cell(cell_id)?;
    let revision = revise_activation_state(&previous, ActivationState::Frontier);
    let successor_id = revision.cell.id;
    self.kernel.append_cell_at(revision.cell, committed_at)?;
    Ok(Some(successor_id))
}
```

- [x] **Step 3: Run targeted tests**

Run: `cargo test -p continuitydb-api --features steward mark_frontier_application`

Expected: PASS.

### Task 4: Update Docs and Roadmap

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Update README current scope**

Add a bullet near the existing Steward native API bullets:

```markdown
- Native feature-gated Steward `MarkFrontier` application API.
```

- [x] **Step 2: Update roadmap milestones**

Add Native API milestone 29:

```markdown
29. Add native accepted Steward `MarkFrontier` application. Implemented optional `steward` feature method `apply_accepted_mark_frontier_proposal_at` so embedders can deterministically apply accepted frontier proposals as append-only StateCell successors while rejected and unsupported proposals do not mutate committed truth.
```

Add Steward milestone 15:

```markdown
15. Add accepted `MarkFrontier` proposal application. Implemented the first deterministic proposal-to-state mutation path: accepted frontier proposals append successor StateCells with `Frontier` activation through the native API, while models remain proposal-only.
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
git add crates/continuitydb-api/src/lib.rs crates/continuitydb-revision/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-steward-mark-frontier-application-design.md docs/superpowers/plans/2026-05-20-native-steward-mark-frontier-application.md
git commit -m "feat: apply accepted mark frontier steward proposals"
```
