# Local Model Response Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a stable local model response schema and GBNF grammar contract for the feature-gated database Steward.

**Architecture:** The contract lives in `continuitydb-steward::local_model` behind the existing `local-model` feature. Public functions return static JSON Schema and GBNF grammar strings; runtime profile helpers expose intent-specific names while reusing existing runner argument generation.

**Tech Stack:** Rust 2021, `serde_json`, existing `continuitydb-steward` local-model feature, existing runtime profile structs.

---

## File Structure

- Modify `crates/continuitydb-steward/src/local_model.rs`: add schema/grammar constants, public accessors, runtime profile helper methods.
- Modify `crates/continuitydb-steward/src/lib.rs`: re-export the contract version and accessors behind `local-model`; add tests.
- Modify `README.md`: record the local model response contract in current scope.
- Modify `docs/roadmap.md`: add a Steward milestone for the published local model response contract.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Contract Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add contract imports to local-model tests**

In the `#[cfg(feature = "local-model")] use super::{ ... }` import list near the top of `crates/continuitydb-steward/src/lib.rs`, add:

```rust
local_model_response_gbnf_grammar, local_model_response_json_schema,
LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
```

- [x] **Step 2: Add JSON Schema contract test**

Add this test after `small_model_candidates_include_default_feasibility_model`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn local_model_response_json_schema_describes_steward_proposals(
) -> Result<(), Box<dyn std::error::Error>> {
    let schema: serde_json::Value = serde_json::from_str(local_model_response_json_schema())?;

    assert_eq!(
        schema["$id"].as_str(),
        Some("https://continuitydb.dev/schemas/local-model-response.schema.json")
    );
    assert_eq!(
        schema["x-continuitydb-schema-version"].as_u64(),
        Some(LOCAL_MODEL_RESPONSE_SCHEMA_VERSION as u64)
    );
    assert_eq!(schema["required"], serde_json::json!(["proposals"]));
    assert!(schema["properties"]["proposals"].is_object());
    assert_eq!(
        schema["$defs"]["action"]["oneOf"][0]["properties"]["type"]["const"].as_str(),
        Some("create_cell_draft")
    );
    assert_eq!(
        schema["$defs"]["action"]["oneOf"][5]["properties"]["type"]["const"].as_str(),
        Some("request_verification")
    );
    Ok(())
}
```

- [x] **Step 3: Add GBNF contract test**

Add this test after the JSON Schema contract test:

```rust
#[cfg(feature = "local-model")]
#[test]
fn local_model_response_gbnf_grammar_describes_proposal_shape() {
    let grammar = local_model_response_gbnf_grammar();

    assert!(grammar.contains("root ::= response"));
    assert!(grammar.contains("response ::= object-start ws proposals-field ws object-end"));
    assert!(grammar.contains("proposal ::= object-start ws action-field"));
    assert!(grammar.contains("action ::= create-cell-draft-action"));
    assert!(grammar.contains("request-verification-action"));
}
```

- [x] **Step 4: Add runtime profile helper tests**

Add this test after `mistral_rs_runtime_profile_builds_deterministic_runner_config`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn runtime_profiles_accept_steward_response_contract_helpers() {
    let llama_config = LlamaCppRuntimeProfile::new("llama-cli", "models/qwen.gguf")
        .with_steward_response_grammar_file("schemas/steward-response.gbnf")
        .runner_config();

    assert!(llama_config
        .command_arguments()
        .windows(2)
        .any(|pair| pair == ["--grammar-file", "schemas/steward-response.gbnf"]));

    let mistral_config = MistralRsRuntimeProfile::new("mistralrs-server", "models/qwen.gguf")
        .with_steward_json_output()
        .runner_config();

    assert!(mistral_config
        .command_arguments()
        .iter()
        .any(|argument| argument == "--json-output"));
}
```

- [x] **Step 5: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward local_model_response --features local-model
```

Expected: FAIL because the public contract functions and helper methods do not exist.

## Task 2: Contract Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add schema version and JSON Schema accessor**

Add near the top of `local_model.rs`, after the imports:

```rust
/// Stable local model response schema version.
pub const LOCAL_MODEL_RESPONSE_SCHEMA_VERSION: u32 = 1;

const LOCAL_MODEL_RESPONSE_JSON_SCHEMA: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://continuitydb.dev/schemas/local-model-response.schema.json",
  "title": "ContinuityDB Local Model Steward Response",
  "type": "object",
  "additionalProperties": false,
  "x-continuitydb-schema-version": 1,
  "required": ["proposals"],
  "properties": {
    "proposals": {
      "type": "array",
      "items": { "$ref": "#/$defs/proposal" }
    }
  },
  "$defs": {
    "proposal": {
      "type": "object",
      "additionalProperties": false,
      "required": ["action", "rationale", "citations"],
      "properties": {
        "action": { "$ref": "#/$defs/action" },
        "rationale": { "type": "string", "minLength": 1 },
        "citations": {
          "type": "array",
          "minItems": 1,
          "items": { "type": "string", "minLength": 1 }
        }
      }
    },
    "action": {
      "oneOf": [
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["type", "anchors", "payload_text"],
          "properties": {
            "type": { "const": "create_cell_draft" },
            "anchors": { "type": "array", "minItems": 1, "items": { "type": "string" } },
            "payload_text": { "type": "string", "minLength": 1 }
          }
        },
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["type", "source", "kind", "target"],
          "properties": {
            "type": { "const": "link_revision" },
            "source": { "type": "string", "format": "uuid" },
            "kind": { "enum": ["predecessor", "supersedes", "conflicts_with", "derives_from"] },
            "target": { "type": "string", "format": "uuid" }
          }
        },
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["type", "cell_id", "proposed_confidence"],
          "properties": {
            "type": { "const": "adjust_confidence" },
            "cell_id": { "type": "string", "format": "uuid" },
            "proposed_confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
          }
        },
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["type", "cell_id", "questions"],
          "properties": {
            "type": { "const": "label_answerability" },
            "cell_id": { "type": "string", "format": "uuid" },
            "questions": { "type": "array", "minItems": 1, "items": { "type": "string", "minLength": 1 } }
          }
        },
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["type", "cell_id"],
          "properties": {
            "type": { "const": "mark_frontier" },
            "cell_id": { "type": "string", "format": "uuid" }
          }
        },
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["type", "request"],
          "properties": {
            "type": { "const": "request_verification" },
            "cell_id": { "type": ["string", "null"], "format": "uuid" },
            "request": { "type": "string", "minLength": 1 }
          }
        }
      ]
    }
  }
}"##;

/// Returns the stable JSON Schema for local model Steward responses.
pub fn local_model_response_json_schema() -> &'static str {
    LOCAL_MODEL_RESPONSE_JSON_SCHEMA
}
```

- [x] **Step 2: Add GBNF grammar accessor**

Add after the JSON Schema accessor:

```rust
const LOCAL_MODEL_RESPONSE_GBNF_GRAMMAR: &str = r#"
root ::= response
response ::= object-start ws proposals-field ws object-end
proposals-field ::= string-proposals ws colon ws array-start ws proposal-list? ws array-end
proposal-list ::= proposal (ws comma ws proposal)*
proposal ::= object-start ws action-field ws comma ws rationale-field ws comma ws citations-field ws object-end
action-field ::= string-action ws colon ws action
action ::= create-cell-draft-action | link-revision-action | adjust-confidence-action | label-answerability-action | mark-frontier-action | request-verification-action
create-cell-draft-action ::= object-start ws type-create-cell-draft ws comma ws anchors-field ws comma ws payload-text-field ws object-end
link-revision-action ::= object-start ws type-link-revision ws comma ws source-field ws comma ws kind-field ws comma ws target-field ws object-end
adjust-confidence-action ::= object-start ws type-adjust-confidence ws comma ws cell-id-field ws comma ws proposed-confidence-field ws object-end
label-answerability-action ::= object-start ws type-label-answerability ws comma ws cell-id-field ws comma ws questions-field ws object-end
mark-frontier-action ::= object-start ws type-mark-frontier ws comma ws cell-id-field ws object-end
request-verification-action ::= object-start ws type-request-verification ws (comma ws cell-id-nullable-field)? ws comma ws request-field ws object-end
anchors-field ::= string-anchors ws colon ws string-array
questions-field ::= string-questions ws colon ws string-array
citations-field ::= string-citations ws colon ws string-array
rationale-field ::= string-rationale ws colon ws string
payload-text-field ::= string-payload-text ws colon ws string
source-field ::= string-source ws colon ws string
target-field ::= string-target ws colon ws string
cell-id-field ::= string-cell-id ws colon ws string
cell-id-nullable-field ::= string-cell-id ws colon ws (string | null)
kind-field ::= string-kind ws colon ws revision-kind
request-field ::= string-request ws colon ws string
proposed-confidence-field ::= string-proposed-confidence ws colon ws number
string-array ::= array-start ws (string (ws comma ws string)*)? ws array-end
revision-kind ::= "\"predecessor\"" | "\"supersedes\"" | "\"conflicts_with\"" | "\"derives_from\""
type-create-cell-draft ::= string-type ws colon ws "\"create_cell_draft\""
type-link-revision ::= string-type ws colon ws "\"link_revision\""
type-adjust-confidence ::= string-type ws colon ws "\"adjust_confidence\""
type-label-answerability ::= string-type ws colon ws "\"label_answerability\""
type-mark-frontier ::= string-type ws colon ws "\"mark_frontier\""
type-request-verification ::= string-type ws colon ws "\"request_verification\""
string-proposals ::= "\"proposals\""
string-action ::= "\"action\""
string-rationale ::= "\"rationale\""
string-citations ::= "\"citations\""
string-type ::= "\"type\""
string-anchors ::= "\"anchors\""
string-payload-text ::= "\"payload_text\""
string-source ::= "\"source\""
string-kind ::= "\"kind\""
string-target ::= "\"target\""
string-cell-id ::= "\"cell_id\""
string-proposed-confidence ::= "\"proposed_confidence\""
string-questions ::= "\"questions\""
string-request ::= "\"request\""
object-start ::= "{"
object-end ::= "}"
array-start ::= "["
array-end ::= "]"
colon ::= ":"
comma ::= ","
null ::= "null"
string ::= "\"" ([^"\\] | "\\" ["\\/bfnrt])* "\""
number ::= "-"? ([0-9] | [1-9] [0-9]*) ("." [0-9]+)?
ws ::= [ \t\n\r]*
"#;

/// Returns a conservative GBNF grammar for local model Steward responses.
pub fn local_model_response_gbnf_grammar() -> &'static str {
    LOCAL_MODEL_RESPONSE_GBNF_GRAMMAR
}
```

- [x] **Step 3: Add runtime profile helper methods**

In `impl LlamaCppRuntimeProfile`, add:

```rust
/// Sets the Steward response grammar file used to constrain JSON output.
pub fn with_steward_response_grammar_file(self, grammar_file: impl Into<PathBuf>) -> Self {
    self.with_grammar_file(grammar_file)
}
```

In `impl MistralRsRuntimeProfile`, add:

```rust
/// Requests Steward JSON output from the runtime wrapper.
pub fn with_steward_json_output(self) -> Self {
    self.with_json_output()
}
```

- [x] **Step 4: Re-export public contract APIs**

In the `#[cfg(feature = "local-model")] pub use local_model::{ ... }` list in `crates/continuitydb-steward/src/lib.rs`, add:

```rust
local_model_response_gbnf_grammar, local_model_response_json_schema,
LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
```

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward local_model_response --features local-model
cargo test -p continuitydb-steward runtime_profiles_accept_steward_response_contract_helpers --features local-model
cargo test -p continuitydb-steward --features local-model
```

Expected: PASS.

## Task 3: Documentation and Full Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-response-contract.md`

- [x] **Step 1: Update README current scope**

Add this bullet after `Local executable Steward model runner.`:

```markdown
- Stable local model Steward response schema and grammar contract.
```

- [x] **Step 2: Update Steward roadmap**

Add this milestone after local model inference and renumber later Steward milestones:

```markdown
6. Publish the local model response contract. Implemented feature-gated JSON Schema and GBNF grammar accessors plus runtime-profile helpers so embedders can constrain real model output before deterministic proposal decoding and policy validation.
```

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit successfully.

- [x] **Step 4: Commit implementation**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-response-contract.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs
git commit -m "feat: publish local model response contract"
```

## Self-Review

- Spec coverage: Tasks cover schema, grammar, profile helpers, exports, tests, README, roadmap, and verification.
- Placeholder scan: No TBD/TODO placeholders remain.
- Type consistency: Function and constant names match across tests, implementation, exports, and docs.
