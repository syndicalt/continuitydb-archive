# Conflict Resolution Kernel Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make conflict-resolution Steward audits explicitly persistable as StateCells through a ContinuityDB `StorageKernel`.

**Architecture:** Add a convenience helper on `ConflictResolutionSteward` that wraps any `StorageKernel` in the existing `KernelProposalStore`, records conflict-resolution proposals through policy evaluation, and returns the proposals plus the backing kernel. This keeps the existing ledger encoding path authoritative while giving embedders a direct kernel-backed API.

**Tech Stack:** Rust 2021, `continuitydb-kernel`, `continuitydb-memory`, `continuitydb-steward`.

---

### Task 1: Kernel-Backed Conflict Audit Helper

**Files:**
- Modify: `crates/continuitydb-steward/src/conflict.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-conflict-resolution-kernel-audit.md`

- [x] **Step 1: Write the failing kernel audit test**

Add a test to `crates/continuitydb-steward/src/conflict.rs` that:

1. Builds two conflicting cells.
2. Runs `recommend_conflict_resolutions`.
3. Calls:

```rust
let (proposals, kernel) = steward.propose_and_record_to_kernel(
    scan,
    created_at(),
    &ProposalPolicy::strict(),
    continuitydb_memory::MemoryKernel::default(),
)?;
```

4. Looks up audit cells:

```rust
let audit_cells = kernel.lookup_cells(CellLookup {
    semantic_anchor: Some("continuitydb:steward:proposal-audit".to_string()),
    ..CellLookup::default()
})?;
```

5. Asserts:

```rust
assert_eq!(proposals.len(), 1);
assert_eq!(audit_cells.len(), 1);
assert!(matches!(audit_cells[0].payload, CellPayload::Json(_)));
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-steward conflict_resolution_steward_records_to_kernel_cells
```

Expected: FAIL because `ConflictResolutionSteward::propose_and_record_to_kernel` does not exist.

- [x] **Step 3: Implement kernel helper**

Add:

```rust
pub fn propose_and_record_to_kernel<K>(
    &self,
    scan: ConflictResolutionScan,
    created_at: DateTime<Utc>,
    policy: &ProposalPolicy,
    kernel: K,
) -> Result<(Vec<StewardProposal>, K), StewardError>
where
    K: StorageKernel,
{
    let mut ledger = StoredProposalLedger::new(KernelProposalStore::new(kernel));
    let proposals = self.propose_and_record(scan, created_at, policy, &mut ledger)?;
    let store = ledger.into_store();
    Ok((proposals, store.into_kernel()))
}
```

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-steward conflict_resolution_steward_records_to_kernel_cells
cargo test -p continuitydb-steward conflict_resolution_steward
cargo test -p continuitydb-steward
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a Steward milestone noting conflict-resolution audits can be stored directly as StateCells through a `StorageKernel`.

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
git add crates/continuitydb-steward/src/conflict.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-conflict-resolution-kernel-audit.md
git commit -m "feat: record conflict audits to kernel"
```
