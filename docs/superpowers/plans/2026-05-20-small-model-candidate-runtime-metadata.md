# Small Model Candidate Runtime Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add machine-readable runtime metadata to the small local Steward model candidate registry.

**Architecture:** Extend `SmallModelCandidate` with static runtime metadata and public accessors. Preserve the fixed registry ordering while exposing the same metadata through `continuitydb local-model-candidates` JSON.

**Tech Stack:** Rust, serde_json CLI assertions, existing `continuitydb-steward` local-model feature tests, existing `continuitydb-cli` tests.

---

### Task 1: Add Failing Steward Candidate Metadata Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add metadata accessor test**

Add a feature-gated test near `small_model_candidates_include_default_feasibility_model`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn small_model_candidates_expose_runtime_metadata() {
    let candidates = small_model_candidates();

    assert!(candidates.iter().all(|candidate| {
        !candidate.recommended_runtime().is_empty()
            && !candidate.artifact_format().is_empty()
            && candidate.requires_grammar()
            && !candidate.notes().is_empty()
    }));
    assert_eq!(candidates[0].recommended_runtime(), "llama.cpp");
    assert_eq!(candidates[0].artifact_format(), "GGUF");
    assert_eq!(candidates[0].recommended_temperature(), 0.0);
}
```

- [x] **Step 2: Run steward focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-steward small_model_candidates_expose_runtime_metadata --features local-model
```

Expected: compile failure because candidate metadata accessors do not exist.

### Task 2: Implement Candidate Runtime Metadata

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add fields to `SmallModelCandidate`**

Add:

```rust
recommended_runtime: &'static str,
artifact_format: &'static str,
recommended_temperature_millis: u16,
requires_grammar: bool,
notes: &'static str,
```

- [x] **Step 2: Add public accessors**

Add methods:

```rust
pub fn recommended_runtime(&self) -> &'static str { self.recommended_runtime }
pub fn artifact_format(&self) -> &'static str { self.artifact_format }
pub fn recommended_temperature(&self) -> f32 { f32::from(self.recommended_temperature_millis) / 1000.0 }
pub fn requires_grammar(&self) -> bool { self.requires_grammar }
pub fn notes(&self) -> &'static str { self.notes }
```

- [x] **Step 3: Populate every fixed candidate**

Use these values for all current candidates:

```rust
recommended_runtime: "llama.cpp",
artifact_format: "GGUF",
recommended_temperature_millis: 0,
requires_grammar: true,
notes: "<candidate-specific operational note>",
```

- [x] **Step 4: Run steward focused test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward small_model_candidates_expose_runtime_metadata --features local-model
```

Expected: PASS.

### Task 3: Expose Metadata In CLI Candidate JSON

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-small-model-candidate-runtime-metadata.md`

- [x] **Step 1: Add CLI candidate JSON assertions**

In `cli_local_model_candidates_outputs_fixed_registry`, assert:

```rust
assert_eq!(json["candidates"][0]["recommended_runtime"].as_str(), Some("llama.cpp"));
assert_eq!(json["candidates"][0]["artifact_format"].as_str(), Some("GGUF"));
assert_eq!(json["candidates"][0]["recommended_temperature"].as_f64(), Some(0.0));
assert_eq!(json["candidates"][0]["requires_grammar"].as_bool(), Some(true));
assert!(json["candidates"][0]["notes"].as_str().is_some_and(|notes| !notes.is_empty()));
```

- [x] **Step 2: Run CLI focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_candidates_outputs_fixed_registry --features local-model
```

Expected: failure because CLI JSON lacks the new fields.

- [x] **Step 3: Add CLI JSON fields**

In `local_model_candidates_json()`, add fields for the new candidate accessors.

- [x] **Step 4: Run CLI focused test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_candidates_outputs_fixed_registry --features local-model
```

Expected: PASS.

- [x] **Step 5: Update README current scope**

Add:

```markdown
- Small local Steward model runtime metadata.
```

- [x] **Step 6: Update Steward roadmap**

Add after milestone 41:

```markdown
42. Add small-model candidate runtime metadata. Extended the local Steward candidate registry with recommended runtime, artifact format, temperature, grammar requirement, and operational notes, and exposed the metadata through CLI candidate JSON.
```

- [x] **Step 7: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 8: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 9: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-small-model-candidate-runtime-metadata-design.md docs/superpowers/plans/2026-05-20-small-model-candidate-runtime-metadata.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose small model runtime metadata"
```
