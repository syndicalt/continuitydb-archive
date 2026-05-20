# Local Model Evaluation Contract Introspection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only local-model evaluation suite introspection so embedders can inspect fixed benchmark contracts before running a model.

**Architecture:** Extend the existing feature-gated `continuitydb-steward` local-model API with borrowed accessors on suite, case, and input types. Keep scoring and CLI behavior unchanged.

**Tech Stack:** Rust, Cargo workspace tests, existing `local-model` feature, existing Steward test module.

---

### Task 1: Prove Default Evaluation Contract Introspection

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Write the failing test**

Add this feature-gated test near the existing default local-model evaluation suite tests:

```rust
#[cfg(feature = "local-model")]
#[test]
fn default_steward_evaluation_suite_exposes_case_contracts() {
    let suite = default_steward_evaluation_suite();

    assert_eq!(suite.len(), 2);
    assert!(!suite.is_empty());
    let cases = suite.cases();

    assert_eq!(cases[0].name(), "insufficient evidence uncertainty");
    assert_eq!(
        cases[0].input().task(),
        "Assess whether thin evidence needs verification."
    );
    assert_eq!(
        cases[0].input().evidence()[0].locator(),
        "continuitydb://evaluation/thin-evidence"
    );
    assert_eq!(
        cases[0].required_citations(),
        ["continuitydb://evaluation/thin-evidence".to_string()].as_slice()
    );
    assert_eq!(
        cases[0].required_rationale_terms(),
        ["uncertainty".to_string()].as_slice()
    );
    assert!(matches!(
        &cases[0].expected_actions()[0],
        StewardAction::RequestVerification { cell_id: None, request }
            if request == "Gather additional source evidence."
    ));

    assert_eq!(cases[1].name(), "conflict classification");
    assert_eq!(
        cases[1].input().task(),
        "Classify whether contradictory release-status claims conflict."
    );
    assert_eq!(
        cases[1].input().evidence()[0].locator(),
        "continuitydb://evaluation/conflict-evidence"
    );
    assert_eq!(
        cases[1].forbidden_rationale_terms(),
        ["verified in production".to_string()].as_slice()
    );
    assert!(matches!(
        &cases[1].expected_actions()[0],
        StewardAction::LinkRevision {
            kind: RevisionLinkKind::ConflictsWith,
            ..
        }
    ));
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite_exposes_case_contracts --features local-model
```

Expected: FAIL at compile time because the public accessor methods do not exist yet.

- [x] **Step 3: Implement minimal read-only accessors**

Add accessors in `crates/continuitydb-steward/src/local_model.rs`:

```rust
impl LocalModelStewardInput {
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn task(&self) -> &str {
        &self.task
    }

    pub fn evidence(&self) -> &[LocalModelEvidence] {
        &self.evidence
    }
}

impl StewardEvaluationCase {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn input(&self) -> &LocalModelStewardInput {
        &self.input
    }

    pub fn expected_actions(&self) -> &[StewardAction] {
        &self.expected_actions
    }

    pub fn required_citations(&self) -> &[String] {
        &self.required_citations
    }

    pub fn required_rationale_terms(&self) -> &[String] {
        &self.required_rationale_terms
    }

    pub fn forbidden_rationale_terms(&self) -> &[String] {
        &self.forbidden_rationale_terms
    }
}

impl StewardEvaluationSuite {
    pub fn cases(&self) -> &[StewardEvaluationCase] {
        &self.cases
    }

    pub fn len(&self) -> usize {
        self.cases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}
```

- [x] **Step 4: Run focused test to verify it passes**

Run:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite_exposes_case_contracts --features local-model
```

Expected: PASS.

### Task 2: Document Roadmap Slice and Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-evaluation-contract-introspection.md`

- [x] **Step 1: Update README scope**

Add this bullet after the default conflict-classification evaluation bullet:

```markdown
- Public local model evaluation contract introspection.
```

- [x] **Step 2: Update roadmap**

Add Steward milestone 34:

```markdown
34. Add local-model evaluation suite introspection. Implemented read-only accessors for fixed evaluation cases, inputs, expectations, required citations, and rationale constraints so embedders can inspect benchmark contracts before running local models.
```

- [x] **Step 3: Mark this plan complete**

Change every checkbox in this plan from `- [ ]` to `- [x]` after the focused and full verification commands pass.

- [x] **Step 4: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass without warnings or whitespace errors.

- [x] **Step 5: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-evaluation-contract-introspection-design.md docs/superpowers/plans/2026-05-20-local-model-evaluation-contract-introspection.md crates/continuitydb-cli/src/main.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-steward/src/local_model.rs
git commit -m "feat: expose local model evaluation contracts"
```
