//! Feature-gated local model Steward boundary.

use chrono::{DateTime, TimeZone, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    ProposalId, ProposalOutcome, ProposalPolicy, StewardAction, StewardError, StewardIdentity,
    StewardProposal,
};

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

/// Backend that runs local model inference for the database Steward.
pub trait LocalModelBackend {
    /// Runs inference for a prompt and returns a JSON proposal response.
    fn infer(&self, request: LocalModelRequest) -> Result<String, StewardError>;
}

/// Configuration for a local executable model runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExecutableRunnerConfig {
    executable: PathBuf,
    model_path: Option<PathBuf>,
    arguments: Vec<String>,
}

impl LocalExecutableRunnerConfig {
    /// Creates local executable runner configuration.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            model_path: None,
            arguments: Vec::new(),
        }
    }

    /// Sets the model path passed to the executable as `--model <path>`.
    pub fn with_model_path(mut self, model_path: impl Into<PathBuf>) -> Self {
        self.model_path = Some(model_path.into());
        self
    }

    /// Appends an argument passed to the executable after any model path.
    pub fn with_argument(mut self, argument: impl AsRef<OsStr>) -> Self {
        self.arguments
            .push(argument.as_ref().to_string_lossy().to_string());
        self
    }

    /// Returns the executable path.
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns deterministic command arguments.
    pub fn command_arguments(&self) -> Vec<String> {
        let mut arguments = Vec::new();
        if let Some(model_path) = &self.model_path {
            arguments.push("--model".to_string());
            arguments.push(model_path.to_string_lossy().to_string());
        }
        arguments.extend(self.arguments.clone());
        arguments
    }
}

/// Deterministic llama.cpp runtime profile for Steward benchmark runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaCppRuntimeProfile {
    executable: PathBuf,
    model_path: PathBuf,
    context_size: usize,
    temperature: String,
    grammar_file: Option<PathBuf>,
}

impl LlamaCppRuntimeProfile {
    /// Creates a llama.cpp profile with conservative deterministic defaults.
    pub fn new(executable: impl Into<PathBuf>, model_path: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            model_path: model_path.into(),
            context_size: 4096,
            temperature: "0".to_string(),
            grammar_file: None,
        }
    }

    /// Sets the context window size.
    pub fn with_context_size(mut self, context_size: usize) -> Self {
        self.context_size = context_size;
        self
    }

    /// Sets the sampling temperature.
    pub fn with_temperature(mut self, temperature: impl Into<String>) -> Self {
        self.temperature = temperature.into();
        self
    }

    /// Sets the grammar file used to constrain Steward JSON output.
    pub fn with_grammar_file(mut self, grammar_file: impl Into<PathBuf>) -> Self {
        self.grammar_file = Some(grammar_file.into());
        self
    }

    /// Sets the Steward response grammar file used to constrain JSON output.
    pub fn with_steward_response_grammar_file(self, grammar_file: impl Into<PathBuf>) -> Self {
        self.with_grammar_file(grammar_file)
    }

    /// Builds the executable runner configuration for this profile.
    pub fn runner_config(&self) -> LocalExecutableRunnerConfig {
        let mut config = LocalExecutableRunnerConfig::new(self.executable.clone())
            .with_model_path(self.model_path.clone())
            .with_argument("--ctx-size")
            .with_argument(self.context_size.to_string())
            .with_argument("--temp")
            .with_argument(&self.temperature);

        if let Some(grammar_file) = &self.grammar_file {
            config = config
                .with_argument("--grammar-file")
                .with_argument(grammar_file);
        }

        config.with_argument("--prompt").with_argument("-")
    }
}

/// Deterministic mistral.rs runtime profile for Steward benchmark runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MistralRsRuntimeProfile {
    executable: PathBuf,
    model_path: PathBuf,
    context_size: usize,
    temperature: String,
    json_output: bool,
}

impl MistralRsRuntimeProfile {
    /// Creates a mistral.rs profile with conservative deterministic defaults.
    pub fn new(executable: impl Into<PathBuf>, model_path: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            model_path: model_path.into(),
            context_size: 4096,
            temperature: "0".to_string(),
            json_output: false,
        }
    }

    /// Sets the context window size.
    pub fn with_context_size(mut self, context_size: usize) -> Self {
        self.context_size = context_size;
        self
    }

    /// Sets the sampling temperature.
    pub fn with_temperature(mut self, temperature: impl Into<String>) -> Self {
        self.temperature = temperature.into();
        self
    }

    /// Requests JSON output from the runtime wrapper.
    pub fn with_json_output(mut self) -> Self {
        self.json_output = true;
        self
    }

    /// Requests Steward JSON output from the runtime wrapper.
    pub fn with_steward_json_output(self) -> Self {
        self.with_json_output()
    }

    /// Builds the executable runner configuration for this profile.
    pub fn runner_config(&self) -> LocalExecutableRunnerConfig {
        let mut config = LocalExecutableRunnerConfig::new(self.executable.clone())
            .with_model_path(self.model_path.clone())
            .with_argument("--max-seq-len")
            .with_argument(self.context_size.to_string())
            .with_argument("--temperature")
            .with_argument(&self.temperature);

        if self.json_output {
            config = config.with_argument("--json-output");
        }

        config.with_argument("--prompt-stdin")
    }
}

/// Local executable backend for model inference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExecutableRunner {
    config: LocalExecutableRunnerConfig,
}

impl LocalExecutableRunner {
    /// Creates a runner from configuration.
    pub fn new(config: LocalExecutableRunnerConfig) -> Self {
        Self { config }
    }

    /// Returns runner configuration.
    pub fn config(&self) -> &LocalExecutableRunnerConfig {
        &self.config
    }
}

impl LocalModelBackend for LocalExecutableRunner {
    fn infer(&self, request: LocalModelRequest) -> Result<String, StewardError> {
        let mut child = Command::new(self.config.executable())
            .args(self.config.command_arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_error| StewardError::LocalModelExecutionFailed)?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(request.prompt().as_bytes())
                .map_err(|_error| StewardError::LocalModelExecutionFailed)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|_error| StewardError::LocalModelExecutionFailed)?;

        if !output.status.success() {
            return Err(StewardError::LocalModelExecutionFailed);
        }

        String::from_utf8(output.stdout).map_err(|_error| StewardError::LocalModelExecutionFailed)
    }
}

/// Request sent to a local model backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalModelRequest {
    task: String,
    evidence: Vec<LocalModelEvidence>,
    prompt: String,
}

impl LocalModelRequest {
    fn from_input(input: &LocalModelStewardInput) -> Self {
        let prompt = build_prompt(&input.task, &input.evidence);
        Self {
            task: input.task.clone(),
            evidence: input.evidence.clone(),
            prompt,
        }
    }

    /// Returns the Steward task.
    pub fn task(&self) -> &str {
        &self.task
    }

    /// Returns evidence snippets available to the local model.
    pub fn evidence(&self) -> &[LocalModelEvidence] {
        &self.evidence
    }

    /// Returns the deterministic prompt sent to the local model.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
}

/// Renders the deterministic prompt sent to a local model Steward backend.
pub fn local_model_prompt_for_input(input: &LocalModelStewardInput) -> String {
    LocalModelRequest::from_input(input).prompt
}

/// Returns the deterministic fingerprint for ordered prompts rendered from a suite.
pub fn local_model_prompt_fingerprint_for_suite(suite: &StewardEvaluationSuite) -> String {
    prompt_fingerprint_for_suite(suite)
}

/// Evidence snippet supplied to a local model Steward.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalModelEvidence {
    locator: String,
    text: String,
}

impl LocalModelEvidence {
    /// Returns the citation locator for this evidence snippet.
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// Returns the evidence text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Feature-gated Steward that asks a local model backend for proposals.
#[derive(Clone, Debug)]
pub struct LocalModelSteward<B> {
    identity: StewardIdentity,
    backend: B,
}

impl<B> LocalModelSteward<B>
where
    B: LocalModelBackend,
{
    /// Creates a local model Steward with a stable identity and backend.
    pub fn new(identity: StewardIdentity, backend: B) -> Self {
        Self { identity, backend }
    }

    /// Returns the configured backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Runs local model inference and decodes validated Steward proposals.
    pub fn propose(
        &self,
        input: LocalModelStewardInput,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        let created_at = input.created_at;
        let response = self.raw_response(input)?;
        decode_response(&response, self.identity.clone(), created_at)
    }

    /// Runs local model inference and returns the raw backend response.
    pub fn raw_response(&self, input: LocalModelStewardInput) -> Result<String, StewardError> {
        let request = LocalModelRequest::from_input(&input);
        self.backend.infer(request)
    }

    fn decode_raw_response(
        &self,
        input: &LocalModelStewardInput,
        response: &str,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        decode_response(response, self.identity.clone(), input.created_at)
    }
}

/// Input for a feature-gated local model Steward run.
#[derive(Clone, Debug)]
pub struct LocalModelStewardInput {
    created_at: DateTime<Utc>,
    task: String,
    evidence: Vec<LocalModelEvidence>,
}

impl LocalModelStewardInput {
    /// Creates input for a local model Steward run.
    pub fn new(created_at: DateTime<Utc>, task: impl Into<String>) -> Self {
        Self {
            created_at,
            task: task.into(),
            evidence: Vec::new(),
        }
    }

    /// Adds an evidence snippet to the model request.
    pub fn with_evidence(mut self, locator: impl Into<String>, text: impl Into<String>) -> Self {
        self.evidence.push(LocalModelEvidence {
            locator: locator.into(),
            text: text.into(),
        });
        self
    }

    /// Returns the deterministic creation time for proposals from this run.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the task instruction sent to the local model.
    pub fn task(&self) -> &str {
        &self.task
    }

    /// Returns evidence snippets available to the local model.
    pub fn evidence(&self) -> &[LocalModelEvidence] {
        &self.evidence
    }
}

/// Fixed proposal-quality case for local Steward model evaluation.
#[derive(Clone, Debug)]
pub struct StewardEvaluationCase {
    name: String,
    input: LocalModelStewardInput,
    expected_actions: Vec<StewardAction>,
    required_citations: Vec<String>,
    required_rationale_terms: Vec<String>,
    forbidden_rationale_terms: Vec<String>,
}

impl StewardEvaluationCase {
    /// Creates an evaluation case.
    pub fn new(
        name: impl Into<String>,
        created_at: DateTime<Utc>,
        task: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            input: LocalModelStewardInput::new(created_at, task),
            expected_actions: Vec::new(),
            required_citations: Vec::new(),
            required_rationale_terms: Vec::new(),
            forbidden_rationale_terms: Vec::new(),
        }
    }

    /// Adds an evidence snippet to this case's local model input.
    pub fn with_evidence(mut self, locator: impl Into<String>, text: impl Into<String>) -> Self {
        self.input = self.input.with_evidence(locator, text);
        self
    }

    /// Requires at least one emitted proposal to match the expected action.
    pub fn expect_action(mut self, action: StewardAction) -> Self {
        self.expected_actions.push(action);
        self
    }

    /// Requires emitted proposals to include this citation locator.
    pub fn require_citation(mut self, locator: impl Into<String>) -> Self {
        self.required_citations.push(locator.into());
        self
    }

    /// Requires emitted proposal rationales to include this term.
    pub fn require_rationale_term(mut self, term: impl Into<String>) -> Self {
        self.required_rationale_terms.push(term.into());
        self
    }

    /// Rejects emitted proposal rationales containing this unsupported term.
    pub fn forbid_rationale_term(mut self, term: impl Into<String>) -> Self {
        self.forbidden_rationale_terms.push(term.into());
        self
    }

    /// Returns the stable case name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the local model input for this case.
    pub fn input(&self) -> &LocalModelStewardInput {
        &self.input
    }

    /// Returns expected actions that at least one emitted proposal must match.
    pub fn expected_actions(&self) -> &[StewardAction] {
        &self.expected_actions
    }

    /// Returns citation locators that emitted proposals must preserve.
    pub fn required_citations(&self) -> &[String] {
        &self.required_citations
    }

    /// Returns rationale terms that emitted proposals must include.
    pub fn required_rationale_terms(&self) -> &[String] {
        &self.required_rationale_terms
    }

    /// Returns unsupported rationale terms that emitted proposals must avoid.
    pub fn forbidden_rationale_terms(&self) -> &[String] {
        &self.forbidden_rationale_terms
    }
}

/// Deterministic suite for evaluating local Steward model proposal quality.
#[derive(Clone, Debug)]
pub struct StewardEvaluationSuite {
    cases: Vec<StewardEvaluationCase>,
}

impl StewardEvaluationSuite {
    /// Creates an evaluation suite from fixed cases.
    pub fn new(cases: Vec<StewardEvaluationCase>) -> Self {
        Self { cases }
    }

    /// Returns the fixed cases in evaluation order.
    pub fn cases(&self) -> &[StewardEvaluationCase] {
        &self.cases
    }

    /// Returns the number of fixed evaluation cases.
    pub fn len(&self) -> usize {
        self.cases.len()
    }

    /// Returns whether this suite has no evaluation cases.
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }

    /// Returns a deterministic fingerprint for the ordered case contract.
    pub fn fingerprint(&self) -> String {
        let mut fields = vec!["continuitydb.local_model.evaluation_suite.v1".to_string()];
        for case in &self.cases {
            fields.push(format!("case.name={}", case.name()));
            fields.push(format!("case.created_at={}", case.input().created_at()));
            fields.push(format!("case.task={}", case.input().task()));
            for evidence in case.input().evidence() {
                fields.push(format!("evidence.locator={}", evidence.locator()));
                fields.push(format!("evidence.text={}", evidence.text()));
            }
            for action in case.expected_actions() {
                let encoded =
                    serde_json::to_string(action).unwrap_or_else(|_error| format!("{action:?}"));
                fields.push(format!("expected_action={encoded}"));
            }
            for citation in case.required_citations() {
                fields.push(format!("required_citation={citation}"));
            }
            for term in case.required_rationale_terms() {
                fields.push(format!("required_rationale_term={term}"));
            }
            for term in case.forbidden_rationale_terms() {
                fields.push(format!("forbidden_rationale_term={term}"));
            }
        }

        fingerprint_fields(&fields)
    }

    /// Evaluates a local model Steward against all cases.
    pub fn evaluate<B>(&self, steward: &LocalModelSteward<B>) -> StewardEvaluationReport
    where
        B: LocalModelBackend,
    {
        let case_reports = self
            .cases
            .iter()
            .map(|case| evaluate_case(case, steward))
            .collect();

        StewardEvaluationReport { case_reports }
    }

    /// Evaluates all cases while preserving raw local model responses.
    pub fn evaluate_with_responses<B>(
        &self,
        steward: &LocalModelSteward<B>,
    ) -> (StewardEvaluationReport, Vec<StewardEvaluationCaseResponse>)
    where
        B: LocalModelBackend,
    {
        let evaluated: Vec<(StewardEvaluationCaseReport, StewardEvaluationCaseResponse)> = self
            .cases
            .iter()
            .map(|case| evaluate_case_with_response(case, steward))
            .collect();
        let case_reports = evaluated
            .iter()
            .map(|(report, _response)| report.clone())
            .collect();
        let responses = evaluated
            .into_iter()
            .map(|(_report, response)| response)
            .collect();

        (StewardEvaluationReport { case_reports }, responses)
    }
}

/// Raw response captured while evaluating one local Steward model case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StewardEvaluationCaseResponse {
    case_name: String,
    response: Option<String>,
}

impl StewardEvaluationCaseResponse {
    fn captured(case_name: String, response: String) -> Self {
        Self {
            case_name,
            response: Some(response),
        }
    }

    fn missing(case_name: String) -> Self {
        Self {
            case_name,
            response: None,
        }
    }

    /// Returns the evaluated case name.
    pub fn case_name(&self) -> &str {
        &self.case_name
    }

    /// Returns the raw model response when inference produced one.
    pub fn response(&self) -> Option<&str> {
        self.response.as_deref()
    }

    /// Returns the raw response byte length, or zero when no response was captured.
    pub fn response_bytes(&self) -> usize {
        self.response.as_ref().map_or(0, String::len)
    }
}

/// Durable response identity metadata for one evaluated local Steward model case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalModelResponseFingerprint {
    case_name: String,
    captured: bool,
    response_fingerprint: Option<String>,
    response_bytes: usize,
}

impl LocalModelResponseFingerprint {
    fn from_response(response: &StewardEvaluationCaseResponse) -> Self {
        Self {
            case_name: response.case_name().to_string(),
            captured: response.response().is_some(),
            response_fingerprint: response.response().map(fingerprint_text),
            response_bytes: response.response_bytes(),
        }
    }

    /// Returns the evaluated case name.
    pub fn case_name(&self) -> &str {
        &self.case_name
    }

    /// Returns whether raw stdout was captured for this case.
    pub fn captured(&self) -> bool {
        self.captured
    }

    /// Returns the deterministic raw response fingerprint when captured.
    pub fn response_fingerprint(&self) -> Option<&str> {
        self.response_fingerprint.as_deref()
    }

    /// Returns the captured response byte count.
    pub fn response_bytes(&self) -> usize {
        self.response_bytes
    }
}

/// Deterministic evaluation report for an entire suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StewardEvaluationReport {
    case_reports: Vec<StewardEvaluationCaseReport>,
}

impl StewardEvaluationReport {
    /// Returns whether every case passed.
    pub fn passed(&self) -> bool {
        self.summary().passed()
    }

    /// Returns per-case evaluation reports.
    pub fn case_reports(&self) -> &[StewardEvaluationCaseReport] {
        &self.case_reports
    }

    /// Returns deterministic aggregate evaluation metrics.
    pub fn summary(&self) -> StewardEvaluationSummary {
        StewardEvaluationSummary::from_report(self)
    }
}

/// Deterministic aggregate metrics for a local Steward model evaluation report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StewardEvaluationSummary {
    total_cases: usize,
    passed_cases: usize,
    failed_cases: usize,
}

impl StewardEvaluationSummary {
    fn from_report(report: &StewardEvaluationReport) -> Self {
        let total_cases = report.case_reports().len();
        let passed_cases = report
            .case_reports()
            .iter()
            .filter(|case_report| case_report.passed())
            .count();
        let failed_cases = total_cases - passed_cases;

        Self {
            total_cases,
            passed_cases,
            failed_cases,
        }
    }

    /// Returns the number of evaluated cases.
    pub fn total_cases(&self) -> usize {
        self.total_cases
    }

    /// Returns the number of passing cases.
    pub fn passed_cases(&self) -> usize {
        self.passed_cases
    }

    /// Returns the number of failing cases.
    pub fn failed_cases(&self) -> usize {
        self.failed_cases
    }

    /// Returns passed cases divided by total cases, or 1.0 for an empty suite.
    pub fn pass_rate(&self) -> f64 {
        if self.total_cases == 0 {
            return 1.0;
        }

        self.passed_cases as f64 / self.total_cases as f64
    }

    /// Returns whether no evaluation cases failed.
    pub fn passed(&self) -> bool {
        self.failed_cases == 0
    }
}

/// Deterministic evaluation report for one case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StewardEvaluationCaseReport {
    name: String,
    failures: Vec<StewardEvaluationFailure>,
}

impl StewardEvaluationCaseReport {
    /// Returns the case name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this case passed.
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    /// Returns deterministic failure reasons.
    pub fn failures(&self) -> &[StewardEvaluationFailure] {
        &self.failures
    }
}

/// Deterministic local model evaluation failure reason.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StewardEvaluationFailure {
    /// The local model backend or decoder failed.
    ModelError,
    /// No emitted proposal matched an expected action.
    MissingExpectedAction {
        /// Expected action that was not emitted.
        action: StewardAction,
    },
    /// Required evidence citation was not preserved.
    MissingCitation {
        /// Missing citation locator.
        locator: String,
    },
    /// Required rationale term was not found in emitted proposal rationales.
    MissingRationaleTerm {
        /// Required term missing from rationale text.
        term: String,
    },
    /// A proposal was rejected by deterministic policy.
    PolicyRejected {
        /// Rejection reasons emitted by policy.
        reasons: Vec<String>,
    },
    /// A proposal rationale included an unsupported claim marker.
    UnsupportedRationaleTerm {
        /// Forbidden term found in rationale text.
        term: String,
    },
}

/// Small embeddable open-source model candidate metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmallModelCandidate {
    model_id: &'static str,
    role: &'static str,
    recommended_runtime: &'static str,
    artifact_format: &'static str,
    recommended_temperature_millis: u16,
    requires_grammar: bool,
    notes: &'static str,
}

impl SmallModelCandidate {
    /// Returns the model identifier.
    pub fn model_id(&self) -> &'static str {
        self.model_id
    }

    /// Returns the candidate evaluation role.
    pub fn role(&self) -> &'static str {
        self.role
    }

    /// Returns the recommended local inference runtime for this candidate.
    pub fn recommended_runtime(&self) -> &'static str {
        self.recommended_runtime
    }

    /// Returns the expected local model artifact format.
    pub fn artifact_format(&self) -> &'static str {
        self.artifact_format
    }

    /// Returns the recommended benchmark temperature for stable proposal output.
    pub fn recommended_temperature(&self) -> f32 {
        f32::from(self.recommended_temperature_millis) / 1000.0
    }

    /// Returns whether this candidate should be run with the Steward grammar.
    pub fn requires_grammar(&self) -> bool {
        self.requires_grammar
    }

    /// Returns operational notes for evaluating this candidate.
    pub fn notes(&self) -> &'static str {
        self.notes
    }

    /// Builds the recommended local executable runner configuration.
    pub fn recommended_runner_config(
        &self,
        executable: impl Into<PathBuf>,
        model_path: impl Into<PathBuf>,
    ) -> LocalExecutableRunnerConfig {
        LlamaCppRuntimeProfile::new(executable, model_path)
            .with_temperature(format_temperature(self.recommended_temperature_millis))
            .runner_config()
    }
}

/// Returns the fixed small model candidates for Steward evaluation.
pub fn small_model_candidates() -> &'static [SmallModelCandidate] {
    &[
        SmallModelCandidate {
            model_id: "Qwen/Qwen2.5-0.5B-Instruct",
            role: "default-feasibility",
            recommended_runtime: "llama.cpp",
            artifact_format: "GGUF",
            recommended_temperature_millis: 0,
            requires_grammar: true,
            notes: "Smallest default feasibility candidate; evaluate first with grammar-constrained JSON output.",
        },
        SmallModelCandidate {
            model_id: "Qwen/Qwen3-0.6B",
            role: "current-reasoning",
            recommended_runtime: "llama.cpp",
            artifact_format: "GGUF",
            recommended_temperature_millis: 0,
            requires_grammar: true,
            notes: "Reasoning comparison candidate; run with thinking disabled or constrained for deterministic benchmarks.",
        },
        SmallModelCandidate {
            model_id: "HuggingFaceTB/SmolLM2-360M-Instruct",
            role: "ultra-small-experimental",
            recommended_runtime: "llama.cpp",
            artifact_format: "GGUF",
            recommended_temperature_millis: 0,
            requires_grammar: true,
            notes: "Lower-bound experimental candidate; use to measure minimum viable constrained proposal quality.",
        },
        SmallModelCandidate {
            model_id: "HuggingFaceTB/SmolLM2-135M-Instruct",
            role: "smoke-test-only",
            recommended_runtime: "llama.cpp",
            artifact_format: "GGUF",
            recommended_temperature_millis: 0,
            requires_grammar: true,
            notes: "Smoke-test candidate only; do not treat passing toy output as production stewardship quality.",
        },
    ]
}

/// Returns the fixed default proposal-quality suite for local Steward model baselines.
pub fn default_steward_evaluation_suite() -> StewardEvaluationSuite {
    let created_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .unwrap_or_else(|| DateTime::<Utc>::from(std::time::UNIX_EPOCH));
    let conflict_source = StateCellId::from_u128(1);
    let conflict_target = StateCellId::from_u128(2);
    let frontier_cell = StateCellId::from_u128(3);
    let supersession_source = StateCellId::from_u128(4);
    let supersession_target = StateCellId::from_u128(5);
    let confidence_cell = StateCellId::from_u128(6);
    let targeted_verification_cell = StateCellId::from_u128(7);

    StewardEvaluationSuite::new(vec![
        StewardEvaluationCase::new(
            "insufficient evidence uncertainty",
            created_at,
            "Assess whether thin evidence needs verification.",
        )
        .with_evidence(
            "continuitydb://evaluation/thin-evidence",
            "One weak source mentions the claim without corroboration.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: None,
            request: "Gather additional source evidence.".to_string(),
        })
        .require_citation("continuitydb://evaluation/thin-evidence")
        .require_rationale_term("uncertainty"),
        StewardEvaluationCase::new(
            "conflict classification",
            created_at,
            "Classify whether contradictory release-status claims conflict.",
        )
        .with_evidence(
            "continuitydb://evaluation/conflict-evidence",
            "The source claim says the release is blocked while the target claim says it shipped.",
        )
        .expect_action(StewardAction::LinkRevision {
            source: conflict_source,
            kind: RevisionLinkKind::ConflictsWith,
            target: conflict_target,
        })
        .require_citation("continuitydb://evaluation/conflict-evidence")
        .forbid_rationale_term("verified in production"),
        StewardEvaluationCase::new(
            "supersession classification",
            created_at,
            "Classify whether newer release evidence supersedes the older status.",
        )
        .with_evidence(
            "continuitydb://evaluation/supersession-evidence",
            "The source claim updates the target release status with newer evidence but does not contradict the older state.",
        )
        .expect_action(StewardAction::LinkRevision {
            source: supersession_source,
            kind: RevisionLinkKind::Supersedes,
            target: supersession_target,
        })
        .require_citation("continuitydb://evaluation/supersession-evidence")
        .require_rationale_term("supersedes")
        .forbid_rationale_term("conflicts with"),
        StewardEvaluationCase::new(
            "unsupported claim boundary",
            created_at,
            "Check whether release evidence supports a shipped deployment claim.",
        )
        .with_evidence(
            "continuitydb://evaluation/unsupported-release-claim",
            "The release notes say the package was built locally; no production deployment evidence is available.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: None,
            request: "Verify deployment status before treating the release as shipped.".to_string(),
        })
        .require_citation("continuitydb://evaluation/unsupported-release-claim")
        .require_rationale_term("unsupported")
        .forbid_rationale_term("deployed to all customers"),
        StewardEvaluationCase::new(
            "confidence adjustment",
            created_at,
            "Adjust confidence for stale deployment status evidence.",
        )
        .with_evidence(
            "continuitydb://evaluation/confidence-evidence",
            "Deployment status evidence is stale and should lower confidence until a newer production signal is available.",
        )
        .expect_action(StewardAction::AdjustConfidence {
            cell_id: confidence_cell,
            proposed_confidence: 0.42,
        })
        .require_citation("continuitydb://evaluation/confidence-evidence")
        .require_rationale_term("confidence")
        .forbid_rationale_term("fully trusted"),
        StewardEvaluationCase::new(
            "targeted verification request",
            created_at,
            "Request verification for a stale high-impact frontier cell.",
        )
        .with_evidence(
            "continuitydb://evaluation/targeted-verification-evidence",
            "A high-impact frontier cell has stale evidence and needs a current source refresh before downstream decisions depend on it.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: Some(targeted_verification_cell),
            request: "Refresh the stale high-impact frontier signal.".to_string(),
        })
        .require_citation("continuitydb://evaluation/targeted-verification-evidence")
        .require_rationale_term("refresh")
        .forbid_rationale_term("no target"),
        StewardEvaluationCase::new(
            "new evidence draft creation",
            created_at,
            "Draft a StateCell from new benchmark evidence.",
        )
        .with_evidence(
            "continuitydb://evaluation/new-benchmark-evidence",
            "A local Steward benchmark produced a new result that should be captured as a draft StateCell for review.",
        )
        .expect_action(StewardAction::CreateCellDraft {
            anchors: vec![SemanticAnchor::new("project:continuitydb:benchmark-result")],
            payload_text:
                "ContinuityDB local Steward benchmark produced a new result requiring review."
                    .to_string(),
        })
        .require_citation("continuitydb://evaluation/new-benchmark-evidence")
        .require_rationale_term("draft")
        .forbid_rationale_term("committed"),
        StewardEvaluationCase::new(
            "multi-source citation preservation",
            created_at,
            "Decide whether a release-status change should stay on the active frontier.",
        )
        .with_evidence(
            "continuitydb://evaluation/release-build-source",
            "Build evidence says release candidate 0.3.0 was produced but not deployed.",
        )
        .with_evidence(
            "continuitydb://evaluation/release-incident-source",
            "Incident evidence says deployment was paused after a packaging regression.",
        )
        .expect_action(StewardAction::MarkFrontier {
            cell_id: frontier_cell,
        })
        .require_citation("continuitydb://evaluation/release-build-source")
        .require_citation("continuitydb://evaluation/release-incident-source")
        .require_rationale_term("frontier"),
        StewardEvaluationCase::new(
            "policy rejection avoidance",
            created_at,
            "Handle invalid answerability-label evidence without emitting an invalid label.",
        )
        .with_evidence(
            "continuitydb://evaluation/invalid-answerability-label",
            "A proposed answerability update has an empty question list and must not be applied as-is.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: None,
            request: "Ask for a concrete answerability question before labeling the cell."
                .to_string(),
        })
        .require_citation("continuitydb://evaluation/invalid-answerability-label")
        .require_rationale_term("invalid")
        .forbid_rationale_term("label applied"),
    ])
}

/// Reproducible local model executable invocation metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalModelRuntimeManifest {
    executable: String,
    arguments: Vec<String>,
}

impl LocalModelRuntimeManifest {
    fn from_runner_config(config: &LocalExecutableRunnerConfig) -> Self {
        Self {
            executable: config.executable().to_string_lossy().to_string(),
            arguments: config.command_arguments(),
        }
    }

    /// Returns the executable path used for the local model run.
    pub fn executable(&self) -> &str {
        &self.executable
    }

    /// Returns the deterministic executable arguments used for the local model run.
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

/// Executable local model benchmark fixture for a fixed Steward evaluation suite.
#[derive(Clone, Debug)]
pub struct LocalModelBenchmark {
    candidate: SmallModelCandidate,
    runner: LocalExecutableRunner,
    suite: StewardEvaluationSuite,
}

impl LocalModelBenchmark {
    /// Creates a benchmark fixture from candidate metadata, runner, and suite.
    pub fn new(
        candidate: SmallModelCandidate,
        runner: LocalExecutableRunner,
        suite: StewardEvaluationSuite,
    ) -> Self {
        Self {
            candidate,
            runner,
            suite,
        }
    }

    /// Returns the benchmark candidate metadata.
    pub fn candidate(&self) -> SmallModelCandidate {
        self.candidate
    }

    /// Returns the executable runner used by this benchmark.
    pub fn runner(&self) -> &LocalExecutableRunner {
        &self.runner
    }

    /// Runs the benchmark suite with the supplied Steward identity.
    pub fn run(&self, identity: StewardIdentity) -> LocalModelBenchmarkReport {
        let steward = LocalModelSteward::new(identity, self.runner.clone());
        let (evaluation, responses) = self.suite.evaluate_with_responses(&steward);
        self.report_from_evaluation(evaluation, &responses)
    }

    /// Runs the benchmark suite and returns raw per-case model responses.
    pub fn run_with_responses(
        &self,
        identity: StewardIdentity,
    ) -> (
        LocalModelBenchmarkReport,
        Vec<StewardEvaluationCaseResponse>,
    ) {
        let steward = LocalModelSteward::new(identity, self.runner.clone());
        let (evaluation, responses) = self.suite.evaluate_with_responses(&steward);
        let report = self.report_from_evaluation(evaluation, &responses);
        (report, responses)
    }

    fn report_from_evaluation(
        &self,
        evaluation: StewardEvaluationReport,
        responses: &[StewardEvaluationCaseResponse],
    ) -> LocalModelBenchmarkReport {
        LocalModelBenchmarkReport {
            candidate: self.candidate,
            response_schema_version: LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
            evaluation_suite_fingerprint: self.suite.fingerprint(),
            schema_fingerprint: fingerprint_text(local_model_response_json_schema()),
            grammar_fingerprint: fingerprint_text(local_model_response_gbnf_grammar()),
            prompt_fingerprint: prompt_fingerprint_for_suite(&self.suite),
            runtime: LocalModelRuntimeManifest::from_runner_config(self.runner.config()),
            response_fingerprints: responses
                .iter()
                .map(LocalModelResponseFingerprint::from_response)
                .collect(),
            evaluation,
        }
    }

    /// Re-runs each benchmark case and reports whether decoded proposal output is stable.
    pub fn run_stability(
        &self,
        identity: StewardIdentity,
        trials: usize,
    ) -> LocalModelStabilityReport {
        let trial_count = trials.max(1);
        let mut proposal_fingerprints_by_case: Vec<Vec<String>> =
            vec![Vec::new(); self.suite.cases().len()];

        for _trial in 0..trial_count {
            let steward = LocalModelSteward::new(identity.clone(), self.runner.clone());
            for (case_index, case) in self.suite.cases().iter().enumerate() {
                proposal_fingerprints_by_case[case_index]
                    .push(stability_fingerprint_for_case(case, &steward));
            }
        }

        let case_reports = self
            .suite
            .cases()
            .iter()
            .zip(proposal_fingerprints_by_case)
            .map(|(case, proposal_fingerprints)| {
                LocalModelStabilityCaseReport::new(case.name().to_string(), proposal_fingerprints)
            })
            .collect();

        LocalModelStabilityReport::new(trial_count, case_reports)
    }
}

/// Stability report for repeated local model benchmark outputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelStabilityReport {
    trials: usize,
    stable: bool,
    case_reports: Vec<LocalModelStabilityCaseReport>,
}

impl LocalModelStabilityReport {
    fn new(trials: usize, case_reports: Vec<LocalModelStabilityCaseReport>) -> Self {
        let stable = case_reports
            .iter()
            .all(LocalModelStabilityCaseReport::stable);

        Self {
            trials,
            stable,
            case_reports,
        }
    }

    /// Returns the number of repeated trials.
    pub fn trials(&self) -> usize {
        self.trials
    }

    /// Returns whether every case emitted stable decoded proposals.
    pub fn stable(&self) -> bool {
        self.stable
    }

    /// Returns per-case stability reports.
    pub fn case_reports(&self) -> &[LocalModelStabilityCaseReport] {
        &self.case_reports
    }
}

/// Per-case stability report for repeated local model benchmark outputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelStabilityCaseReport {
    name: String,
    stable: bool,
    proposal_fingerprints: Vec<String>,
    changed_trials: Vec<usize>,
}

impl LocalModelStabilityCaseReport {
    fn new(name: String, proposal_fingerprints: Vec<String>) -> Self {
        let first = proposal_fingerprints.first();
        let changed_trials: Vec<usize> = proposal_fingerprints
            .iter()
            .enumerate()
            .filter_map(|(index, fingerprint)| {
                first
                    .is_some_and(|first| first != fingerprint)
                    .then_some(index + 1)
            })
            .collect();
        let stable = changed_trials.is_empty();

        Self {
            name,
            stable,
            proposal_fingerprints,
            changed_trials,
        }
    }

    /// Returns the evaluated case name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this case emitted stable decoded proposals.
    pub fn stable(&self) -> bool {
        self.stable
    }

    /// Returns the decoded proposal fingerprints for each trial.
    pub fn proposal_fingerprints(&self) -> &[String] {
        &self.proposal_fingerprints
    }

    /// Returns one-based trial numbers whose proposal fingerprint changed.
    pub fn changed_trials(&self) -> &[usize] {
        &self.changed_trials
    }
}

fn stability_fingerprint_for_case<B>(
    case: &StewardEvaluationCase,
    steward: &LocalModelSteward<B>,
) -> String
where
    B: LocalModelBackend,
{
    match steward.propose(case.input().clone()) {
        Ok(proposals) => {
            let mut fields = vec!["ok".to_string()];
            for proposal in proposals {
                fields.push(stability_field_for_proposal(&proposal));
            }
            fingerprint_fields(&fields)
        }
        Err(error) => fingerprint_fields(&["error".to_string(), format!("{error:?}")]),
    }
}

fn stability_field_for_proposal(proposal: &StewardProposal) -> String {
    serde_json::to_string(&serde_json::json!({
        "action": proposal.action(),
        "rationale": proposal.rationale(),
        "citations": proposal.citations(),
        "created_at": proposal.created_at(),
    }))
    .unwrap_or_else(|_error| format!("{proposal:?}"))
}

/// Report emitted by an executable local model benchmark run.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelBenchmarkReport {
    candidate: SmallModelCandidate,
    response_schema_version: u32,
    evaluation_suite_fingerprint: String,
    schema_fingerprint: String,
    grammar_fingerprint: String,
    prompt_fingerprint: String,
    runtime: LocalModelRuntimeManifest,
    response_fingerprints: Vec<LocalModelResponseFingerprint>,
    evaluation: StewardEvaluationReport,
}

impl LocalModelBenchmarkReport {
    /// Returns the evaluated model candidate metadata.
    pub fn candidate(&self) -> SmallModelCandidate {
        self.candidate
    }

    /// Returns the local model response schema version used for decoding.
    pub fn response_schema_version(&self) -> u32 {
        self.response_schema_version
    }

    /// Returns the deterministic fingerprint for the evaluated suite contract.
    pub fn evaluation_suite_fingerprint(&self) -> &str {
        &self.evaluation_suite_fingerprint
    }

    /// Returns the deterministic fingerprint for the response JSON Schema text.
    pub fn schema_fingerprint(&self) -> &str {
        &self.schema_fingerprint
    }

    /// Returns the deterministic fingerprint for the response GBNF grammar text.
    pub fn grammar_fingerprint(&self) -> &str {
        &self.grammar_fingerprint
    }

    /// Returns the deterministic fingerprint for the rendered prompt contract.
    pub fn prompt_fingerprint(&self) -> &str {
        &self.prompt_fingerprint
    }

    /// Returns the runtime manifest for the evaluated local model invocation.
    pub fn runtime(&self) -> &LocalModelRuntimeManifest {
        &self.runtime
    }

    /// Returns durable per-case raw response fingerprint metadata.
    pub fn response_fingerprints(&self) -> &[LocalModelResponseFingerprint] {
        &self.response_fingerprints
    }

    /// Returns the proposal-quality evaluation report.
    pub fn evaluation(&self) -> &StewardEvaluationReport {
        &self.evaluation
    }

    /// Returns deterministic aggregate evaluation metrics.
    pub fn evaluation_summary(&self) -> StewardEvaluationSummary {
        self.evaluation.summary()
    }

    /// Returns whether every benchmark case passed.
    pub fn passed(&self) -> bool {
        self.evaluation_summary().passed()
    }
}

/// Durable baseline record for an executable local model benchmark run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelBenchmarkBaseline {
    candidate_model_id: String,
    candidate_role: String,
    #[serde(default)]
    response_schema_version: u32,
    #[serde(default)]
    evaluation_suite_fingerprint: String,
    #[serde(default)]
    schema_fingerprint: String,
    #[serde(default)]
    grammar_fingerprint: String,
    #[serde(default)]
    prompt_fingerprint: String,
    #[serde(default)]
    runtime: LocalModelRuntimeManifest,
    #[serde(default)]
    response_fingerprints: Vec<LocalModelResponseFingerprint>,
    evaluation: StewardEvaluationReport,
    recorded_at: DateTime<Utc>,
}

impl LocalModelBenchmarkBaseline {
    /// Creates a durable baseline record from a benchmark report and timestamp.
    pub fn from_report(report: LocalModelBenchmarkReport, recorded_at: DateTime<Utc>) -> Self {
        Self {
            candidate_model_id: report.candidate.model_id().to_string(),
            candidate_role: report.candidate.role().to_string(),
            response_schema_version: report.response_schema_version,
            evaluation_suite_fingerprint: report.evaluation_suite_fingerprint,
            schema_fingerprint: report.schema_fingerprint,
            grammar_fingerprint: report.grammar_fingerprint,
            prompt_fingerprint: report.prompt_fingerprint,
            runtime: report.runtime,
            response_fingerprints: report.response_fingerprints,
            evaluation: report.evaluation,
            recorded_at,
        }
    }

    /// Returns the evaluated model identifier.
    pub fn candidate_model_id(&self) -> &str {
        &self.candidate_model_id
    }

    /// Returns the evaluated model role.
    pub fn candidate_role(&self) -> &str {
        &self.candidate_role
    }

    /// Returns the local model response schema version used for this baseline.
    pub fn response_schema_version(&self) -> u32 {
        self.response_schema_version
    }

    /// Returns the deterministic fingerprint for the evaluated suite contract.
    pub fn evaluation_suite_fingerprint(&self) -> &str {
        &self.evaluation_suite_fingerprint
    }

    /// Returns the deterministic fingerprint for the response JSON Schema text.
    pub fn schema_fingerprint(&self) -> &str {
        &self.schema_fingerprint
    }

    /// Returns the deterministic fingerprint for the response GBNF grammar text.
    pub fn grammar_fingerprint(&self) -> &str {
        &self.grammar_fingerprint
    }

    /// Returns the deterministic fingerprint for the rendered prompt contract.
    pub fn prompt_fingerprint(&self) -> &str {
        &self.prompt_fingerprint
    }

    /// Returns the runtime manifest that produced this baseline.
    pub fn runtime(&self) -> &LocalModelRuntimeManifest {
        &self.runtime
    }

    /// Returns durable per-case raw response fingerprint metadata.
    pub fn response_fingerprints(&self) -> &[LocalModelResponseFingerprint] {
        &self.response_fingerprints
    }

    /// Returns the recorded evaluation report.
    pub fn evaluation(&self) -> &StewardEvaluationReport {
        &self.evaluation
    }

    /// Returns deterministic aggregate evaluation metrics.
    pub fn evaluation_summary(&self) -> StewardEvaluationSummary {
        self.evaluation.summary()
    }

    /// Returns when this baseline was recorded.
    pub fn recorded_at(&self) -> DateTime<Utc> {
        self.recorded_at
    }

    /// Returns whether every benchmark case passed.
    pub fn passed(&self) -> bool {
        self.evaluation_summary().passed()
    }
}

/// Deterministic comparison between two local model benchmark baselines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelBenchmarkRegression {
    candidate_model_id: String,
    candidate_role: String,
    previous_recorded_at: DateTime<Utc>,
    current_recorded_at: DateTime<Utc>,
    previous_passed_cases: usize,
    current_passed_cases: usize,
    pass_count_delta: isize,
    regressed: bool,
}

impl LocalModelBenchmarkRegression {
    /// Compares a previous baseline with a current baseline for regression gating.
    pub fn compare(
        previous: &LocalModelBenchmarkBaseline,
        current: &LocalModelBenchmarkBaseline,
    ) -> Self {
        let previous_passed_cases = previous.evaluation_summary().passed_cases();
        let current_passed_cases = current.evaluation_summary().passed_cases();
        let pass_count_delta = current_passed_cases as isize - previous_passed_cases as isize;
        let regressed = current_passed_cases < previous_passed_cases
            || (previous.passed() && !current.passed());

        Self {
            candidate_model_id: current.candidate_model_id().to_string(),
            candidate_role: current.candidate_role().to_string(),
            previous_recorded_at: previous.recorded_at(),
            current_recorded_at: current.recorded_at(),
            previous_passed_cases,
            current_passed_cases,
            pass_count_delta,
            regressed,
        }
    }

    /// Returns the evaluated model identifier.
    pub fn candidate_model_id(&self) -> &str {
        &self.candidate_model_id
    }

    /// Returns the evaluated model role.
    pub fn candidate_role(&self) -> &str {
        &self.candidate_role
    }

    /// Returns when the previous baseline was recorded.
    pub fn previous_recorded_at(&self) -> DateTime<Utc> {
        self.previous_recorded_at
    }

    /// Returns when the current baseline was recorded.
    pub fn current_recorded_at(&self) -> DateTime<Utc> {
        self.current_recorded_at
    }

    /// Returns the number of passing cases in the previous baseline.
    pub fn previous_passed_cases(&self) -> usize {
        self.previous_passed_cases
    }

    /// Returns the number of passing cases in the current baseline.
    pub fn current_passed_cases(&self) -> usize {
        self.current_passed_cases
    }

    /// Returns current passing cases minus previous passing cases.
    pub fn pass_count_delta(&self) -> isize {
        self.pass_count_delta
    }

    /// Returns whether the current baseline regressed versus the previous baseline.
    pub fn regressed(&self) -> bool {
        self.regressed
    }
}

/// Result of recording a current benchmark baseline and comparing with prior state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalModelBenchmarkGateReport {
    current_baseline: LocalModelBenchmarkBaseline,
    regression: Option<LocalModelBenchmarkRegression>,
}

impl LocalModelBenchmarkGateReport {
    /// Creates a local model benchmark gate report.
    pub fn new(
        current_baseline: LocalModelBenchmarkBaseline,
        regression: Option<LocalModelBenchmarkRegression>,
    ) -> Self {
        Self {
            current_baseline,
            regression,
        }
    }

    /// Returns the newly recorded baseline.
    pub fn current_baseline(&self) -> &LocalModelBenchmarkBaseline {
        &self.current_baseline
    }

    /// Returns the regression comparison, if a previous baseline existed.
    pub fn regression(&self) -> Option<&LocalModelBenchmarkRegression> {
        self.regression.as_ref()
    }

    /// Returns whether the current baseline regressed against the previous baseline.
    pub fn regressed(&self) -> bool {
        self.regression
            .as_ref()
            .is_some_and(LocalModelBenchmarkRegression::regressed)
    }
}

/// Runs a configured local model benchmark and records the resulting baseline.
pub fn record_local_model_benchmark_baseline<S>(
    benchmark: &LocalModelBenchmark,
    identity: StewardIdentity,
    recorded_at: DateTime<Utc>,
    store: &mut S,
) -> Result<LocalModelBenchmarkBaseline, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    let baseline = LocalModelBenchmarkBaseline::from_report(benchmark.run(identity), recorded_at);
    store.append_baseline(baseline.clone())?;
    Ok(baseline)
}

/// Records a benchmark baseline and compares it with the latest previous baseline.
pub fn record_local_model_benchmark_baseline_with_regression<S>(
    benchmark: &LocalModelBenchmark,
    identity: StewardIdentity,
    recorded_at: DateTime<Utc>,
    store: &mut S,
) -> Result<LocalModelBenchmarkGateReport, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    let current = LocalModelBenchmarkBaseline::from_report(benchmark.run(identity), recorded_at);
    let previous = latest_compatible_local_model_benchmark_baseline(store, &current)?;
    let regression = previous
        .as_ref()
        .map(|previous| LocalModelBenchmarkRegression::compare(previous, &current));
    store.append_baseline(current.clone())?;

    Ok(LocalModelBenchmarkGateReport::new(current, regression))
}

/// Returns the newest stored benchmark baseline for a model candidate.
pub fn latest_local_model_benchmark_baseline<S>(
    store: &S,
    candidate: SmallModelCandidate,
) -> Result<Option<LocalModelBenchmarkBaseline>, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    Ok(store
        .list_baselines()?
        .into_iter()
        .filter(|baseline| baseline.candidate_model_id() == candidate.model_id())
        .filter(|baseline| baseline.candidate_role() == candidate.role())
        .max_by_key(LocalModelBenchmarkBaseline::recorded_at))
}

/// Returns the newest stored benchmark baseline compatible with the supplied current baseline.
pub fn latest_compatible_local_model_benchmark_baseline<S>(
    store: &S,
    current: &LocalModelBenchmarkBaseline,
) -> Result<Option<LocalModelBenchmarkBaseline>, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    Ok(store
        .list_baselines()?
        .into_iter()
        .filter(|baseline| baseline.candidate_model_id() == current.candidate_model_id())
        .filter(|baseline| baseline.candidate_role() == current.candidate_role())
        .filter(|baseline| baseline.response_schema_version() == current.response_schema_version())
        .filter(|baseline| {
            baseline.evaluation_suite_fingerprint() == current.evaluation_suite_fingerprint()
        })
        .filter(|baseline| baseline.schema_fingerprint() == current.schema_fingerprint())
        .filter(|baseline| baseline.grammar_fingerprint() == current.grammar_fingerprint())
        .filter(|baseline| baseline.prompt_fingerprint() == current.prompt_fingerprint())
        .filter(|baseline| baseline.runtime() == current.runtime())
        .max_by_key(LocalModelBenchmarkBaseline::recorded_at))
}

fn prompt_fingerprint_for_suite(suite: &StewardEvaluationSuite) -> String {
    let prompts = suite
        .cases()
        .iter()
        .map(|case| local_model_prompt_for_input(case.input()))
        .collect::<Vec<_>>();
    fingerprint_fields(&prompts)
}

fn fingerprint_text(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn format_temperature(temperature_millis: u16) -> String {
    if temperature_millis == 0 {
        return "0".to_string();
    }

    let whole = temperature_millis / 1000;
    let fractional = temperature_millis % 1000;
    format!("{whole}.{fractional:03}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn fingerprint_fields(fields: &[String]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for field in fields {
        for byte in field.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Storage contract for append-only local model benchmark baselines.
pub trait LocalModelBenchmarkBaselineStore {
    /// Appends a benchmark baseline.
    fn append_baseline(
        &mut self,
        baseline: LocalModelBenchmarkBaseline,
    ) -> Result<(), StewardError>;

    /// Lists all benchmark baselines in insertion order.
    fn list_baselines(&self) -> Result<Vec<LocalModelBenchmarkBaseline>, StewardError>;
}

/// In-memory local model benchmark baseline store for correctness tests.
#[derive(Default)]
pub struct MemoryLocalModelBenchmarkBaselineStore {
    baselines: Vec<LocalModelBenchmarkBaseline>,
}

impl LocalModelBenchmarkBaselineStore for MemoryLocalModelBenchmarkBaselineStore {
    fn append_baseline(
        &mut self,
        baseline: LocalModelBenchmarkBaseline,
    ) -> Result<(), StewardError> {
        self.baselines.push(baseline);
        Ok(())
    }

    fn list_baselines(&self) -> Result<Vec<LocalModelBenchmarkBaseline>, StewardError> {
        Ok(self.baselines.clone())
    }
}

/// JSONL file-backed local model benchmark baseline store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLocalModelBenchmarkBaselineStore {
    path: PathBuf,
}

impl FileLocalModelBenchmarkBaselineStore {
    /// Opens a JSONL benchmark baseline store at the supplied path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StewardError> {
        let path = path.as_ref().to_path_buf();
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreIo)?;

        Ok(Self { path })
    }

    /// Returns the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_baselines(&self) -> Result<Vec<LocalModelBenchmarkBaseline>, StewardError> {
        let file = File::open(&self.path)
            .map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreIo)?;
        let reader = BufReader::new(file);
        let mut baselines = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreIo)?;
            if line.trim().is_empty() {
                continue;
            }
            baselines.push(
                serde_json::from_str(&line)
                    .map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreCorrupt)?,
            );
        }

        Ok(baselines)
    }
}

impl LocalModelBenchmarkBaselineStore for FileLocalModelBenchmarkBaselineStore {
    fn append_baseline(
        &mut self,
        baseline: LocalModelBenchmarkBaseline,
    ) -> Result<(), StewardError> {
        let encoded = serde_json::to_string(&baseline)
            .map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreCorrupt)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreIo)?;

        file.write_all(encoded.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|_error| StewardError::LocalModelBenchmarkBaselineStoreIo)
    }

    fn list_baselines(&self) -> Result<Vec<LocalModelBenchmarkBaseline>, StewardError> {
        self.read_baselines()
    }
}

#[derive(Debug, Deserialize)]
struct ModelResponse {
    proposals: Vec<ModelProposal>,
}

#[derive(Debug, Deserialize)]
struct ModelProposal {
    action: ModelAction,
    rationale: String,
    citations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum ModelAction {
    CreateCellDraft {
        anchors: Vec<SemanticAnchor>,
        payload_text: String,
    },
    LinkRevision {
        source: StateCellId,
        kind: ModelRevisionLinkKind,
        target: StateCellId,
    },
    AdjustConfidence {
        cell_id: StateCellId,
        proposed_confidence: f32,
    },
    LabelAnswerability {
        cell_id: StateCellId,
        questions: Vec<String>,
    },
    MarkFrontier {
        cell_id: StateCellId,
    },
    RequestVerification {
        cell_id: Option<StateCellId>,
        request: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ModelRevisionLinkKind {
    Predecessor,
    Supersedes,
    ConflictsWith,
    DerivesFrom,
}

impl From<ModelRevisionLinkKind> for RevisionLinkKind {
    fn from(value: ModelRevisionLinkKind) -> Self {
        match value {
            ModelRevisionLinkKind::Predecessor => Self::Predecessor,
            ModelRevisionLinkKind::Supersedes => Self::Supersedes,
            ModelRevisionLinkKind::ConflictsWith => Self::ConflictsWith,
            ModelRevisionLinkKind::DerivesFrom => Self::DerivesFrom,
        }
    }
}

fn decode_response(
    response: &str,
    steward: StewardIdentity,
    created_at: DateTime<Utc>,
) -> Result<Vec<StewardProposal>, StewardError> {
    let response: ModelResponse =
        serde_json::from_str(response).map_err(|_error| StewardError::InvalidModelResponse)?;

    response
        .proposals
        .into_iter()
        .map(|proposal| {
            StewardProposal::new(
                ProposalId::new(),
                steward.clone(),
                proposal.action.into_steward_action(),
                proposal.rationale,
                proposal.citations,
                created_at,
            )
        })
        .collect()
}

impl ModelAction {
    fn into_steward_action(self) -> StewardAction {
        match self {
            Self::CreateCellDraft {
                anchors,
                payload_text,
            } => StewardAction::CreateCellDraft {
                anchors,
                payload_text,
            },
            Self::LinkRevision {
                source,
                kind,
                target,
            } => StewardAction::LinkRevision {
                source,
                kind: kind.into(),
                target,
            },
            Self::AdjustConfidence {
                cell_id,
                proposed_confidence,
            } => StewardAction::AdjustConfidence {
                cell_id,
                proposed_confidence,
            },
            Self::LabelAnswerability { cell_id, questions } => {
                StewardAction::LabelAnswerability { cell_id, questions }
            }
            Self::MarkFrontier { cell_id } => StewardAction::MarkFrontier { cell_id },
            Self::RequestVerification { cell_id, request } => {
                StewardAction::RequestVerification { cell_id, request }
            }
        }
    }
}

fn build_prompt(task: &str, evidence: &[LocalModelEvidence]) -> String {
    let mut prompt = String::from(
        "You are the ContinuityDB database Steward. Emit only JSON with a proposals array. \
         Do not mutate truth directly. Preserve citation locators exactly.\n\n",
    );
    prompt.push_str("Task:\n");
    prompt.push_str(task);
    prompt.push_str("\n\nEvidence:\n");

    for item in evidence {
        prompt.push_str("- ");
        prompt.push_str(&item.locator);
        prompt.push_str(": ");
        prompt.push_str(&item.text);
        prompt.push('\n');
    }

    prompt
}

fn evaluate_case<B>(
    case: &StewardEvaluationCase,
    steward: &LocalModelSteward<B>,
) -> StewardEvaluationCaseReport
where
    B: LocalModelBackend,
{
    evaluate_case_with_response(case, steward).0
}

fn evaluate_case_with_response<B>(
    case: &StewardEvaluationCase,
    steward: &LocalModelSteward<B>,
) -> (StewardEvaluationCaseReport, StewardEvaluationCaseResponse)
where
    B: LocalModelBackend,
{
    let response = match steward.raw_response(case.input.clone()) {
        Ok(response) => response,
        Err(_error) => {
            return (
                StewardEvaluationCaseReport {
                    name: case.name.clone(),
                    failures: vec![StewardEvaluationFailure::ModelError],
                },
                StewardEvaluationCaseResponse::missing(case.name.clone()),
            );
        }
    };
    let proposals = match steward.decode_raw_response(&case.input, &response) {
        Ok(proposals) => proposals,
        Err(_error) => {
            return (
                StewardEvaluationCaseReport {
                    name: case.name.clone(),
                    failures: vec![StewardEvaluationFailure::ModelError],
                },
                StewardEvaluationCaseResponse::captured(case.name.clone(), response),
            );
        }
    };
    (
        evaluate_case_proposals(case, &proposals),
        StewardEvaluationCaseResponse::captured(case.name.clone(), response),
    )
}

fn evaluate_case_proposals(
    case: &StewardEvaluationCase,
    proposals: &[StewardProposal],
) -> StewardEvaluationCaseReport {
    let mut failures = Vec::new();

    for expected in &case.expected_actions {
        if proposals
            .iter()
            .all(|proposal| proposal.action() != expected)
        {
            failures.push(StewardEvaluationFailure::MissingExpectedAction {
                action: expected.clone(),
            });
        }
    }

    for required in &case.required_citations {
        if proposals
            .iter()
            .flat_map(StewardProposal::citations)
            .all(|citation| citation != required)
        {
            failures.push(StewardEvaluationFailure::MissingCitation {
                locator: required.clone(),
            });
        }
    }

    let policy = ProposalPolicy::strict();
    for proposal in proposals {
        let decision = policy.evaluate(proposal, proposal.created_at());
        if decision.outcome() == ProposalOutcome::Rejected {
            failures.push(StewardEvaluationFailure::PolicyRejected {
                reasons: decision.reasons().to_vec(),
            });
        }
    }

    let rationales: Vec<String> = proposals
        .iter()
        .map(|proposal| proposal.rationale().to_ascii_lowercase())
        .collect();
    for term in &case.required_rationale_terms {
        let normalized = term.to_ascii_lowercase();
        if rationales
            .iter()
            .all(|rationale| !rationale.contains(&normalized))
        {
            failures.push(StewardEvaluationFailure::MissingRationaleTerm { term: term.clone() });
        }
    }
    for term in &case.forbidden_rationale_terms {
        let normalized = term.to_ascii_lowercase();
        if rationales
            .iter()
            .any(|rationale| rationale.contains(&normalized))
        {
            failures
                .push(StewardEvaluationFailure::UnsupportedRationaleTerm { term: term.clone() });
        }
    }

    StewardEvaluationCaseReport {
        name: case.name.clone(),
        failures,
    }
}
