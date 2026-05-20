# Native Steward AdjustConfidence Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic native API application for accepted Steward `AdjustConfidence` proposals as append-only `StateCell` successors.

**Architecture:** Add an evidence-confidence revision helper to `continuitydb-revision`, then expose a feature-gated `ContinuityDb<K>` method that applies only accepted `AdjustConfidence` audit records. Rejected records remain no-ops; accepted unsupported actions fail explicitly.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-revision`, `continuitydb-api`, optional `continuitydb-steward` feature, existing memory kernel tests.

---

### Task 1: Add Evidence Confidence Revision Helper

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`

- [x] **Step 1: Write failing confidence revision test**

Add a test near the answerability revision test:

```rust
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
    assert_eq!(revised.cell.evidence[0].citation, previous.evidence[0].citation);
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
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-revision evidence_confidence_revision_creates_successor_with_revision_links`

Expected: FAIL because `revise_evidence_confidence` does not exist.

- [x] **Step 3: Implement evidence confidence revision helper**

Update imports:

```rust
use continuitydb_core::{
    ActivationState, Answerability, Confidence, SemanticAnchor, StateCell, StateCellId,
    UtilityFeedback,
};
```

Add:

```rust
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
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-revision evidence_confidence_revision_creates_successor_with_revision_links`

Expected: PASS.

### Task 2: Add Native API AdjustConfidence Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing accepted application test**

Add this feature-gated test near the existing Steward application tests:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_adjust_confidence_application_appends_successor(
) -> Result<(), Box<dyn std::error::Error>> {
    let initial_commit = test_steward_time()?;
    let apply_commit = Utc
        .with_ymd_and_hms(2026, 5, 20, 13, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let original_id = db.ingest_cell_at(
        sample_cell("project:continuitydb:apply-confidence", 0.41, 12)?,
        initial_commit,
    )?;
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::AdjustConfidence {
            cell_id: original_id,
            proposed_confidence: 0.86,
        },
        "New evidence increases confidence.",
        vec!["test://confidence-apply".to_string()],
        initial_commit,
    )?;
    let record =
        db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

    let successor_id = db
        .apply_accepted_adjust_confidence_proposal_at(&record, apply_commit)?
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
    assert_eq!(original.evidence[0].confidence, Confidence::new(0.41)?);
    assert_eq!(successor.evidence[0].confidence, Confidence::new(0.86)?);
    assert_eq!(successor.evidence[0].source, original.evidence[0].source);
    assert_eq!(successor.evidence[0].citation, original.evidence[0].citation);
    assert_eq!(successor.evidence[0].trust, original.evidence[0].trust);
    assert_eq!(successor.system_time.from(), apply_commit);
    Ok(())
}
```

- [x] **Step 2: Write failing no-op and error tests**

Add tests:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_adjust_confidence_application_ignores_rejected_record(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let original_id = db.ingest_cell_at(
        sample_cell("project:continuitydb:rejected-confidence", 0.41, 12)?,
        committed_at,
    )?;
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::AdjustConfidence {
            cell_id: original_id,
            proposed_confidence: 0.86,
        },
        "Policy rejected this confidence application.",
        vec!["test://confidence-rejected".to_string()],
        committed_at,
    )?;
    let decision = continuitydb_steward::ProposalDecision::new(
        proposal.id(),
        ProposalOutcome::Rejected,
        vec!["policy:test-rejected".to_string()],
        committed_at,
    );
    let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

    let applied = db.apply_accepted_adjust_confidence_proposal_at(&record, committed_at)?;

    assert_eq!(applied, None);
    assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
    Ok(())
}

#[cfg(feature = "steward")]
#[test]
fn api_adjust_confidence_application_rejects_unsupported_accepted_action(
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
        vec!["test://unsupported-confidence-apply".to_string()],
        committed_at,
    )?;
    let record =
        db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

    let result = db.apply_accepted_adjust_confidence_proposal_at(&record, committed_at);

    assert!(matches!(
        result,
        Err(ContinuityError::UnsupportedStewardProposalAction)
    ));
    Ok(())
}

#[cfg(feature = "steward")]
#[test]
fn api_adjust_confidence_application_reports_missing_cell(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let missing_id = StateCellId::new();
    let proposal = StewardProposal::new(
        ProposalId::new(),
        test_steward_identity()?,
        StewardAction::AdjustConfidence {
            cell_id: missing_id,
            proposed_confidence: 0.86,
        },
        "Missing target should be reported before application.",
        vec!["test://confidence-missing".to_string()],
        committed_at,
    )?;
    let record =
        db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

    let result = db.apply_accepted_adjust_confidence_proposal_at(&record, committed_at);

    assert!(matches!(
        result,
        Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
    ));
    Ok(())
}
```

- [x] **Step 3: Run tests to verify they fail**

Run: `cargo test -p continuitydb-api --features steward adjust_confidence_application`

Expected: FAIL because the application API does not exist.

### Task 3: Implement Native API Application

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add feature-gated imports**

Add `Confidence` and `revise_evidence_confidence` behind the `steward` feature.

- [x] **Step 2: Add application method**

Add:

```rust
#[cfg(feature = "steward")]
pub fn apply_accepted_adjust_confidence_proposal_at(
    &mut self,
    record: &ProposalAuditRecord,
    committed_at: DateTime<Utc>,
) -> Result<Option<StateCellId>, ContinuityError> {
    if record.decision().outcome() == ProposalOutcome::Rejected {
        return Ok(None);
    }

    let (cell_id, proposed_confidence) = match record.proposal().action() {
        StewardAction::AdjustConfidence {
            cell_id,
            proposed_confidence,
        } => (*cell_id, *proposed_confidence),
        _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
    };

    let confidence = Confidence::new(proposed_confidence)?;
    let previous = self.lookup_one_cell(cell_id)?;
    let revision = revise_evidence_confidence(&previous, confidence);
    let successor_id = revision.cell.id;
    self.kernel.append_cell_at(revision.cell, committed_at)?;
    Ok(Some(successor_id))
}
```

- [x] **Step 3: Run targeted tests**

Run: `cargo test -p continuitydb-api --features steward adjust_confidence_application`

Expected: PASS.

### Task 4: Update Docs and Roadmap

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- Native feature-gated Steward `AdjustConfidence` application API.
```

- [x] **Step 2: Update roadmap milestones**

Add Native API milestone 31:

```markdown
31. Add native accepted Steward `AdjustConfidence` application. Implemented optional `steward` feature method `apply_accepted_adjust_confidence_proposal_at` so embedders can deterministically apply accepted confidence proposals as append-only StateCell successors while rejected and unsupported proposals do not mutate committed truth.
```

Add Steward milestone 17:

```markdown
17. Add accepted `AdjustConfidence` proposal application. Implemented deterministic confidence proposal application through the native API by appending successor StateCells with revised evidence confidence while preserving the model-as-proposer boundary.
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
git add crates/continuitydb-api/src/lib.rs crates/continuitydb-revision/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-steward-adjust-confidence-application-design.md docs/superpowers/plans/2026-05-20-native-steward-adjust-confidence-application.md
git commit -m "feat: apply accepted confidence steward proposals"
```
