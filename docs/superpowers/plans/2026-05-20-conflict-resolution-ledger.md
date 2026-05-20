# Conflict Resolution Ledger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist conflict-resolution Steward proposals through the existing proposal ledger path so deterministic revision policy output is durable and auditable.

**Architecture:** Extend `ConflictResolutionSteward` with a generic helper that converts a `ConflictResolutionScan` into proposals, evaluates each proposal with a supplied `ProposalPolicy`, and records proposal/decision pairs into any `StoredProposalLedger<S>`. This reuses existing ledger stores instead of creating a new persistence mechanism.

**Tech Stack:** Rust 2021, `chrono`, `continuitydb-revision`, `continuitydb-steward`.

---

### Task 1: Record Conflict-Resolution Proposals

**Files:**
- Modify: `crates/continuitydb-steward/src/conflict.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-conflict-resolution-ledger.md`

- [x] **Step 1: Write the failing ledger test**

Add a test to `crates/continuitydb-steward/src/conflict.rs` that:

1. Builds low-confidence and high-confidence conflicting cells.
2. Runs `recommend_conflict_resolutions`.
3. Creates `StoredProposalLedger::new(MemoryProposalStore::default())`.
4. Calls:

```rust
let proposals = steward.propose_and_record(
    scan,
    created_at(),
    &ProposalPolicy::strict(),
    &mut ledger,
)?;
```

5. Asserts:

```rust
assert_eq!(proposals.len(), 1);
let records = ledger.records()?;
assert_eq!(records.len(), 1);
assert_eq!(records[0].proposal().id(), proposals[0].id());
assert_eq!(records[0].decision().outcome(), ProposalOutcome::Accepted);
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-steward conflict_resolution_steward_records_policy_decisions
```

Expected: FAIL because `ConflictResolutionSteward::propose_and_record` does not exist.

- [x] **Step 3: Implement ledger helper**

Add to `ConflictResolutionSteward`:

```rust
pub fn propose_and_record<S>(
    &self,
    scan: ConflictResolutionScan,
    created_at: DateTime<Utc>,
    policy: &ProposalPolicy,
    ledger: &mut StoredProposalLedger<S>,
) -> Result<Vec<StewardProposal>, StewardError>
where
    S: ProposalLedgerStore,
{
    let proposals = self.propose(scan, created_at)?;
    for proposal in proposals.iter().cloned() {
        let decision = policy.evaluate(&proposal, created_at);
        ledger.record(proposal, decision)?;
    }
    Ok(proposals)
}
```

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-steward conflict_resolution_steward_records_policy_decisions
cargo test -p continuitydb-steward conflict_resolution_steward
cargo test -p continuitydb-steward
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a Steward milestone noting conflict-resolution Steward proposals can now be policy-evaluated and persisted through existing proposal ledgers.

- [x] **Step 6: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 7: Commit**

```bash
git add crates/continuitydb-steward/src/conflict.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-conflict-resolution-ledger.md
git commit -m "feat: record conflict resolution proposals"
```
