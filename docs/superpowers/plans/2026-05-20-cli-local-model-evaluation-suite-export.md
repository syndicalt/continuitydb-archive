# CLI Local Model Evaluation Suite Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated CLI command that exports the default local-model Steward evaluation suite contract as deterministic JSON.

**Architecture:** Add a `local-model-evaluation-suite` subcommand in `continuitydb-cli` behind the existing `local-model` feature. Build the JSON from `default_steward_evaluation_suite()` using public read-only accessors, preserving the suite definition in `continuitydb-steward`.

**Tech Stack:** Rust, Clap, serde_json, Cargo workspace tests, existing `local-model` feature.

---

### Task 1: Prove CLI Evaluation Suite Export

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Write the failing CLI test**

Add this test after `cli_benchmark_local_model_records_baseline`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_evaluation_suite_outputs_case_contracts(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-evaluation-suite")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["response_schema_version"].as_u64(), Some(1));
    assert_eq!(json["total_cases"].as_u64(), Some(2));
    assert_eq!(
        json["cases"][0]["name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert_eq!(
        json["cases"][0]["task"].as_str(),
        Some("Assess whether thin evidence needs verification.")
    );
    assert_eq!(
        json["cases"][0]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/thin-evidence")
    );
    assert_eq!(
        json["cases"][0]["expected_actions"][0]["type"].as_str(),
        Some("request_verification")
    );
    assert_eq!(
        json["cases"][0]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/thin-evidence")
    );
    assert_eq!(
        json["cases"][0]["required_rationale_terms"][0].as_str(),
        Some("uncertainty")
    );
    assert_eq!(
        json["cases"][1]["name"].as_str(),
        Some("conflict classification")
    );
    assert_eq!(
        json["cases"][1]["task"].as_str(),
        Some("Classify whether contradictory release-status claims conflict.")
    );
    assert_eq!(
        json["cases"][1]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/conflict-evidence")
    );
    assert_eq!(
        json["cases"][1]["expected_actions"][0]["type"].as_str(),
        Some("link_revision")
    );
    assert_eq!(
        json["cases"][1]["expected_actions"][0]["kind"].as_str(),
        Some("conflicts_with")
    );
    assert_eq!(
        json["cases"][1]["forbidden_rationale_terms"][0].as_str(),
        Some("verified in production")
    );
    Ok(())
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

Expected: FAIL because `local-model-evaluation-suite` is not a recognized subcommand.

### Task 2: Implement CLI Command

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add the feature-gated subcommand**

Add this variant to `enum Command` next to the other local-model commands:

```rust
/// Print the default local Steward model evaluation suite contract as JSON.
#[cfg(feature = "local-model")]
LocalModelEvaluationSuite,
```

- [x] **Step 2: Route the command**

Add this match arm in `run()`:

```rust
#[cfg(feature = "local-model")]
Some(Command::LocalModelEvaluationSuite) => {
    let output = local_model_evaluation_suite_json();
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

- [x] **Step 3: Build JSON from public accessors**

Add this helper near the other local-model helpers:

```rust
#[cfg(feature = "local-model")]
fn local_model_evaluation_suite_json() -> serde_json::Value {
    let suite = default_steward_evaluation_suite();
    let cases: Vec<serde_json::Value> = suite
        .cases()
        .iter()
        .map(|case| {
            let input = case.input();
            let evidence: Vec<serde_json::Value> = input
                .evidence()
                .iter()
                .map(|evidence| {
                    serde_json::json!({
                        "locator": evidence.locator(),
                        "text": evidence.text(),
                    })
                })
                .collect();

            serde_json::json!({
                "name": case.name(),
                "created_at": input.created_at(),
                "task": input.task(),
                "evidence": evidence,
                "expected_actions": case
                    .expected_actions()
                    .iter()
                    .map(local_model_action_json)
                    .collect::<Vec<_>>(),
                "required_citations": case.required_citations(),
                "required_rationale_terms": case.required_rationale_terms(),
                "forbidden_rationale_terms": case.forbidden_rationale_terms(),
            })
        })
        .collect();

    serde_json::json!({
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "total_cases": suite.len(),
        "cases": cases,
    })
}
```

- [x] **Step 4: Add stable action formatting**

Add this helper so the CLI artifact matches the local-model response contract instead of Rust enum serialization:

```rust
#[cfg(feature = "local-model")]
fn local_model_action_json(action: &StewardAction) -> serde_json::Value {
    match action {
        StewardAction::CreateCellDraft {
            anchors,
            payload_text,
        } => serde_json::json!({
            "type": "create_cell_draft",
            "anchors": anchors,
            "payload_text": payload_text,
        }),
        StewardAction::LinkRevision {
            source,
            kind,
            target,
        } => serde_json::json!({
            "type": "link_revision",
            "source": source,
            "kind": local_model_revision_kind(*kind),
            "target": target,
        }),
        StewardAction::RequestVerification { cell_id, request } => serde_json::json!({
            "type": "request_verification",
            "cell_id": cell_id,
            "request": request,
        }),
        StewardAction::AdjustConfidence {
            cell_id,
            proposed_confidence,
        } => serde_json::json!({
            "type": "adjust_confidence",
            "cell_id": cell_id,
            "proposed_confidence": proposed_confidence,
        }),
        StewardAction::LabelAnswerability { cell_id, questions } => serde_json::json!({
            "type": "label_answerability",
            "cell_id": cell_id,
            "questions": questions,
        }),
        StewardAction::MarkFrontier { cell_id } => serde_json::json!({
            "type": "mark_frontier",
            "cell_id": cell_id,
        }),
    }
}

#[cfg(feature = "local-model")]
fn local_model_revision_kind(kind: RevisionLinkKind) -> &'static str {
    match kind {
        RevisionLinkKind::Predecessor => "predecessor",
        RevisionLinkKind::Supersedes => "supersedes",
        RevisionLinkKind::ConflictsWith => "conflicts_with",
        RevisionLinkKind::DerivesFrom => "derives_from",
    }
}
```

- [x] **Step 5: Run focused test to verify it passes**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

Expected: PASS.

### Task 3: Document and Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-evaluation-suite-export.md`

- [x] **Step 1: Update README scope**

Add this bullet after public local model evaluation contract introspection:

```markdown
- CLI local model evaluation suite contract export.
```

- [x] **Step 2: Update roadmap**

Add Steward milestone 35:

```markdown
35. Add CLI local-model evaluation suite export. Implemented a feature-gated `local-model-evaluation-suite` command that prints the fixed benchmark case contracts as JSON using the public evaluation introspection API.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-evaluation-suite-export-design.md docs/superpowers/plans/2026-05-20-cli-local-model-evaluation-suite-export.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: export local model evaluation suite"
```
