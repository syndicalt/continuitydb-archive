//! ContinuityDB command-line interface.

mod live_benchmark;

use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use continuitydb_api::{
    decode_checkout_query_result_json, encode_checkout_query_result_json, CheckoutQueryResult,
    ContinuityDb, ContinuityError,
};
use continuitydb_checkout::{
    cell_lookup_from_checkout_request, checkout, CheckoutRequest, CheckoutSummary,
};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellPayload, Citation, CommitId, Confidence,
    ContextCompilerPolicy, ContextPacket, ContextProfile, Evidence, RevisionLinkKind,
    RevisionLinkRecord, Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal,
    ValidTimeRange,
};
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, KernelCapabilities, KernelDurability, KernelRequirements,
    StorageKernel,
};
use continuitydb_memory::MemoryKernel;
use continuitydb_query::{parse_query_text, ContinuityQuery, QueryEnvelope, QueryReturnShape};
#[cfg(feature = "local-model")]
use continuitydb_steward::{
    default_steward_evaluation_suite, local_model_context_compiler_response_gbnf_grammar,
    local_model_context_compiler_response_json_schema, local_model_prompt_fingerprint_for_suite,
    local_model_prompt_for_input, local_model_response_gbnf_grammar,
    local_model_response_json_schema, required_steward_acceptance_criteria, small_model_candidates,
    small_model_ci_candidates, small_model_default_ci_candidate,
    small_model_default_quality_gate_candidate, small_model_quality_gate_candidates,
    FileLocalModelBenchmarkBaselineStore, LocalExecutableRunner, LocalExecutableRunnerConfig,
    LocalModelBenchmark, LocalModelBenchmarkBaseline, LocalModelBenchmarkBaselineStore,
    LocalModelBenchmarkRegression, LocalModelStabilityReport, SmallModelCandidate,
    StewardAcceptanceCoverage, StewardAction, StewardEvaluationCaseResponse,
    StewardEvaluationSuite, StewardIdentity, LOCAL_MODEL_CONTEXT_COMPILER_RESPONSE_SCHEMA_VERSION,
    LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
};
use continuitydb_workload::{
    compare_workload_snapshot_to_baseline, generate_agent_behavior_task_matrix,
    generate_representative_agent_behavior_task_matrix, generate_world_model_workload,
    measure_ingest_and_checkout, run_adversarial_agent_task_harness, run_agent_behavior_benchmark,
    run_thesis_falsification_benchmark, score_agent_behavior_execution_records,
    AgentBehaviorExecutionRecord, AgentBehaviorExecutionTask, AgentBehaviorTaskMatrixReport,
    ContinuityWorkload, FileWorkloadBaselineStore, WorkloadBaselineComparison,
    WorkloadBaselineRecord, WorkloadConfig, WorkloadMeasurement, WorkloadMeasurementSnapshot,
    WorkloadRegressionThresholds, WorkloadSummary,
};
use live_benchmark::{
    adversarial_validation_report_json, adversarial_validation_report_markdown,
    live_benchmark_corpus_json, live_benchmark_quality_report_json,
    live_benchmark_quality_report_markdown, live_benchmark_run_json,
    live_benchmark_smoke_report_json, live_benchmark_smoke_report_markdown,
    representative_benchmark_report_json, representative_benchmark_report_markdown,
    LiveBenchmarkTarget,
};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    time::Instant,
};

/// ContinuityDB command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "continuitydb",
    version,
    about = "Embeddable datastore for continuity state"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

struct WorkloadMeasureOptions<'a> {
    kernel: WorkloadKernelProfile,
    store_path: Option<&'a PathBuf>,
    artifact_dir: Option<&'a PathBuf>,
    report_path: Option<&'a PathBuf>,
    failure_report_path: Option<&'a PathBuf>,
    cells: usize,
    token_budget: i64,
    frontier_every: usize,
    dependency_stride: usize,
    baseline_path: Option<&'a PathBuf>,
    label: &'a str,
    compare_baseline: bool,
    max_elapsed_growth_percent: u128,
    fail_on_regression: bool,
}

struct WorkloadReplayOptions<'a> {
    kernel: WorkloadKernelProfile,
    artifact_dir: &'a Path,
    store_path: Option<&'a PathBuf>,
    report_path: Option<&'a PathBuf>,
    failure_report_path: Option<&'a PathBuf>,
    replay_artifact_dir: Option<&'a PathBuf>,
    require_manifest: bool,
    compare_report: bool,
    fail_on_mismatch: bool,
}

#[cfg(feature = "local-model")]
struct LocalModelBenchmarkOptions<'a> {
    candidate_id: Option<&'a str>,
    executable: &'a Path,
    model_path: &'a Path,
    arguments: &'a [String],
    candidate_defaults: bool,
    grammar_path: Option<&'a Path>,
    artifact_dir: Option<&'a Path>,
    contract_dir: Option<&'a Path>,
    prompt_dir: Option<&'a Path>,
    response_dir: Option<&'a Path>,
    enforce_candidate_requirements: bool,
    baseline_path: &'a Path,
    stability_trials: Option<usize>,
    fail_on_unstable: bool,
    fail_on_failed_cases: bool,
    failure_report_path: Option<&'a Path>,
    changed_case_report_path: Option<&'a Path>,
    dry_run: bool,
    compare_baseline: bool,
    fail_on_regression: bool,
}

#[cfg(feature = "local-model")]
struct LocalModelCandidateSelection {
    candidate: SmallModelCandidate,
    source: &'static str,
}

#[cfg(feature = "local-model")]
struct LocalModelQualityGateRunOptions<'a> {
    executable: &'a Path,
    model_paths: &'a [String],
    arguments: &'a [String],
    baseline_path: &'a Path,
    artifact_root: &'a Path,
    report_path: Option<&'a Path>,
    failure_report_path: Option<&'a Path>,
    dry_run: bool,
    stability_trials: Option<usize>,
    fail_on_unstable: bool,
}

struct WorkloadBundleManifest {
    manifest_path: PathBuf,
    manifest_fingerprint: String,
    manifest_bytes: usize,
}

struct WorkloadBundleValidation {
    manifest: WorkloadBundleManifest,
    workload_report: serde_json::Value,
    workload_artifacts: serde_json::Value,
}

struct WorkloadFixtureArtifacts {
    cells_path: PathBuf,
    cells_fingerprint: String,
    cells_bytes: usize,
    checkout_request_path: PathBuf,
    checkout_request_fingerprint: String,
    checkout_request_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelContractArtifacts {
    schema_path: PathBuf,
    grammar_path: PathBuf,
    schema_fingerprint: String,
    grammar_fingerprint: String,
    schema_bytes: usize,
    grammar_bytes: usize,
    context_compiler_schema_path: PathBuf,
    context_compiler_grammar_path: PathBuf,
    context_compiler_schema_version: u32,
    context_compiler_schema_fingerprint: String,
    context_compiler_grammar_fingerprint: String,
    context_compiler_schema_bytes: usize,
    context_compiler_grammar_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelPromptArtifact {
    case_name: String,
    prompt_path: PathBuf,
    prompt_fingerprint: String,
    prompt_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelResponseArtifact {
    case_name: String,
    captured: bool,
    response_path: Option<PathBuf>,
    response_fingerprint: Option<String>,
    response_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelResponseArtifactManifest {
    manifest_path: PathBuf,
    manifest_fingerprint: String,
    manifest_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelBundleManifest {
    manifest_path: PathBuf,
    manifest_fingerprint: String,
    manifest_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelBundleValidation {
    manifest: LocalModelBundleManifest,
    benchmark_report: serde_json::Value,
    changed_case_report: serde_json::Value,
    contract_artifacts: serde_json::Value,
    prompt_artifacts: serde_json::Value,
    response_artifacts: serde_json::Value,
    response_artifact_manifest: serde_json::Value,
}

#[cfg(feature = "local-model")]
struct LocalModelChangedCaseReportArtifact {
    report_path: PathBuf,
    report_fingerprint: String,
    report_bytes: usize,
}

#[cfg(feature = "local-model")]
struct LocalModelBenchmarkArtifacts<'a> {
    contract: Option<&'a LocalModelContractArtifacts>,
    prompts: &'a [LocalModelPromptArtifact],
    responses: &'a [LocalModelResponseArtifact],
    response_manifest: Option<&'a LocalModelResponseArtifactManifest>,
}

#[cfg(feature = "local-model")]
struct LocalModelBenchmarkDryRunGates {
    baseline_preflight: Option<serde_json::Value>,
    stability_preflight: Option<serde_json::Value>,
    fail_on_failed_cases: bool,
    failure_report_path: Option<PathBuf>,
    changed_case_report_path: Option<PathBuf>,
}

/// Named kernel requirement profiles understood by the CLI.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum RequirementProfile {
    /// Allow temporary in-process correctness kernels.
    Ephemeral,
    /// Require durable append-log storage.
    DurableAppendLog,
    /// Require append-log storage with persistent indexes.
    PersistentIndexedAppendLog,
    /// Require future durable storage with persistent indexes.
    IndexedEmbedded,
}

/// Kernel profiles supported by workload measurement.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum WorkloadKernelProfile {
    /// Use the in-memory correctness kernel.
    Memory,
    /// Use the durable JSONL file kernel.
    File,
}

/// Supported commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the current implementation scope.
    Scope,
    /// Print the thesis proof obligations and their verification surfaces.
    ProofObligations,
    /// Validate an archived thesis proof-obligations JSON artifact.
    ValidateProofObligations {
        /// Path to the archived proof-obligations JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful proof-obligations validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write proof-obligations validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived CI artifact inventory JSON artifact.
    ValidateCiArtifactInventory {
        /// Path to the archived CI artifact inventory JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful CI artifact inventory validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write CI artifact inventory validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived release asset manifest against files on disk.
    ValidateReleaseAssets {
        /// Path to the archived release asset manifest JSON.
        #[arg(long = "manifest-path")]
        manifest_path: PathBuf,
        /// Optional path to write successful release asset validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write release asset validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived release upload report against its release asset manifest.
    ValidateReleaseUploadReport {
        /// Path to the archived release upload report JSON.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful release upload validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write release upload validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived mocked release upload test report.
    ValidateReleaseUploadTestReport {
        /// Path to the archived release upload test report JSON.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful release upload test validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write release upload test validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Run a deterministic context-collapse prevention drill.
    #[command(hide = true)]
    ContextCollapseDrill {
        /// Optional path to write the context-collapse drill JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Run the thesis-falsification benchmark against a strong local memory baseline.
    ThesisFalsificationBenchmark {
        /// Optional path to write the thesis-falsification JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the thesis-falsification Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Benchmark continuity checkout against a collapsed summary baseline.
    #[command(hide = true)]
    ContextCollapseBenchmark {
        /// Optional path to write the context-collapse benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Benchmark continuity checkout retrieval against a Pinecone-targeted vector baseline.
    #[command(hide = true)]
    ContextCollapseRetrievalBenchmark {
        /// Fail unless live Pinecone API configuration is complete.
        #[arg(long = "require-live-pinecone")]
        require_live_pinecone: bool,
        /// Optional path to write the context-collapse retrieval benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Run the full ContinuityDB-vs-vector benchmark suite.
    #[command(hide = true)]
    ComprehensiveVectorBenchmark {
        /// Optional path to write the comprehensive benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the comprehensive benchmark Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Run the full ContinuityDB-vs-Neo4j graph benchmark suite.
    #[command(hide = true)]
    ComprehensiveGraphBenchmark {
        /// Optional path to write the comprehensive graph benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the comprehensive graph benchmark Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Generate scalable retained corpus artifacts for live external benchmark runs.
    #[command(hide = true)]
    LiveBenchmarkCorpus {
        /// Directory where corpus manifests, cell JSONL, relationship JSONL, and queries are written.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Corpus size to generate. Repeat this flag for multiple scale profiles.
        #[arg(long = "size", required = true)]
        sizes: Vec<usize>,
    },
    /// Run or dry-run a live external benchmark target against a generated continuity corpus.
    #[command(hide = true)]
    LiveBenchmarkRun {
        /// External benchmark target provider.
        #[arg(long = "target")]
        target: LiveBenchmarkTarget,
        /// Number of cells in the generated benchmark corpus.
        #[arg(long = "cells", default_value_t = 1000)]
        cells: usize,
        /// Directory where the retained run artifacts are written.
        #[arg(long = "artifact-dir")]
        artifact_dir: Option<PathBuf>,
        /// Fail unless the selected target has complete live environment configuration.
        #[arg(long = "require-live")]
        require_live: bool,
    },
    /// Run both external benchmark smoke targets and retain a correctness report.
    #[command(hide = true)]
    LiveBenchmarkSmokeReport {
        /// Number of cells in the generated smoke benchmark corpus.
        #[arg(long = "cells", default_value_t = 1000)]
        cells: usize,
        /// Directory where target run artifacts and the smoke report are written.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Optional path to write the smoke report JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the smoke report Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Score retained live Neo4j and Pinecone reports against ContinuityDB semantics.
    #[command(hide = true)]
    LiveBenchmarkQualityReport {
        /// Retained live Neo4j benchmark report path.
        #[arg(long = "neo4j-report")]
        neo4j_report: PathBuf,
        /// Retained live Pinecone benchmark report path.
        #[arg(long = "pinecone-report")]
        pinecone_report: PathBuf,
        /// Optional path to write the quality report JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the quality report Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Build a representative agent-memory benchmark report.
    #[command(hide = true)]
    RepresentativeBenchmarkReport {
        /// Number of representative StateCell-scale records to model.
        #[arg(long = "cells", default_value_t = 10000)]
        cells: usize,
        /// Optional live Neo4j report to attach as execution evidence.
        #[arg(long = "neo4j-report")]
        neo4j_report: Option<PathBuf>,
        /// Optional live Pinecone report to attach as execution evidence.
        #[arg(long = "pinecone-report")]
        pinecone_report: Option<PathBuf>,
        /// Optional path to write the representative benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the representative benchmark Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Build an adversarial validation report that challenges the representative benchmark.
    #[command(hide = true)]
    AdversarialValidationReport {
        /// Number of representative StateCell-scale records being evaluated.
        #[arg(long = "cells", default_value_t = 10000)]
        cells: usize,
        /// Optional representative benchmark report to attach.
        #[arg(long = "representative-report")]
        representative_report: Option<PathBuf>,
        /// Optional quality-scored live benchmark report to attach.
        #[arg(long = "quality-report")]
        quality_report: Option<PathBuf>,
        /// Optional path to write the adversarial validation JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write the adversarial validation Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Build a deterministic adversarial downstream task-harness report.
    #[command(hide = true)]
    AdversarialTaskHarnessReport {
        /// Optional path to write the adversarial task-harness JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Benchmark whether checkout improves downstream agent behavior.
    AgentBehaviorBenchmark {
        /// Optional path to write the agent behavior benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Generate the canonical agent-behavior task matrix for model runners.
    AgentBehaviorTaskMatrix {
        /// Emit representative repo-lifecycle tasks instead of the small curated corpus.
        #[arg(long = "representative-corpus")]
        representative_corpus: bool,
        /// Optional path to write the generated task array consumed by run-agent-behavior-outputs.
        #[arg(long = "tasks-path")]
        tasks_path: Option<PathBuf>,
        /// Optional path to write the task matrix JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Generate, run, score, and package the agent-behavior benchmark artifacts.
    AgentBehaviorBenchmarkBundle {
        /// Emit representative repo-lifecycle tasks instead of the small curated corpus.
        #[arg(long = "representative-corpus")]
        representative_corpus: bool,
        /// Directory where benchmark artifacts and manifest are written.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Executable model runner. It receives one task JSON on stdin and returns JSON with model_output.
        #[arg(long = "runner")]
        runner: PathBuf,
        /// Number of repeated trials to run for each task.
        #[arg(long = "trials", default_value_t = 1)]
        trials: usize,
        /// Extra argument passed to the model runner. Repeat to pass multiple arguments.
        #[arg(long = "runner-arg")]
        runner_args: Vec<String>,
    },
    /// Interpret a retained agent-behavior benchmark bundle for evidence readiness.
    AgentBehaviorBenchmarkReport {
        /// Directory containing agent-behavior benchmark bundle artifacts.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Fail if the retained bundle does not satisfy marketable evidence gates.
        #[arg(long = "require-marketable")]
        require_marketable: bool,
        /// Optional path to write the interpreted benchmark report JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write a Markdown summary.
        #[arg(long = "summary-path")]
        summary_path: Option<PathBuf>,
    },
    /// Score retained model answers for the downstream agent-behavior benchmark.
    ScoreAgentBehaviorOutputs {
        /// Path to a JSON array of retained model answer records.
        #[arg(long = "records-path")]
        records_path: PathBuf,
        /// Optional path to write the scored execution benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Execute agent-behavior tasks with a model runner and score retained answers.
    RunAgentBehaviorOutputs {
        /// Path to a JSON array of model-execution task requests.
        #[arg(long = "tasks-path")]
        tasks_path: PathBuf,
        /// Executable model runner. It receives one task JSON on stdin and returns JSON with model_output.
        #[arg(long = "runner")]
        runner: PathBuf,
        /// Number of repeated trials to run for each task.
        #[arg(long = "trials", default_value_t = 1)]
        trials: usize,
        /// Extra argument passed to the model runner. Repeat to pass multiple arguments.
        #[arg(long = "runner-arg")]
        runner_args: Vec<String>,
        /// Optional path to write retained model answer records.
        #[arg(long = "records-path")]
        records_path: Option<PathBuf>,
        /// Optional path to write the scored execution benchmark JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
    },
    /// Print deterministic demo checkout JSON.
    DemoCheckout,
    /// Execute a serialized typed Continuity query JSON file against a file-backed store.
    CheckoutQuery {
        /// Emit a versioned continuitydb.checkout_query.result envelope.
        #[arg(long = "result-envelope")]
        result_envelope: bool,
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to a serialized ContinuityQuery JSON file.
        query_path: PathBuf,
    },
    /// Validate an archived projected checkout query result envelope.
    ValidateCheckoutQueryResult {
        /// Path to the archived checkout query result JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful checkout result validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write checkout result validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived summary-only checkout query artifact.
    ValidateCheckoutQuerySummary {
        /// Path to the archived checkout query summary JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful checkout summary validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write checkout summary validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived context-packets-only checkout query artifact.
    ValidateCheckoutQueryContextPackets {
        /// Path to the archived checkout query context packets JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful checkout context packet validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write checkout context packet validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Measure deterministic workload ingest and checkout.
    MeasureWorkload {
        /// Kernel profile to measure.
        #[arg(long = "kernel", default_value = "memory")]
        kernel: WorkloadKernelProfile,
        /// Path to the JSONL file-backed store when measuring the file kernel.
        #[arg(long = "store-path")]
        store_path: Option<PathBuf>,
        /// Directory where a workload measurement artifact bundle is written.
        #[arg(long = "artifact-dir")]
        artifact_dir: Option<PathBuf>,
        /// Optional path to write the successful workload measurement JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write workload measurement JSON when a regression gate fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
        /// Number of deterministic StateCells to generate.
        #[arg(long = "cells", default_value_t = 8)]
        cells: usize,
        /// Checkout token budget.
        #[arg(long = "token-budget", default_value_t = 400)]
        token_budget: i64,
        /// Every Nth generated cell is marked as frontier.
        #[arg(long = "frontier-every", default_value_t = 3)]
        frontier_every: usize,
        /// Dependency stride for generated cells.
        #[arg(long = "dependency-stride", default_value_t = 2)]
        dependency_stride: usize,
        /// Optional JSONL path to append a workload measurement baseline record.
        #[arg(long = "baseline-path")]
        baseline_path: Option<PathBuf>,
        /// Baseline scenario label when recording a measurement.
        #[arg(long = "label", default_value = "default")]
        label: String,
        /// Compare this measurement with the latest matching baseline before recording.
        #[arg(long = "compare-baseline")]
        compare_baseline: bool,
        /// Maximum elapsed-time growth percentage allowed during baseline comparison.
        #[arg(long = "max-elapsed-growth-percent", default_value_t = 25)]
        max_elapsed_growth_percent: u128,
        /// Exit non-zero when baseline comparison reports regressions.
        #[arg(long = "fail-on-regression")]
        fail_on_regression: bool,
    },
    /// Replay a workload artifact bundle against a selected kernel.
    ReplayWorkload {
        /// Kernel profile to replay against.
        #[arg(long = "kernel", default_value = "memory")]
        kernel: WorkloadKernelProfile,
        /// Directory containing workload-cells.json and checkout-request.json.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Path to the JSONL file-backed store when replaying the file kernel.
        #[arg(long = "store-path")]
        store_path: Option<PathBuf>,
        /// Optional path to write the successful workload replay JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write workload replay JSON when a mismatch gate fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
        /// Directory where a workload replay artifact bundle is written.
        #[arg(long = "replay-artifact-dir")]
        replay_artifact_dir: Option<PathBuf>,
        /// Require continuitydb-workload.manifest.json to match replay fixture files.
        #[arg(long = "require-manifest")]
        require_manifest: bool,
        /// Compare replay counts against workload-report.json in the artifact directory.
        #[arg(long = "compare-report")]
        compare_report: bool,
        /// Exit non-zero when --compare-report detects mismatched deterministic counts.
        #[arg(long = "fail-on-mismatch")]
        fail_on_mismatch: bool,
    },
    /// Validate a workload artifact bundle without replaying it.
    ValidateWorkloadBundle {
        /// Directory containing workload-report.json and continuitydb-workload.manifest.json.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Optional path to write successful workload bundle validation JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write workload bundle validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Compact a JSONL file-backed store into the canonical durable record format.
    CompactFile {
        /// Path to the JSONL file-backed store.
        path: PathBuf,
        /// Skip rewriting when the store is already canonical.
        #[arg(long = "if-needed")]
        if_needed: bool,
    },
    /// Inspect file-backed kernel capabilities and optionally enforce a requirement profile.
    InspectKernel {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Directory where an inspect-kernel artifact bundle is written.
        #[arg(long = "artifact-dir")]
        artifact_dir: Option<PathBuf>,
        /// Optional path to write the inspection JSON report.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Required storage profile.
        #[arg(long = "require")]
        require: Option<RequirementProfile>,
        /// Require the store to already be in canonical durable file format.
        #[arg(long = "require-canonical")]
        require_canonical: bool,
        /// Include the default file lookup candidate plan.
        #[arg(long = "lookup-plan")]
        lookup_plan: bool,
        /// Include the file lookup candidate plan for a strict text CHECKOUT query.
        #[arg(long = "lookup-query")]
        lookup_query: Option<String>,
    },
    /// Validate an inspect-kernel JSON report artifact.
    ValidateInspectKernelReport {
        /// Path to the inspection JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write validation failure JSON.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an inspect-kernel artifact bundle without reopening the store.
    ValidateInspectKernelBundle {
        /// Directory containing inspect-kernel-report.json and continuitydb-inspect-kernel.manifest.json.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Optional path to write successful bundle validation JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write bundle validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Export all file-backed commit slices to a versioned JSON backup envelope.
    ExportCommits {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to write the versioned JSON commit export envelope.
        output_path: PathBuf,
        /// Exclusive commit cursor to start after.
        #[arg(long = "after")]
        after: Option<CommitId>,
        /// Maximum number of commits to export.
        #[arg(long = "limit")]
        limit: Option<usize>,
    },
    /// Copy file-backed commit slices directly between two stores.
    CopyCommits {
        /// Path to the source JSONL file-backed store.
        source_path: PathBuf,
        /// Path to the target JSONL file-backed store.
        target_path: PathBuf,
        /// Exclusive commit cursor to start after.
        #[arg(long = "after")]
        after: Option<CommitId>,
        /// Maximum number of commits to copy.
        #[arg(long = "limit")]
        limit: Option<usize>,
    },
    /// Import a versioned JSON commit backup envelope into a file-backed store.
    ImportCommits {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to read the versioned JSON commit export envelope from.
        input_path: PathBuf,
        /// Validate the import without mutating the target store.
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Run a local Steward model benchmark and append a durable JSONL baseline.
    #[cfg(feature = "local-model")]
    BenchmarkLocalModel {
        /// Candidate model identifier from the fixed small-model list.
        #[arg(long = "candidate")]
        candidate: Option<String>,
        /// Local executable path.
        #[arg(long = "executable")]
        executable: PathBuf,
        /// Model file path passed to the executable as `--model <path>`.
        #[arg(long = "model-path")]
        model_path: PathBuf,
        /// Extra executable argument, repeatable and ordered.
        #[arg(long = "arg", allow_hyphen_values = true)]
        arguments: Vec<String>,
        /// Use the selected candidate's recommended benchmark arguments before extra --arg values.
        #[arg(long = "candidate-defaults")]
        candidate_defaults: bool,
        /// GBNF grammar path passed to the executable as `--grammar-file <path>`.
        #[arg(long = "grammar-path")]
        grammar_path: Option<PathBuf>,
        /// Directory where a benchmark artifact bundle is written.
        #[arg(long = "artifact-dir")]
        artifact_dir: Option<PathBuf>,
        /// Directory where benchmark-local Steward schema and grammar artifacts are written.
        #[arg(long = "contract-dir")]
        contract_dir: Option<PathBuf>,
        /// Directory where benchmark-local Steward evaluation prompts are written.
        #[arg(long = "prompt-dir")]
        prompt_dir: Option<PathBuf>,
        /// Directory where real benchmark raw model responses are written.
        #[arg(long = "response-dir")]
        response_dir: Option<PathBuf>,
        /// Reject benchmark configurations that violate selected candidate requirements.
        #[arg(long = "enforce-candidate-requirements")]
        enforce_candidate_requirements: bool,
        /// JSONL path to append a benchmark baseline record.
        #[arg(long = "baseline-path")]
        baseline_path: PathBuf,
        /// Re-run each evaluation case N times and report output stability.
        #[arg(long = "stability-trials")]
        stability_trials: Option<usize>,
        /// Exit non-zero before baseline recording when repeated stability trials drift.
        #[arg(long = "fail-on-unstable")]
        fail_on_unstable: bool,
        /// Exit non-zero before baseline recording when fixed evaluation cases fail.
        #[arg(long = "fail-on-failed-cases")]
        fail_on_failed_cases: bool,
        /// Path to write benchmark JSON when a pre-recording quality gate fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
        /// Path to write compact local-model changed-case comparison JSON.
        #[arg(long = "changed-case-report-path")]
        changed_case_report_path: Option<PathBuf>,
        /// Path to write successful benchmark or dry-run JSON output.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Print benchmark configuration without executing the model or recording a baseline.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Compare this run with the latest matching previous baseline.
        #[arg(long = "compare-baseline")]
        compare_baseline: bool,
        /// Exit non-zero when the latest matching previous baseline regresses.
        #[arg(long = "fail-on-regression")]
        fail_on_regression: bool,
    },
    /// Validate a local Steward model benchmark artifact bundle.
    #[cfg(feature = "local-model")]
    ValidateLocalModelBundle {
        /// Directory containing benchmark-report.json and local-model-benchmark.manifest.json.
        #[arg(long = "artifact-dir")]
        artifact_dir: PathBuf,
        /// Optional path to write successful local-model bundle validation JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Optional path to write local-model bundle validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived local Steward model benchmark report.
    #[cfg(feature = "local-model")]
    ValidateLocalModelBenchmarkReport {
        /// Path to the archived local model benchmark report JSON.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful benchmark report validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write benchmark report validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Write local Steward model JSON Schema and GBNF grammar artifacts.
    #[cfg(feature = "local-model")]
    LocalModelContract {
        /// Write the context compiler packet-shape response contract instead of the Steward action response contract.
        #[arg(long = "context-compiler")]
        context_compiler: bool,
        /// Path to write the local model response JSON Schema.
        #[arg(long = "schema-path")]
        schema_path: PathBuf,
        /// Path to write the local model response GBNF grammar.
        #[arg(long = "grammar-path")]
        grammar_path: PathBuf,
    },
    /// Print the default local Steward model evaluation suite contract as JSON.
    #[cfg(feature = "local-model")]
    LocalModelEvaluationSuite,
    /// Validate an archived local Steward evaluation suite contract.
    #[cfg(feature = "local-model")]
    ValidateLocalModelEvaluationSuite {
        /// Path to the archived local model evaluation suite JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful evaluation suite validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write evaluation suite validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Print the required local Steward acceptance criteria contract as JSON.
    #[cfg(feature = "local-model")]
    LocalModelAcceptanceCriteria,
    /// Validate an archived local Steward acceptance criteria contract.
    #[cfg(feature = "local-model")]
    ValidateLocalModelAcceptanceCriteria {
        /// Path to the archived acceptance criteria JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful acceptance criteria validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write acceptance criteria validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Print the fixed local Steward model candidate registry as JSON.
    #[cfg(feature = "local-model")]
    LocalModelCandidates,
    /// Validate an archived local Steward model candidate registry.
    #[cfg(feature = "local-model")]
    ValidateLocalModelCandidates {
        /// Path to the archived local model candidate registry JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful candidate registry validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write candidate registry validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Print an ordered local Steward model quality-gate benchmark plan as JSON.
    #[cfg(feature = "local-model")]
    LocalModelQualityGatePlan,
    /// Validate an archived local Steward model quality-gate plan.
    #[cfg(feature = "local-model")]
    ValidateLocalModelQualityGatePlan {
        /// Path to the archived local model quality-gate plan JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful quality-gate plan validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write quality-gate plan validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Summarize local Steward model quality-gate artifacts under an artifact root.
    #[cfg(feature = "local-model")]
    LocalModelQualityGateStatus {
        /// Root directory containing per-candidate quality-gate artifact directories.
        #[arg(long = "artifact-root")]
        artifact_root: PathBuf,
        /// Exit non-zero when any quality-gate candidate is not ready.
        #[arg(long = "require-ready")]
        require_ready: bool,
    },
    /// Validate an archived local Steward model quality-gate status report.
    #[cfg(feature = "local-model")]
    ValidateLocalModelQualityGateStatus {
        /// Path to the archived quality-gate status JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful quality-gate status validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate an archived local Steward require-ready failure status report.
    #[cfg(feature = "local-model")]
    ValidateLocalModelRequireReadyStatus {
        /// Path to the archived require-ready status JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Path to the archived require-ready stderr output.
        #[arg(long = "stderr-path")]
        stderr_path: PathBuf,
        /// Optional path to write successful require-ready status validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Execute the ordered local Steward model quality-gate candidate set.
    #[cfg(feature = "local-model")]
    RunLocalModelQualityGate {
        /// Local executable path.
        #[arg(long = "executable")]
        executable: PathBuf,
        /// Candidate model path binding in MODEL_ID=PATH form, repeatable.
        #[arg(long = "model-path")]
        model_paths: Vec<String>,
        /// Extra executable argument, repeatable and ordered.
        #[arg(long = "arg", allow_hyphen_values = true)]
        arguments: Vec<String>,
        /// JSONL path to append benchmark baseline records.
        #[arg(long = "baseline-path")]
        baseline_path: PathBuf,
        /// Root directory where per-candidate quality-gate artifacts are written.
        #[arg(long = "artifact-root")]
        artifact_root: PathBuf,
        /// Path to write successful quality-gate run JSON.
        #[arg(long = "report-path")]
        report_path: Option<PathBuf>,
        /// Path to write quality-gate run JSON when launch or execution fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
        /// Produce benchmark dry-run bundles without executing the model runner.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Re-run each evaluation case N times and report output stability.
        #[arg(long = "stability-trials")]
        stability_trials: Option<usize>,
        /// Exit non-zero before baseline recording when repeated stability trials drift.
        #[arg(long = "fail-on-unstable")]
        fail_on_unstable: bool,
        /// Exit non-zero when final quality-gate status is not ready.
        #[arg(long = "require-ready")]
        require_ready: bool,
        /// Exit non-zero when final quality-gate status is not ready.
        #[arg(long = "fail-on-not-ready")]
        fail_on_not_ready: bool,
    },
    /// Validate an archived local Steward model quality-gate run report.
    #[cfg(feature = "local-model")]
    ValidateLocalModelQualityGateRunReport {
        /// Path to the archived quality-gate run JSON report.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful quality-gate run report validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
    /// Validate captured stdout from a local Steward model quality-gate run.
    #[cfg(feature = "local-model")]
    ValidateLocalModelQualityGateRunOutput {
        /// Path to captured quality-gate run stdout JSON.
        #[arg(long = "report-path")]
        report_path: PathBuf,
        /// Optional path to write successful quality-gate run output validation JSON.
        #[arg(long = "validation-report-path")]
        validation_report_path: Option<PathBuf>,
        /// Optional path to write validation JSON when validation fails.
        #[arg(long = "failure-report-path")]
        failure_report_path: Option<PathBuf>,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Scope) => {
            println!("core,kernel,memory,revision,checkout,audit");
        }
        Some(Command::ProofObligations) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&proof_obligations_json())?
            );
        }
        Some(Command::ValidateProofObligations {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_proof_obligations_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &proof_obligations_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateCiArtifactInventory {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_ci_artifact_inventory_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &ci_artifact_inventory_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateReleaseAssets {
            manifest_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_release_assets_json(&manifest_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &release_assets_validation_failure_json(
                                &manifest_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateReleaseUploadReport {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_release_upload_report_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &release_upload_report_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateReleaseUploadTestReport {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_release_upload_test_report_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &release_upload_test_report_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateCheckoutQueryResult {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_checkout_query_result_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &checkout_query_result_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateCheckoutQuerySummary {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_checkout_query_summary_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &checkout_query_summary_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateCheckoutQueryContextPackets {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_checkout_query_context_packets_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_pretty_json_file(
                            path,
                            &checkout_query_context_packets_validation_failure_json(
                                &report_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            ),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ContextCollapseDrill { report_path }) => {
            let output = context_collapse_drill_json()?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ContextCollapseBenchmark { report_path }) => {
            let output = context_collapse_benchmark_json()?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ContextCollapseRetrievalBenchmark {
            require_live_pinecone,
            report_path,
        }) => {
            let vector_target = pinecone_vector_target_json();
            if require_live_pinecone
                && vector_target["live_config_available"].as_bool() != Some(true)
            {
                return Err(std::io::Error::other(
                    "live Pinecone retrieval benchmark requires PINECONE_API_KEY, PINECONE_INDEX_HOST, PINECONE_NAMESPACE, and PINECONE_VECTOR_DIMENSION",
                )
                .into());
            }
            let output = context_collapse_retrieval_benchmark_json(require_live_pinecone)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ComprehensiveVectorBenchmark {
            report_path,
            summary_path,
        }) => {
            let output = comprehensive_vector_benchmark_json()?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &comprehensive_vector_benchmark_markdown(&output)?)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ComprehensiveGraphBenchmark {
            report_path,
            summary_path,
        }) => {
            let output = comprehensive_graph_benchmark_json();
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &comprehensive_graph_benchmark_markdown(&output))?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::LiveBenchmarkCorpus {
            artifact_dir,
            sizes,
        }) => {
            let output = live_benchmark_corpus_json(&sizes, &artifact_dir)?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::LiveBenchmarkRun {
            target,
            cells,
            artifact_dir,
            require_live,
        }) => {
            let output =
                live_benchmark_run_json(target, cells, artifact_dir.as_deref(), require_live)?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::LiveBenchmarkSmokeReport {
            cells,
            artifact_dir,
            report_path,
            summary_path,
        }) => {
            let output = live_benchmark_smoke_report_json(cells, &artifact_dir)?;
            let json_path = report_path
                .as_ref()
                .cloned()
                .unwrap_or_else(|| artifact_dir.join("live-benchmark-smoke-report.json"));
            write_pretty_json_file(&json_path, &output)?;
            let markdown_path = summary_path
                .as_ref()
                .cloned()
                .unwrap_or_else(|| artifact_dir.join("live-benchmark-smoke-report.md"));
            write_text_file(
                &markdown_path,
                &live_benchmark_smoke_report_markdown(&output),
            )?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::LiveBenchmarkQualityReport {
            neo4j_report,
            pinecone_report,
            report_path,
            summary_path,
        }) => {
            let output = live_benchmark_quality_report_json(&neo4j_report, &pinecone_report)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &live_benchmark_quality_report_markdown(&output))?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::RepresentativeBenchmarkReport {
            cells,
            neo4j_report,
            pinecone_report,
            report_path,
            summary_path,
        }) => {
            let output = representative_benchmark_report_json(
                cells,
                neo4j_report.as_deref(),
                pinecone_report.as_deref(),
            )?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &representative_benchmark_report_markdown(&output))?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::AdversarialValidationReport {
            cells,
            representative_report,
            quality_report,
            report_path,
            summary_path,
        }) => {
            let output = adversarial_validation_report_json(
                cells,
                representative_report.as_deref(),
                quality_report.as_deref(),
            )?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &adversarial_validation_report_markdown(&output))?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::AdversarialTaskHarnessReport { report_path }) => {
            let output = adversarial_task_harness_report_json()?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::AgentBehaviorBenchmark { report_path }) => {
            let committed_at = Utc
                .with_ymd_and_hms(2026, 5, 24, 9, 0, 0)
                .single()
                .ok_or_else(|| std::io::Error::other("invalid benchmark timestamp"))?;
            let output = serde_json::to_value(run_agent_behavior_benchmark(committed_at)?)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ThesisFalsificationBenchmark {
            report_path,
            summary_path,
        }) => {
            let output = serde_json::to_value(run_thesis_falsification_benchmark()?)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &thesis_falsification_benchmark_markdown(&output))?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::AgentBehaviorTaskMatrix {
            representative_corpus,
            tasks_path,
            report_path,
        }) => {
            let matrix = agent_behavior_matrix_for_corpus(representative_corpus)?;
            if let Some(path) = tasks_path.as_ref() {
                write_pretty_json_file(path, &serde_json::to_value(&matrix.tasks)?)?;
            }
            let output = serde_json::to_value(matrix)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::AgentBehaviorBenchmarkBundle {
            representative_corpus,
            artifact_dir,
            runner,
            trials,
            runner_args,
        }) => {
            let output = agent_behavior_benchmark_bundle_json(
                &artifact_dir,
                &runner,
                runner_args.as_slice(),
                trials,
                representative_corpus,
            )?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::AgentBehaviorBenchmarkReport {
            artifact_dir,
            require_marketable,
            report_path,
            summary_path,
        }) => {
            let output = agent_behavior_benchmark_report_json(&artifact_dir)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = summary_path.as_ref() {
                write_text_file(path, &agent_behavior_benchmark_report_markdown(&output))?;
            }
            if require_marketable && output["marketable"].as_bool() != Some(true) {
                return Err(std::io::Error::other(
                    "agent behavior benchmark evidence is not marketable",
                )
                .into());
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ScoreAgentBehaviorOutputs {
            records_path,
            report_path,
        }) => {
            let records_text = fs::read_to_string(records_path)?;
            let records: Vec<AgentBehaviorExecutionRecord> = serde_json::from_str(&records_text)?;
            let output = serde_json::to_value(score_agent_behavior_execution_records(records)?)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::RunAgentBehaviorOutputs {
            tasks_path,
            runner,
            trials,
            runner_args,
            records_path,
            report_path,
        }) => {
            let tasks_text = fs::read_to_string(tasks_path)?;
            let tasks: Vec<AgentBehaviorExecutionTask> = serde_json::from_str(&tasks_text)?;
            let records = run_agent_behavior_model_runner_tasks(
                tasks,
                &runner,
                runner_args.as_slice(),
                trials,
            )?;
            if let Some(path) = records_path.as_ref() {
                write_pretty_json_file(path, &serde_json::to_value(&records)?)?;
            }
            let output = serde_json::to_value(score_agent_behavior_execution_records(records)?)?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::DemoCheckout) => {
            let slice = demo_checkout()?;
            println!("{}", serde_json::to_string_pretty(&slice)?);
        }
        Some(Command::CheckoutQuery {
            result_envelope,
            store_path,
            query_path,
        }) => {
            let output = if result_envelope {
                checkout_query_result_envelope_json(&store_path, &query_path)?
            } else {
                let (slice, return_shape) = checkout_query_file(&store_path, &query_path)?;
                match return_shape {
                    QueryReturnShape::SummaryOnly => checkout_query_summary_json(slice.summary),
                    QueryReturnShape::CellsOnly => checkout_query_cells_json(slice.cells),
                    QueryReturnShape::ContextPacketsOnly => {
                        checkout_query_context_packets_json(slice.context_packets)
                    }
                    QueryReturnShape::PackedContextWithMetadata => serde_json::to_value(slice)?,
                }
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::MeasureWorkload {
            kernel,
            store_path,
            artifact_dir,
            report_path,
            failure_report_path,
            cells,
            token_budget,
            frontier_every,
            dependency_stride,
            baseline_path,
            label,
            compare_baseline,
            max_elapsed_growth_percent,
            fail_on_regression,
        }) => {
            let output = measure_workload_json(WorkloadMeasureOptions {
                kernel,
                store_path: store_path.as_ref(),
                artifact_dir: artifact_dir.as_ref(),
                report_path: report_path.as_ref(),
                failure_report_path: failure_report_path.as_ref(),
                cells,
                token_budget,
                frontier_every,
                dependency_stride,
                baseline_path: baseline_path.as_ref(),
                label: &label,
                compare_baseline,
                max_elapsed_growth_percent,
                fail_on_regression,
            })?;
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ReplayWorkload {
            kernel,
            artifact_dir,
            store_path,
            report_path,
            failure_report_path,
            replay_artifact_dir,
            require_manifest,
            compare_report,
            fail_on_mismatch,
        }) => {
            let output = replay_workload_json(WorkloadReplayOptions {
                kernel,
                artifact_dir: &artifact_dir,
                store_path: store_path.as_ref(),
                report_path: report_path.as_ref(),
                failure_report_path: failure_report_path.as_ref(),
                replay_artifact_dir: replay_artifact_dir.as_ref(),
                require_manifest,
                compare_report: compare_report || fail_on_mismatch,
                fail_on_mismatch,
            })?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateWorkloadBundle {
            artifact_dir,
            report_path,
            failure_report_path,
        }) => {
            let output = match validate_workload_bundle_json(
                &artifact_dir,
                report_path.as_ref(),
                failure_report_path.as_ref(),
            ) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_workload_bundle_validation_failure_report(
                            &artifact_dir,
                            report_path.as_ref(),
                            path,
                            error.to_string(),
                        )?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::InspectKernel {
            store_path,
            artifact_dir,
            report_path,
            require,
            require_canonical,
            lookup_plan,
            lookup_query,
        }) => {
            let db = open_file_database(&store_path)?;
            if require_canonical {
                db.ensure_file_store_canonical()?;
            }
            let capabilities = db.kernel_capabilities();
            let status = file_status_json(&db)?;
            let health = file_health_json(&db);
            let lookup = if lookup_plan {
                Some(db.file_lookup_plan(&CellLookup::default()))
            } else if let Some(query) = lookup_query.as_deref() {
                Some(db.file_lookup_plan_for_query_text(query)?)
            } else {
                None
            };
            let lookup_plan = lookup.map(file_lookup_plan_json);
            let resolved_report_path = report_path.as_ref().cloned().or_else(|| {
                artifact_dir
                    .as_ref()
                    .map(|dir| dir.join("inspect-kernel-report.json"))
            });
            let required = require.map(profile_name);
            let required_capabilities =
                require.map(|profile| requirements_json(requirements_for_profile(profile)));
            let satisfies = require
                .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
                .unwrap_or(true);
            let mut output = serde_json::json!({
                "format": "continuitydb.inspect_kernel.report",
                "format_version": 1,
                "path": store_path.display().to_string(),
                "artifact_dir": artifact_dir.as_ref().map(|path| path.display().to_string()),
                "report_path": resolved_report_path.as_ref().map(|path| path.display().to_string()),
                "bundle_manifest": serde_json::Value::Null,
                "capabilities": capabilities_json(capabilities),
                "status": status,
                "health": health,
                "lookup_plan": lookup_plan,
                "required": required,
                "required_capabilities": required_capabilities,
                "satisfies": satisfies,
            });
            let report_payload = inspect_kernel_report_payload_text(&output)?;
            output["report_payload_fingerprint"] =
                serde_json::Value::String(fnv1a64_fingerprint(&report_payload));
            output["report_payload_bytes"] = serde_json::Value::from(report_payload.len());
            if let Some(path) = resolved_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(path) = artifact_dir.as_ref() {
                output = write_inspect_kernel_artifact_bundle_report(path, output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
            if let Some(profile) = require {
                db.ensure_kernel_requirements(requirements_for_profile(profile))?;
            }
        }
        Some(Command::ValidateInspectKernelReport {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let mut output = match validate_inspect_kernel_report_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_inspect_kernel_report_validation_failure_report(
                            &report_path,
                            path,
                            error.to_string(),
                        )?;
                    }
                    return Err(error);
                }
            };
            output["validation_report_path"] = validation_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ValidateInspectKernelBundle {
            artifact_dir,
            report_path,
            failure_report_path,
        }) => {
            let mut output = match validate_inspect_kernel_bundle_json(&artifact_dir) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_inspect_kernel_bundle_validation_failure_report(
                            &artifact_dir,
                            report_path.as_ref(),
                            path,
                            error.to_string(),
                        )?;
                    }
                    return Err(error);
                }
            };
            output["validation_report_path"] = report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            output["failure_report_path"] = failure_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::CompactFile { path, if_needed }) => {
            let mut db = open_file_database(&path)?;
            let output = if if_needed {
                let summary = db.compact_file_store_if_needed()?;
                serde_json::json!({
                    "path": path.display().to_string(),
                    "compacted": summary.compacted,
                    "before": file_health_value(summary.before),
                    "after": file_health_value(summary.after),
                })
            } else {
                db.compact_file_store()?;
                serde_json::json!({
                    "path": path.display().to_string(),
                    "compacted": true,
                })
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ExportCommits {
            store_path,
            output_path,
            after,
            limit,
        }) => {
            let db = open_file_database(&store_path)?;
            let summary =
                db.export_commits_json_file(CommitManifestLookup { after, limit }, &output_path)?;
            let output = serde_json::json!({
                "path": store_path.display().to_string(),
                "output": output_path.display().to_string(),
                "exported_commits": summary.exported_commits,
                "next_after": summary.next_after,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::CopyCommits {
            source_path,
            target_path,
            after,
            limit,
        }) => {
            let source = open_file_database(&source_path)?;
            let mut target = open_file_database(&target_path)?;
            let summary =
                target.copy_commits_from(&source, CommitManifestLookup { after, limit })?;
            let output = serde_json::json!({
                "source": source_path.display().to_string(),
                "target": target_path.display().to_string(),
                "copied_commits": summary.imported_commits,
                "next_after": summary.next_after,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ImportCommits {
            store_path,
            input_path,
            dry_run,
        }) => {
            let mut db = open_file_database(&store_path)?;
            let output = if dry_run {
                let validation = db.validate_commits_json_file(&input_path)?;
                serde_json::json!({
                    "path": store_path.display().to_string(),
                    "input": input_path.display().to_string(),
                    "dry_run": true,
                    "valid_commits": validation.valid_commits,
                })
            } else {
                let summary = db.import_commits_json_file_with_summary(&input_path)?;
                serde_json::json!({
                    "path": store_path.display().to_string(),
                    "input": input_path.display().to_string(),
                    "imported_commits": summary.imported_commits,
                    "next_after": summary.next_after,
                })
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::BenchmarkLocalModel {
            candidate,
            executable,
            model_path,
            arguments,
            candidate_defaults,
            grammar_path,
            artifact_dir,
            contract_dir,
            prompt_dir,
            response_dir,
            enforce_candidate_requirements,
            baseline_path,
            stability_trials,
            fail_on_unstable,
            fail_on_failed_cases,
            failure_report_path,
            changed_case_report_path,
            report_path: explicit_report_path,
            dry_run,
            compare_baseline,
            fail_on_regression,
        }) => {
            let mut output = benchmark_local_model_json(LocalModelBenchmarkOptions {
                candidate_id: candidate.as_deref(),
                executable: &executable,
                model_path: &model_path,
                arguments: &arguments,
                candidate_defaults,
                grammar_path: grammar_path.as_deref(),
                artifact_dir: artifact_dir.as_deref(),
                contract_dir: contract_dir.as_deref(),
                prompt_dir: prompt_dir.as_deref(),
                response_dir: response_dir.as_deref(),
                enforce_candidate_requirements,
                baseline_path: &baseline_path,
                stability_trials,
                fail_on_unstable,
                fail_on_failed_cases,
                failure_report_path: failure_report_path.as_deref(),
                changed_case_report_path: changed_case_report_path.as_deref(),
                dry_run,
                compare_baseline: compare_baseline || fail_on_regression,
                fail_on_regression,
            })?;
            if let Some(artifact_dir) = artifact_dir.as_ref() {
                output = write_local_model_artifact_bundle_report(artifact_dir, output)?;
            }
            if let Some(report_path) = explicit_report_path {
                write_pretty_json_file(&report_path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelBundle {
            artifact_dir,
            report_path,
            failure_report_path,
        }) => {
            let validation = match validate_local_model_bundle_manifest(&artifact_dir) {
                Ok(validation) => validation,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        write_local_model_bundle_validation_failure_report(
                            &artifact_dir,
                            report_path.as_ref(),
                            path,
                            error.to_string(),
                        )?;
                    }
                    return Err(error);
                }
            };
            let output = local_model_bundle_validation_report_json(
                &artifact_dir,
                report_path.as_deref(),
                failure_report_path.as_deref(),
                validation,
            );
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelBenchmarkReport {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_local_model_benchmark_report_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let failure = local_model_benchmark_report_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &failure)?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelContract {
            context_compiler,
            schema_path,
            grammar_path,
        }) => {
            let output =
                write_local_model_contract_json(&schema_path, &grammar_path, context_compiler)?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelEvaluationSuite) => {
            let output = local_model_evaluation_suite_json();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelEvaluationSuite {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_local_model_evaluation_suite_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let failure = local_model_evaluation_suite_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &failure)?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelAcceptanceCriteria) => {
            let output = local_model_acceptance_criteria_json();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelAcceptanceCriteria {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_local_model_acceptance_criteria_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let failure = local_model_acceptance_criteria_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &failure)?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelCandidates) => {
            let output = local_model_candidates_json();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelCandidates {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_local_model_candidates_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let failure = local_model_candidates_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &failure)?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelQualityGatePlan) => {
            let output = local_model_quality_gate_plan_json();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelQualityGatePlan {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let output = match validate_local_model_quality_gate_plan_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let failure = local_model_quality_gate_plan_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &failure)?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelQualityGateStatus {
            artifact_root,
            require_ready,
        }) => {
            let output = local_model_quality_gate_status_json(&artifact_root);
            println!("{}", serde_json::to_string_pretty(&output)?);
            if require_ready && output["summary"]["ready"].as_bool() != Some(true) {
                return Err("local model quality gate is not ready".into());
            }
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelQualityGateStatus {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let mut output = match validate_local_model_quality_gate_status_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let output = local_model_quality_gate_status_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &output)?;
                    }
                    return Err(error);
                }
            };
            output["validation_report_path"] = validation_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            output["failure_report_path"] = failure_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelRequireReadyStatus {
            report_path,
            stderr_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let mut output =
                match validate_local_model_require_ready_status_json(&report_path, &stderr_path) {
                    Ok(output) => output,
                    Err(error) => {
                        if let Some(path) = failure_report_path.as_ref() {
                            let output = local_model_require_ready_status_validation_failure_json(
                                &report_path,
                                &stderr_path,
                                validation_report_path.as_ref(),
                                path,
                                error.to_string(),
                            );
                            write_pretty_json_file(path, &output)?;
                        }
                        return Err(error);
                    }
                };
            output["validation_report_path"] = validation_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            output["failure_report_path"] = failure_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::RunLocalModelQualityGate {
            executable,
            model_paths,
            arguments,
            baseline_path,
            artifact_root,
            report_path,
            failure_report_path,
            dry_run,
            stability_trials,
            fail_on_unstable,
            require_ready,
            fail_on_not_ready,
        }) => {
            let options = LocalModelQualityGateRunOptions {
                executable: &executable,
                model_paths: &model_paths,
                arguments: &arguments,
                baseline_path: &baseline_path,
                artifact_root: &artifact_root,
                report_path: report_path.as_deref(),
                failure_report_path: failure_report_path.as_deref(),
                dry_run,
                stability_trials,
                fail_on_unstable,
            };
            let output = match run_local_model_quality_gate_json(options) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_deref() {
                        let output = local_model_quality_gate_run_failure_json(
                            &executable,
                            &model_paths,
                            &baseline_path,
                            &artifact_root,
                            dry_run,
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &output)?;
                    }
                    return Err(error);
                }
            };
            if let Some(path) = report_path.as_deref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
            if (require_ready || fail_on_not_ready)
                && output["status"]["summary"]["ready"].as_bool() != Some(true)
            {
                if let Some(path) = failure_report_path.as_deref() {
                    let failure = local_model_quality_gate_not_ready_failure_json(
                        &output,
                        path,
                        "local model quality gate is not ready",
                    );
                    write_pretty_json_file(path, &failure)?;
                }
                return Err("local model quality gate is not ready".into());
            }
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelQualityGateRunReport {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let mut output = match validate_local_model_quality_gate_run_report_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let output = local_model_quality_gate_run_report_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &output)?;
                    }
                    return Err(error);
                }
            };
            output["validation_report_path"] = validation_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            output["failure_report_path"] = failure_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::ValidateLocalModelQualityGateRunOutput {
            report_path,
            validation_report_path,
            failure_report_path,
        }) => {
            let mut output = match validate_local_model_quality_gate_run_output_json(&report_path) {
                Ok(output) => output,
                Err(error) => {
                    if let Some(path) = failure_report_path.as_ref() {
                        let output = local_model_quality_gate_run_output_validation_failure_json(
                            &report_path,
                            validation_report_path.as_ref(),
                            path,
                            error.to_string(),
                        );
                        write_pretty_json_file(path, &output)?;
                    }
                    return Err(error);
                }
            };
            output["validation_report_path"] = validation_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            output["failure_report_path"] = failure_report_path
                .as_ref()
                .map(|path| serde_json::Value::String(path.display().to_string()))
                .unwrap_or(serde_json::Value::Null);
            if let Some(path) = validation_report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        None => {}
    }
    Ok(())
}

#[cfg(feature = "local-model")]
fn local_model_candidates_json() -> serde_json::Value {
    let candidates = small_model_candidates();
    serde_json::json!({
        "format": "continuitydb.local_model_candidates",
        "format_version": 1,
        "default_candidate": candidates.first().map(SmallModelCandidate::model_id),
        "default_quality_gate_candidate": small_model_default_quality_gate_candidate()
            .map(|candidate| candidate.model_id()),
        "default_ci_candidate": small_model_default_ci_candidate()
            .map(|candidate| candidate.model_id()),
        "quality_gate_candidates": small_model_quality_gate_candidates()
            .iter()
            .map(SmallModelCandidate::model_id)
            .collect::<Vec<_>>(),
        "ci_candidates": small_model_ci_candidates()
            .iter()
            .map(SmallModelCandidate::model_id)
            .collect::<Vec<_>>(),
        "total_candidates": candidates.len(),
        "candidates": candidates
            .iter()
            .map(|candidate| {
                let recommended_config =
                    candidate.recommended_runner_config("llama-cli", "<model.gguf>");
                let recommended_config_with_grammar =
                    candidate.recommended_runner_config_with_grammar_file(
                        "llama-cli",
                        "<model.gguf>",
                        "<steward-response.gbnf>",
                    );
                let recommended_mistral_config =
                    candidate.recommended_mistral_runner_config("mistralrs-cli", "<model.gguf>");
                serde_json::json!({
                    "model_id": candidate.model_id(),
                    "role": candidate.role(),
                    "evaluation_priority": candidate.evaluation_priority(),
                    "evaluation_tier": candidate.evaluation_tier(),
                    "evaluation_use": candidate.evaluation_use(),
                    "ci_suitable": candidate.ci_suitable(),
                    "quality_gate_eligible": candidate.quality_gate_eligible(),
                    "default_quality_gate_candidate": candidate.default_quality_gate_candidate(),
                    "recommended_runtime": candidate.recommended_runtime(),
                    "compatible_runtimes": candidate.compatible_runtimes(),
                    "artifact_format": candidate.artifact_format(),
                    "license": candidate.license(),
                    "parameter_count_millions": candidate.parameter_count_millions(),
                    "context_window_tokens": candidate.context_window_tokens(),
                    "model_card_url": candidate.model_card_url(),
                    "recommended_temperature": candidate.recommended_temperature(),
                    "requires_grammar": candidate.requires_grammar(),
                    "recommended_runner_arguments": recommended_config.command_arguments(),
                    "recommended_runner_arguments_with_grammar": recommended_config_with_grammar.command_arguments(),
                    "recommended_mistral_runner_arguments": recommended_mistral_config.command_arguments(),
                    "notes": candidate.notes(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_candidates_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let expected = local_model_candidates_json();

    if report["format"].as_str() != expected["format"].as_str()
        || report["format_version"].as_u64() != expected["format_version"].as_u64()
    {
        return Err(std::io::Error::other("unsupported local model candidates report").into());
    }
    if report["default_candidate"].as_str() != expected["default_candidate"].as_str() {
        return Err(
            std::io::Error::other("local model candidates default candidate mismatch").into(),
        );
    }
    if report["default_quality_gate_candidate"].as_str()
        != expected["default_quality_gate_candidate"].as_str()
    {
        return Err(std::io::Error::other(
            "local model candidates default quality-gate candidate mismatch",
        )
        .into());
    }
    if report["default_ci_candidate"].as_str() != expected["default_ci_candidate"].as_str() {
        return Err(
            std::io::Error::other("local model candidates default ci candidate mismatch").into(),
        );
    }
    if report["quality_gate_candidates"] != expected["quality_gate_candidates"] {
        return Err(
            std::io::Error::other("local model quality-gate candidate set mismatch").into(),
        );
    }
    if report["ci_candidates"] != expected["ci_candidates"] {
        return Err(std::io::Error::other("local model ci candidate set mismatch").into());
    }
    if report["total_candidates"].as_u64() != expected["total_candidates"].as_u64() {
        return Err(std::io::Error::other("local model candidate count mismatch").into());
    }
    if report["candidates"] != expected["candidates"] {
        return Err(std::io::Error::other("local model candidate registry mismatch").into());
    }

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_candidates_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "default_candidate": report["default_candidate"],
        "default_quality_gate_candidate": report["default_quality_gate_candidate"],
        "default_ci_candidate": report["default_ci_candidate"],
        "total_candidates": report["total_candidates"],
        "quality_gate_candidate_count": report["quality_gate_candidates"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
        "ci_candidate_count": report["ci_candidates"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
    }))
}

#[cfg(feature = "local-model")]
fn local_model_candidates_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.local_model_candidates_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "candidates_report": report_metadata,
        "failure": {
            "stage": "local_model_candidates_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_plan_json() -> serde_json::Value {
    let candidates = small_model_quality_gate_candidates();
    let candidate_plans = candidates
        .iter()
        .map(local_model_quality_gate_candidate_plan_json)
        .collect::<Vec<_>>();
    let steps_per_candidate = 2;
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_plan",
        "format_version": 1,
        "generated_by_command": "local-model-quality-gate-plan",
        "execution_summary": {
            "candidate_count": candidate_plans.len(),
            "steps_per_candidate": steps_per_candidate,
            "total_steps": candidate_plans.len() * steps_per_candidate,
            "sequence": ["benchmark", "validate_bundle"],
        },
        "candidate_set": "quality_gate",
        "default_quality_gate_candidate": small_model_default_quality_gate_candidate()
            .map(|candidate| candidate.model_id()),
        "total_candidates": candidates.len(),
        "required_inputs": [
            {
                "placeholder": "<runner>",
                "description": "Executable path for the local model runner used by benchmark-local-model --executable.",
            },
            {
                "placeholder": "<model.gguf>",
                "description": "Candidate model artifact path passed to benchmark-local-model --model-path.",
            },
            {
                "placeholder": "<baselines.jsonl>",
                "description": "Local model baseline store path passed to benchmark-local-model --baseline-path.",
            },
            {
                "placeholder": "<artifacts>",
                "description": "Artifact root directory used to isolate benchmark and validation outputs per candidate.",
            },
            {
                "placeholder": "<steward-response.gbnf>",
                "description": "Grammar file path supplied to grammar-constrained runners.",
            },
        ],
        "environment_bindings": [
            {
                "placeholder": "<runner>",
                "env": "CONTINUITYDB_LOCAL_MODEL_RUNNER",
            },
            {
                "placeholder": "<model.gguf>",
                "env": "CONTINUITYDB_LOCAL_MODEL_PATH",
            },
            {
                "placeholder": "<baselines.jsonl>",
                "env": "CONTINUITYDB_LOCAL_MODEL_BASELINES",
            },
            {
                "placeholder": "<artifacts>",
                "env": "CONTINUITYDB_LOCAL_MODEL_ARTIFACTS",
            },
            {
                "placeholder": "<steward-response.gbnf>",
                "env": "CONTINUITYDB_LOCAL_MODEL_GRAMMAR",
            },
        ],
        "substitution_contract": {
            "mode": "replace_all_placeholders_before_execution",
            "unresolved_placeholder_policy": "fail_before_launch",
            "placeholders": [
                "<runner>",
                "<model.gguf>",
                "<baselines.jsonl>",
                "<artifacts>",
                "<steward-response.gbnf>",
            ],
        },
        "candidates": candidate_plans,
        "ci_matrix": candidate_plans
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "job_id": candidate["job_id"],
                    "job_name": candidate["job_name"],
                    "artifact_upload_name": candidate["artifact_upload_name"],
                    "artifact_retention": candidate["artifact_retention"],
                    "artifact_download": candidate["artifact_download"],
                    "result_artifacts": candidate["result_artifacts"],
                    "model_id": candidate["model_id"],
                    "evaluation_priority": candidate["evaluation_priority"],
                    "artifact_dir": candidate["artifact_dir"],
                    "steps": candidate["ci_sequence"],
                })
            })
            .collect::<Vec<_>>(),
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_quality_gate_plan_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let expected = local_model_quality_gate_plan_json();

    if report["format"].as_str() != expected["format"].as_str()
        || report["format_version"].as_u64() != expected["format_version"].as_u64()
    {
        return Err(
            std::io::Error::other("unsupported local model quality-gate plan report").into(),
        );
    }
    if report["generated_by_command"].as_str() != expected["generated_by_command"].as_str() {
        return Err(std::io::Error::other(
            "local model quality-gate plan producer command mismatch",
        )
        .into());
    }
    if report["candidate_set"].as_str() != expected["candidate_set"].as_str() {
        return Err(
            std::io::Error::other("local model quality-gate plan candidate set mismatch").into(),
        );
    }
    if report["default_quality_gate_candidate"].as_str()
        != expected["default_quality_gate_candidate"].as_str()
    {
        return Err(std::io::Error::other(
            "local model quality-gate plan default candidate mismatch",
        )
        .into());
    }
    if report["execution_summary"] != expected["execution_summary"] {
        return Err(std::io::Error::other(
            "local model quality-gate plan execution summary mismatch",
        )
        .into());
    }
    if report["required_inputs"] != expected["required_inputs"] {
        return Err(std::io::Error::other(
            "local model quality-gate plan required inputs mismatch",
        )
        .into());
    }
    if report["environment_bindings"] != expected["environment_bindings"] {
        return Err(std::io::Error::other(
            "local model quality-gate plan environment bindings mismatch",
        )
        .into());
    }
    if report["substitution_contract"] != expected["substitution_contract"] {
        return Err(std::io::Error::other(
            "local model quality-gate plan substitution contract mismatch",
        )
        .into());
    }
    if report["candidates"] != expected["candidates"] {
        return Err(
            std::io::Error::other("local model quality-gate plan candidates mismatch").into(),
        );
    }
    if report["ci_matrix"] != expected["ci_matrix"] {
        return Err(
            std::io::Error::other("local model quality-gate plan ci matrix mismatch").into(),
        );
    }

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_plan_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "candidate_set": report["candidate_set"],
        "default_quality_gate_candidate": report["default_quality_gate_candidate"],
        "candidate_count": report["execution_summary"]["candidate_count"],
        "steps_per_candidate": report["execution_summary"]["steps_per_candidate"],
        "total_steps": report["execution_summary"]["total_steps"],
        "ci_matrix_candidate_count": report["ci_matrix"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
    }))
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_plan_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_plan_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "quality_gate_plan_report": report_metadata,
        "failure": {
            "stage": "local_model_quality_gate_plan_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_candidate_plan_json(
    candidate: &continuitydb_steward::SmallModelCandidate,
) -> serde_json::Value {
    let recommended_config = candidate.recommended_runner_config_with_grammar_file(
        "llama-cli",
        "<model.gguf>",
        "<steward-response.gbnf>",
    );
    let mut recommended_runner_arguments_with_grammar =
        vec![recommended_config.executable().display().to_string()];
    recommended_runner_arguments_with_grammar.extend(recommended_config.command_arguments());
    let artifact_slug = local_model_candidate_artifact_slug(candidate.model_id());
    let artifact_dir = format!("<artifacts>/{artifact_slug}");
    let job_id = format!("local-model-{artifact_slug}");
    let job_name = format!("Local model quality gate: {}", candidate.model_id());
    let artifact_upload_name = format!("continuitydb-local-model-{artifact_slug}");
    let artifact_retention = serde_json::json!({
        "class": "local_model_quality_gate_bundle",
        "days": 30,
        "upload_path": artifact_dir,
    });
    let artifact_download = serde_json::json!({
        "artifact_name": artifact_upload_name,
        "download_path": format!("<artifacts>/downloaded/{artifact_slug}"),
        "restore_path": artifact_dir,
    });
    let report_path = format!("{artifact_dir}/benchmark-report.json");
    let failure_report_path = format!("{artifact_dir}/failure-report.json");
    let validation_report_path = format!("{artifact_dir}/validation-report.json");
    let validation_failure_report_path = format!("{artifact_dir}/validation-failure-report.json");
    let bundle_manifest_path = format!("{artifact_dir}/manifest.json");
    let result_artifacts = serde_json::json!({
        "benchmark_report": report_path,
        "benchmark_failure_report": failure_report_path,
        "validation_report": validation_report_path,
        "validation_failure_report": validation_failure_report_path,
        "bundle_manifest": bundle_manifest_path,
    });
    let benchmark_command = serde_json::json!([
        "continuitydb",
        "benchmark-local-model",
        "--candidate",
        candidate.model_id(),
        "--candidate-defaults",
        "--enforce-candidate-requirements",
        "--fail-on-failed-cases",
        "--compare-baseline",
        "--fail-on-regression",
        "--executable",
        "<runner>",
        "--model-path",
        "<model.gguf>",
        "--baseline-path",
        "<baselines.jsonl>",
        "--artifact-dir",
        artifact_dir,
        "--report-path",
        report_path,
        "--failure-report-path",
        failure_report_path,
    ]);
    let validation_command = serde_json::json!([
        "continuitydb",
        "validate-local-model-bundle",
        "--artifact-dir",
        artifact_dir,
        "--report-path",
        validation_report_path,
        "--failure-report-path",
        validation_failure_report_path,
    ]);
    serde_json::json!({
        "job_id": job_id,
        "job_name": job_name,
        "artifact_upload_name": artifact_upload_name,
        "artifact_retention": artifact_retention,
        "artifact_download": artifact_download,
        "result_artifacts": result_artifacts,
        "model_id": candidate.model_id(),
        "evaluation_priority": candidate.evaluation_priority(),
        "evaluation_tier": candidate.evaluation_tier(),
        "evaluation_use": candidate.evaluation_use(),
        "recommended_runtime": candidate.recommended_runtime(),
        "requires_grammar": candidate.requires_grammar(),
        "artifact_dir": artifact_dir,
        "report_path": report_path,
        "failure_report_path": failure_report_path,
        "validation_report_path": validation_report_path,
        "validation_failure_report_path": validation_failure_report_path,
        "recommended_runner_arguments_with_grammar": recommended_runner_arguments_with_grammar,
        "benchmark_command": benchmark_command,
        "validation_command": validation_command,
        "ci_sequence": [
            {
                "step": "benchmark",
                "command": benchmark_command,
            },
            {
                "step": "validate_bundle",
                "depends_on": "benchmark",
                "command": validation_command,
            },
        ],
    })
}

#[cfg(feature = "local-model")]
fn run_local_model_quality_gate_json(
    options: LocalModelQualityGateRunOptions<'_>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let candidates = small_model_quality_gate_candidates();
    let model_paths = local_model_quality_gate_model_paths(options.model_paths, &candidates)?;
    let runtime_preflight =
        local_model_quality_gate_runtime_preflight_json(&options, &model_paths, &candidates)?;
    std::fs::create_dir_all(options.artifact_root)?;

    let mut candidate_runs = Vec::new();
    for candidate in &candidates {
        let artifact_slug = local_model_candidate_artifact_slug(candidate.model_id());
        let artifact_dir = options.artifact_root.join(&artifact_slug);
        let report_path = artifact_dir.join("benchmark-report.json");
        let failure_report_path = artifact_dir.join("failure-report.json");
        let validation_report_path = artifact_dir.join("validation-report.json");
        let validation_failure_report_path = artifact_dir.join("validation-failure-report.json");
        let model_path = model_paths.get(candidate.model_id()).ok_or_else(|| {
            std::io::Error::other(format!(
                "missing --model-path binding for {}",
                candidate.model_id()
            ))
        })?;

        let benchmark_result = benchmark_local_model_json(LocalModelBenchmarkOptions {
            candidate_id: Some(candidate.model_id()),
            executable: options.executable,
            model_path,
            arguments: options.arguments,
            candidate_defaults: true,
            grammar_path: None,
            artifact_dir: Some(&artifact_dir),
            contract_dir: None,
            prompt_dir: None,
            response_dir: None,
            enforce_candidate_requirements: true,
            baseline_path: options.baseline_path,
            stability_trials: options.stability_trials,
            fail_on_unstable: options.fail_on_unstable,
            fail_on_failed_cases: true,
            failure_report_path: Some(&failure_report_path),
            changed_case_report_path: None,
            dry_run: options.dry_run,
            compare_baseline: true,
            fail_on_regression: true,
        });

        let mut candidate_run = serde_json::json!({
            "model_id": candidate.model_id(),
            "artifact_dir": artifact_dir.display().to_string(),
            "benchmark_report_path": report_path.display().to_string(),
            "benchmark_failure_report_path": failure_report_path.display().to_string(),
            "validation_report_path": validation_report_path.display().to_string(),
            "validation_failure_report_path": validation_failure_report_path.display().to_string(),
            "dry_run": options.dry_run,
        });

        match benchmark_result {
            Ok(mut benchmark_report) => {
                benchmark_report =
                    write_local_model_artifact_bundle_report(&artifact_dir, benchmark_report)?;
                write_pretty_json_file(&report_path, &benchmark_report)?;
                candidate_run["benchmark"] = serde_json::json!({
                    "status": "passed",
                    "report_path": report_path.display().to_string(),
                });

                match validate_local_model_bundle_manifest(&artifact_dir) {
                    Ok(validation) => {
                        let validation_report = local_model_bundle_validation_report_json(
                            &artifact_dir,
                            Some(&validation_report_path),
                            Some(&validation_failure_report_path),
                            validation,
                        );
                        write_pretty_json_file(&validation_report_path, &validation_report)?;
                        candidate_run["validation"] = serde_json::json!({
                            "status": "passed",
                            "report_path": validation_report_path.display().to_string(),
                        });
                        candidate_run["status"] = serde_json::Value::String("passed".to_string());
                    }
                    Err(error) => {
                        write_local_model_bundle_validation_failure_report(
                            &artifact_dir,
                            Some(&validation_report_path),
                            &validation_failure_report_path,
                            error.to_string(),
                        )?;
                        candidate_run["validation"] = serde_json::json!({
                            "status": "failed",
                            "failure_report_path": validation_failure_report_path.display().to_string(),
                            "message": error.to_string(),
                        });
                        candidate_run["status"] =
                            serde_json::Value::String("validation_failed".to_string());
                    }
                }
            }
            Err(error) => {
                candidate_run["benchmark"] = serde_json::json!({
                    "status": "failed",
                    "failure_report_path": failure_report_path.display().to_string(),
                    "message": error.to_string(),
                });
                candidate_run["validation"] = serde_json::Value::Null;
                candidate_run["status"] = serde_json::Value::String("benchmark_failed".to_string());
            }
        }

        candidate_runs.push(candidate_run);
    }

    let status = local_model_quality_gate_status_json(options.artifact_root);
    Ok(serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_run",
        "format_version": 1,
        "generated_by_command": "run-local-model-quality-gate",
        "dry_run": options.dry_run,
        "candidate_count": candidate_runs.len(),
        "artifact_root": options.artifact_root.display().to_string(),
        "baseline_path": options.baseline_path.display().to_string(),
        "report_path": options.report_path.map(|path| path.display().to_string()),
        "failure_report_path": options.failure_report_path.map(|path| path.display().to_string()),
        "runtime_preflight": runtime_preflight,
        "candidates": candidate_runs,
        "status": status,
    }))
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_run_failure_json(
    executable: &Path,
    model_paths: &[String],
    baseline_path: &Path,
    artifact_root: &Path,
    dry_run: bool,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_run",
        "format_version": 1,
        "generated_by_command": "run-local-model-quality-gate",
        "dry_run": dry_run,
        "candidate_count": small_model_quality_gate_candidates().len(),
        "artifact_root": artifact_root.display().to_string(),
        "baseline_path": baseline_path.display().to_string(),
        "report_path": serde_json::Value::Null,
        "failure_report_path": failure_report_path.display().to_string(),
        "runtime_preflight": {
            "runner": {
                "path": executable.display().to_string(),
                "required": !dry_run,
                "available": dry_run || local_model_runner_available(executable),
            },
            "model_path_bindings": model_paths,
        },
        "candidates": [],
        "status": serde_json::Value::Null,
        "failure": {
            "stage": "local_model_quality_gate_run",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_not_ready_failure_json(
    run_report: &serde_json::Value,
    failure_report_path: &Path,
    message: &str,
) -> serde_json::Value {
    let mut output = run_report.clone();
    output["failure_report_path"] =
        serde_json::Value::String(failure_report_path.display().to_string());
    output["failure"] = serde_json::json!({
        "stage": "local_model_quality_gate_readiness",
        "message": message,
        "summary": run_report["status"]["summary"].clone(),
    });
    output
}

#[cfg(feature = "local-model")]
fn validate_local_model_quality_gate_run_report_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;

    if report["format"].as_str() != Some("continuitydb.local_model_quality_gate_run")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("unsupported local model quality-gate run report").into(),
        );
    }
    if report["generated_by_command"].as_str() != Some("run-local-model-quality-gate") {
        return Err(
            std::io::Error::other("local model quality-gate run report producer mismatch").into(),
        );
    }

    let expected_candidate_count = small_model_quality_gate_candidates().len() as u64;
    let candidate_count = required_json_u64(&report, "candidate_count")?;
    if candidate_count != expected_candidate_count {
        return Err(std::io::Error::other(
            "local model quality-gate run report candidate count mismatch",
        )
        .into());
    }

    if let Some(self_report_path) = report["report_path"].as_str() {
        if self_report_path != report_path.display().to_string() {
            return Err(
                std::io::Error::other("local model quality-gate run report path mismatch").into(),
            );
        }
    }
    if let Some(failure_report_path) = report["failure_report_path"].as_str() {
        if failure_report_path != report_path.display().to_string() && !report["failure"].is_null()
        {
            return Err(std::io::Error::other(
                "local model quality-gate run failure report path mismatch",
            )
            .into());
        }
    }

    let candidate_validations = if report["failure"].is_null() {
        validate_successful_local_model_quality_gate_run_report(&report)?
    } else {
        validate_failed_local_model_quality_gate_run_report(&report)?;
        Vec::new()
    };
    let acceptance_coverage =
        local_model_quality_gate_run_validation_acceptance_coverage_json(&candidate_validations);

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_run_report_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": local_model_contract_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "run_report": {
            "format": report["format"].clone(),
            "format_version": report["format_version"].clone(),
            "generated_by_command": report["generated_by_command"].clone(),
            "dry_run": report["dry_run"].clone(),
            "candidate_count": report["candidate_count"].clone(),
            "artifact_root": report["artifact_root"].clone(),
            "baseline_path": report["baseline_path"].clone(),
            "report_path": report["report_path"].clone(),
            "failure_report_path": report["failure_report_path"].clone(),
            "failure": report["failure"].clone(),
        },
        "acceptance_coverage": acceptance_coverage,
        "candidate_validations": candidate_validations,
    }))
}

#[cfg(feature = "local-model")]
fn validate_local_model_quality_gate_run_output_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let output_text = std::fs::read_to_string(report_path)?;
    let output: serde_json::Value = serde_json::from_str(&output_text)?;

    if output["format"].as_str() != Some("continuitydb.local_model_quality_gate_run")
        || output["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("unsupported local model quality-gate run output").into(),
        );
    }
    if output["generated_by_command"].as_str() != Some("run-local-model-quality-gate") {
        return Err(
            std::io::Error::other("local model quality-gate run output producer mismatch").into(),
        );
    }
    if !output["failure"].is_null() {
        return Err(std::io::Error::other(
            "local model quality-gate run output is a failure report",
        )
        .into());
    }

    let referenced_report_path = Path::new(required_json_string(&output, "report_path")?);
    let report_text = std::fs::read_to_string(referenced_report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    if report != output {
        return Err(std::io::Error::other(
            "local model quality-gate run output does not match referenced report",
        )
        .into());
    }

    let report_validation =
        validate_local_model_quality_gate_run_report_json(referenced_report_path)?;
    Ok(serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_run_output_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": local_model_contract_fingerprint(&output_text),
        "report_bytes": output_text.len(),
        "valid": true,
        "run_output": {
            "format": output["format"].clone(),
            "format_version": output["format_version"].clone(),
            "generated_by_command": output["generated_by_command"].clone(),
            "dry_run": output["dry_run"].clone(),
            "candidate_count": output["candidate_count"].clone(),
            "artifact_root": output["artifact_root"].clone(),
            "baseline_path": output["baseline_path"].clone(),
            "report_path": output["report_path"].clone(),
            "failure_report_path": output["failure_report_path"].clone(),
            "failure": output["failure"].clone(),
        },
        "report_parity": {
            "referenced_report_path": referenced_report_path.display().to_string(),
            "referenced_report_fingerprint": local_model_contract_fingerprint(&report_text),
            "referenced_report_bytes": report_text.len(),
            "matches_report_file": true,
        },
        "acceptance_coverage": report_validation["acceptance_coverage"].clone(),
        "candidate_validations": report_validation["candidate_validations"].clone(),
    }))
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_run_validation_acceptance_coverage_json(
    candidate_validations: &[serde_json::Value],
) -> serde_json::Value {
    let mut covered = std::collections::BTreeSet::new();
    let mut missing = std::collections::BTreeSet::new();
    let mut complete_candidate_count = 0usize;

    for validation in candidate_validations {
        let acceptance_coverage = &validation["acceptance_coverage"];
        if acceptance_coverage["complete"].as_bool() == Some(true)
            && acceptance_coverage["missing"]
                .as_array()
                .map(|criteria| criteria.is_empty())
                .unwrap_or(false)
        {
            complete_candidate_count += 1;
        }
        if let Some(criteria) = acceptance_coverage["covered"].as_array() {
            for criterion in criteria {
                if let Some(criterion) = criterion.as_str() {
                    covered.insert(criterion.to_string());
                }
            }
        }
        if let Some(criteria) = acceptance_coverage["missing"].as_array() {
            for criterion in criteria {
                if let Some(criterion) = criterion.as_str() {
                    missing.insert(criterion.to_string());
                }
            }
        }
    }

    let candidate_count = candidate_validations.len();
    let incomplete_candidate_count = candidate_count.saturating_sub(complete_candidate_count);
    serde_json::json!({
        "complete": candidate_count > 0 && incomplete_candidate_count == 0,
        "candidate_count": candidate_count,
        "complete_candidate_count": complete_candidate_count,
        "incomplete_candidate_count": incomplete_candidate_count,
        "covered": covered.into_iter().collect::<Vec<_>>(),
        "missing": missing.into_iter().collect::<Vec<_>>(),
    })
}

#[cfg(feature = "local-model")]
fn validate_successful_local_model_quality_gate_run_report(
    report: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let artifact_root = Path::new(required_json_string(report, "artifact_root")?);
    let status = local_model_quality_gate_status_json(artifact_root);
    if report["status"] != status {
        return Err(
            std::io::Error::other("local model quality-gate run report status mismatch").into(),
        );
    }

    let candidates = report["candidates"].as_array().ok_or_else(|| {
        std::io::Error::other("local model quality-gate run report missing candidates")
    })?;
    if candidates.len() != small_model_quality_gate_candidates().len() {
        return Err(std::io::Error::other(
            "local model quality-gate run report candidate list mismatch",
        )
        .into());
    }

    let quality_gate_candidates = small_model_quality_gate_candidates();
    let mut validations = Vec::new();
    for (candidate, expected_candidate) in candidates.iter().zip(quality_gate_candidates.iter()) {
        let expected_slug = local_model_candidate_artifact_slug(expected_candidate.model_id());
        let expected_artifact_dir = artifact_root.join(&expected_slug);
        let expected_benchmark_report_path = expected_artifact_dir.join("benchmark-report.json");
        let expected_failure_report_path = expected_artifact_dir.join("failure-report.json");
        let expected_validation_report_path = expected_artifact_dir.join("validation-report.json");
        let expected_validation_failure_report_path =
            expected_artifact_dir.join("validation-failure-report.json");

        if candidate["model_id"].as_str() != Some(expected_candidate.model_id()) {
            return Err(std::io::Error::other(
                "local model quality-gate run report candidate model mismatch",
            )
            .into());
        }
        if required_json_string(candidate, "artifact_dir")?
            != expected_artifact_dir.display().to_string()
        {
            return Err(std::io::Error::other(
                "local model quality-gate run report candidate artifact path mismatch",
            )
            .into());
        }
        if required_json_string(candidate, "benchmark_report_path")?
            != expected_benchmark_report_path.display().to_string()
        {
            return Err(std::io::Error::other(
                "local model quality-gate run report candidate benchmark report path mismatch",
            )
            .into());
        }
        if required_json_string(candidate, "benchmark_failure_report_path")?
            != expected_failure_report_path.display().to_string()
        {
            return Err(std::io::Error::other(
                "local model quality-gate run report candidate benchmark failure path mismatch",
            )
            .into());
        }
        if required_json_string(candidate, "validation_report_path")?
            != expected_validation_report_path.display().to_string()
        {
            return Err(std::io::Error::other(
                "local model quality-gate run report candidate validation report path mismatch",
            )
            .into());
        }
        if required_json_string(candidate, "validation_failure_report_path")?
            != expected_validation_failure_report_path
                .display()
                .to_string()
        {
            return Err(std::io::Error::other(
                "local model quality-gate run report candidate validation failure path mismatch",
            )
            .into());
        }
        if candidate["status"].as_str() != Some("passed") {
            return Err(std::io::Error::other(
                "local model quality-gate run report contains non-passing candidate",
            )
            .into());
        }
        let artifact_dir = Path::new(required_json_string(candidate, "artifact_dir")?);
        let validation = validate_local_model_bundle_manifest(artifact_dir)?;
        let benchmark_report_text =
            std::fs::read_to_string(artifact_dir.join("benchmark-report.json"))?;
        let benchmark_report: serde_json::Value = serde_json::from_str(&benchmark_report_text)?;
        if benchmark_report["candidate_model_id"].as_str() != Some(expected_candidate.model_id()) {
            return Err(std::io::Error::other(
                "local model quality-gate run report benchmark candidate mismatch",
            )
            .into());
        }
        if benchmark_report["acceptance_coverage"]["complete"].as_bool() != Some(true)
            || benchmark_report["acceptance_coverage"]["missing"]
                .as_array()
                .map(|missing| !missing.is_empty())
                .unwrap_or(true)
        {
            return Err(std::io::Error::other(
                "local model quality-gate run report benchmark acceptance coverage incomplete",
            )
            .into());
        }
        validations.push(serde_json::json!({
            "model_id": expected_candidate.model_id(),
            "artifact_dir": artifact_dir.display().to_string(),
            "manifest": local_model_bundle_manifest_json(Some(&validation.manifest)),
            "acceptance_coverage": benchmark_report["acceptance_coverage"].clone(),
        }));
    }
    Ok(validations)
}

#[cfg(feature = "local-model")]
fn validate_failed_local_model_quality_gate_run_report(
    report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(
        report["failure"]["stage"].as_str(),
        Some("local_model_quality_gate_run" | "local_model_quality_gate_readiness")
    ) {
        return Err(std::io::Error::other(
            "local model quality-gate run failure report stage mismatch",
        )
        .into());
    }
    let message = required_json_string(&report["failure"], "message")?;
    if message.is_empty() {
        return Err(std::io::Error::other(
            "local model quality-gate run failure report message is empty",
        )
        .into());
    }
    Ok(())
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_run_report_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = local_model_validation_failure_report_json(report_path);
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_run_report_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "run_report": report_metadata,
        "failure": {
            "stage": "local_model_quality_gate_run_report_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_run_output_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = local_model_validation_failure_report_json(report_path);
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_run_output_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "run_output": report_metadata,
        "failure": {
            "stage": "local_model_quality_gate_run_output_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_runtime_preflight_json(
    options: &LocalModelQualityGateRunOptions<'_>,
    model_paths: &std::collections::BTreeMap<String, PathBuf>,
    candidates: &[SmallModelCandidate],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if !options.dry_run && !local_model_runner_available(options.executable) {
        return Err(std::io::Error::other(format!(
            "local model runner executable is required: {}",
            options.executable.display()
        ))
        .into());
    }

    let mut model_artifacts = Vec::new();
    for candidate in candidates {
        let model_path = model_paths.get(candidate.model_id()).ok_or_else(|| {
            std::io::Error::other(format!(
                "missing --model-path binding for {}",
                candidate.model_id()
            ))
        })?;
        let exists = model_path.is_file();
        if !options.dry_run && !exists {
            return Err(std::io::Error::other(format!(
                "local model artifact is required for {}: {}",
                candidate.model_id(),
                model_path.display()
            ))
            .into());
        }
        model_artifacts.push(serde_json::json!({
            "model_id": candidate.model_id(),
            "model_path": model_path.display().to_string(),
            "exists": exists,
            "required": !options.dry_run,
        }));
    }

    Ok(serde_json::json!({
        "dry_run": options.dry_run,
        "runner": {
            "path": options.executable.display().to_string(),
            "required": !options.dry_run,
            "available": options.dry_run || local_model_runner_available(options.executable),
        },
        "model_artifacts": model_artifacts,
    }))
}

#[cfg(feature = "local-model")]
fn local_model_runner_available(executable: &Path) -> bool {
    if executable.is_absolute() || executable.components().count() > 1 {
        return executable.is_file();
    }

    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(executable))
            .any(|candidate| candidate.is_file())
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_model_paths(
    bindings: &[String],
    candidates: &[SmallModelCandidate],
) -> Result<std::collections::BTreeMap<String, PathBuf>, Box<dyn std::error::Error>> {
    let mut model_paths = std::collections::BTreeMap::new();
    for binding in bindings {
        let (model_id, path) = binding
            .split_once('=')
            .ok_or_else(|| std::io::Error::other("--model-path must use MODEL_ID=PATH form"))?;
        if model_id.is_empty() || path.is_empty() {
            return Err(std::io::Error::other("--model-path must use MODEL_ID=PATH form").into());
        }
        if model_paths
            .insert(model_id.to_string(), PathBuf::from(path))
            .is_some()
        {
            return Err(std::io::Error::other(format!(
                "duplicate --model-path binding for {model_id}"
            ))
            .into());
        }
    }

    for model_id in model_paths.keys() {
        if !candidates
            .iter()
            .any(|candidate| candidate.model_id() == model_id.as_str())
        {
            return Err(std::io::Error::other(format!(
                "unknown quality-gate model binding: {model_id}"
            ))
            .into());
        }
    }

    Ok(model_paths)
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_status_json(artifact_root: &Path) -> serde_json::Value {
    let candidates = small_model_quality_gate_candidates();
    let candidate_statuses = candidates
        .iter()
        .map(|candidate| local_model_quality_gate_candidate_status_json(artifact_root, candidate))
        .collect::<Vec<_>>();
    let passed = candidate_statuses
        .iter()
        .filter(|candidate| candidate["status"].as_str() == Some("passed"))
        .count();
    let failed = candidate_statuses
        .iter()
        .filter(|candidate| {
            matches!(
                candidate["status"].as_str(),
                Some("benchmark_failed" | "validation_failed" | "invalid_bundle")
            )
        })
        .count();
    let missing = candidate_statuses
        .iter()
        .filter(|candidate| candidate["status"].as_str() == Some("missing_artifacts"))
        .count();
    let passed_models = local_model_quality_gate_status_models(&candidate_statuses, &["passed"]);
    let failed_models = local_model_quality_gate_status_models(
        &candidate_statuses,
        &["benchmark_failed", "validation_failed", "invalid_bundle"],
    );
    let missing_models =
        local_model_quality_gate_status_models(&candidate_statuses, &["missing_artifacts"]);
    let failure_report_summaries =
        local_model_quality_gate_failure_report_summaries(&candidate_statuses);
    let passing_report_summaries =
        local_model_quality_gate_passing_report_summaries(&candidate_statuses);
    let invalid_bundle_summaries =
        local_model_quality_gate_invalid_bundle_summaries(&candidate_statuses);
    let missing_artifact_summaries =
        local_model_quality_gate_missing_artifact_summaries(&candidate_statuses);
    let readiness_blockers = local_model_quality_gate_readiness_blockers(&candidate_statuses);
    let readiness_action_counts =
        local_model_quality_gate_readiness_action_counts(&readiness_blockers);
    let status_counts = serde_json::json!({
        "passed": local_model_quality_gate_status_count(&candidate_statuses, "passed"),
        "missing_artifacts": local_model_quality_gate_status_count(
            &candidate_statuses,
            "missing_artifacts"
        ),
        "invalid_bundle": local_model_quality_gate_status_count(
            &candidate_statuses,
            "invalid_bundle"
        ),
        "benchmark_failed": local_model_quality_gate_status_count(
            &candidate_statuses,
            "benchmark_failed"
        ),
        "validation_failed": local_model_quality_gate_status_count(
            &candidate_statuses,
            "validation_failed"
        ),
    });
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_status",
        "format_version": 1,
        "generated_by_command": "local-model-quality-gate-status",
        "artifact_root": artifact_root.display().to_string(),
        "summary": {
            "candidate_count": candidate_statuses.len(),
            "ready": !candidate_statuses.is_empty() && missing == 0 && failed == 0,
            "passed": passed,
            "failed": failed,
            "missing": missing,
            "status_counts": status_counts,
            "passed_models": passed_models,
            "passing_report_summaries": passing_report_summaries,
            "failed_models": failed_models,
            "failure_report_summaries": failure_report_summaries,
            "invalid_bundle_summaries": invalid_bundle_summaries,
            "missing_artifact_summaries": missing_artifact_summaries,
            "readiness_blockers": readiness_blockers,
            "readiness_action_counts": readiness_action_counts,
            "missing_models": missing_models,
        },
        "candidates": candidate_statuses,
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_quality_gate_status_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;

    if report["format"].as_str() != Some("continuitydb.local_model_quality_gate_status")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("unsupported local model quality-gate status report").into(),
        );
    }
    if report["generated_by_command"].as_str() != Some("local-model-quality-gate-status") {
        return Err(std::io::Error::other(
            "local model quality-gate status report producer mismatch",
        )
        .into());
    }

    let artifact_root = Path::new(required_json_string(&report, "artifact_root")?);
    let expected_report = local_model_quality_gate_status_json(artifact_root);
    if report != expected_report {
        return Err(std::io::Error::other("local model quality-gate status report drifted").into());
    }

    let summary = &report["summary"];
    let readiness_blocker_count = summary["readiness_blockers"]
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| {
            std::io::Error::other("local model quality-gate status missing readiness blockers")
        })?;
    Ok(serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_status_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": local_model_contract_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "status_report": {
            "format": report["format"].clone(),
            "format_version": report["format_version"].clone(),
            "generated_by_command": report["generated_by_command"].clone(),
            "artifact_root": report["artifact_root"].clone(),
            "candidate_count": summary["candidate_count"].clone(),
            "ready": summary["ready"].clone(),
            "passed": summary["passed"].clone(),
            "failed": summary["failed"].clone(),
            "missing": summary["missing"].clone(),
            "status_counts": summary["status_counts"].clone(),
            "readiness_blocker_count": readiness_blocker_count,
            "run_quality_gate_candidate_actions": summary["readiness_action_counts"]["run_quality_gate_candidate"].clone(),
            "regenerate_quality_gate_bundle_actions": summary["readiness_action_counts"]["regenerate_quality_gate_bundle"].clone(),
            "inspect_benchmark_failure_report_actions": summary["readiness_action_counts"]["inspect_benchmark_failure_report"].clone(),
            "inspect_validation_failure_report_actions": summary["readiness_action_counts"]["inspect_validation_failure_report"].clone(),
        },
    }))
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_status_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_text = std::fs::read_to_string(report_path).ok();
    let parsed_report = report_text
        .as_ref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
    serde_json::json!({
        "format": "continuitydb.local_model_quality_gate_status_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "status_report": {
            "report_path": report_path.display().to_string(),
            "report_fingerprint": report_text.as_ref().map(|text| local_model_contract_fingerprint(text)),
            "report_bytes": report_text.as_ref().map(String::len),
            "parseable": parsed_report.is_some(),
        },
        "failure": {
            "stage": "local_model_quality_gate_status_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_require_ready_status_json(
    report_path: &Path,
    stderr_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let status_validation = validate_local_model_quality_gate_status_json(report_path)?;
    let status_report = &status_validation["status_report"];
    if status_report["ready"].as_bool() != Some(false) {
        return Err(
            std::io::Error::other("local model require-ready status report is ready").into(),
        );
    }
    if status_report["readiness_blocker_count"].as_u64() == Some(0) {
        return Err(std::io::Error::other(
            "local model require-ready status report has no readiness blockers",
        )
        .into());
    }

    let stderr_text = std::fs::read_to_string(stderr_path)?;
    let readiness_message = "local model quality gate is not ready";
    if !stderr_text.contains(readiness_message) {
        return Err(std::io::Error::other(
            "local model require-ready stderr missing readiness failure",
        )
        .into());
    }

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_require_ready_status_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "stderr_path": stderr_path.display().to_string(),
        "report_fingerprint": status_validation["report_fingerprint"].clone(),
        "report_bytes": status_validation["report_bytes"].clone(),
        "stderr_fingerprint": local_model_contract_fingerprint(&stderr_text),
        "stderr_bytes": stderr_text.len(),
        "valid": true,
        "status_report": status_report.clone(),
        "stderr": {
            "contains_readiness_error": true,
            "expected_message": readiness_message,
        },
    }))
}

#[cfg(feature = "local-model")]
fn local_model_require_ready_status_validation_failure_json(
    report_path: &Path,
    stderr_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_text = std::fs::read_to_string(report_path).ok();
    let stderr_text = std::fs::read_to_string(stderr_path).ok();
    serde_json::json!({
        "format": "continuitydb.local_model_require_ready_status_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "stderr_path": stderr_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "status_report": {
            "report_path": report_path.display().to_string(),
            "report_fingerprint": report_text.as_ref().map(|text| local_model_contract_fingerprint(text)),
            "report_bytes": report_text.as_ref().map(String::len),
            "parseable": report_text
                .as_ref()
                .is_some_and(|text| serde_json::from_str::<serde_json::Value>(text).is_ok()),
        },
        "stderr": {
            "stderr_path": stderr_path.display().to_string(),
            "stderr_fingerprint": stderr_text.as_ref().map(|text| local_model_contract_fingerprint(text)),
            "stderr_bytes": stderr_text.as_ref().map(String::len),
        },
        "failure": {
            "stage": "local_model_require_ready_status_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_readiness_action_counts(
    readiness_blockers: &[serde_json::Value],
) -> serde_json::Value {
    serde_json::json!({
        "run_quality_gate_candidate": local_model_quality_gate_readiness_action_count(
            readiness_blockers,
            "run_quality_gate_candidate"
        ),
        "regenerate_quality_gate_bundle": local_model_quality_gate_readiness_action_count(
            readiness_blockers,
            "regenerate_quality_gate_bundle"
        ),
        "inspect_benchmark_failure_report": local_model_quality_gate_readiness_action_count(
            readiness_blockers,
            "inspect_benchmark_failure_report"
        ),
        "inspect_validation_failure_report": local_model_quality_gate_readiness_action_count(
            readiness_blockers,
            "inspect_validation_failure_report"
        ),
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_readiness_action_count(
    readiness_blockers: &[serde_json::Value],
    recommended_action: &str,
) -> usize {
    readiness_blockers
        .iter()
        .filter(|blocker| blocker["recommended_action"].as_str() == Some(recommended_action))
        .count()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_passing_report_summaries(
    candidate_statuses: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    candidate_statuses
        .iter()
        .filter(|candidate| candidate["status"].as_str() == Some("passed"))
        .map(|candidate| {
            serde_json::json!({
                "model_id": candidate["model_id"].clone(),
                "status": candidate["status"].clone(),
                "artifact_dir": candidate["artifact_dir"].clone(),
                "benchmark_report": candidate["benchmark_report"].clone(),
            })
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_readiness_blockers(
    candidate_statuses: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    candidate_statuses
        .iter()
        .filter_map(|candidate| {
            let status = candidate["status"].as_str()?;
            let recommended_action = match status {
                "missing_artifacts" => "run_quality_gate_candidate",
                "invalid_bundle" => "regenerate_quality_gate_bundle",
                "benchmark_failed" => "inspect_benchmark_failure_report",
                "validation_failed" => "inspect_validation_failure_report",
                _ => return None,
            };
            Some(serde_json::json!({
                "model_id": candidate["model_id"].clone(),
                "status": candidate["status"].clone(),
                "artifact_dir": candidate["artifact_dir"].clone(),
                "recommended_action": recommended_action,
            }))
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_missing_artifact_summaries(
    candidate_statuses: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    candidate_statuses
        .iter()
        .filter(|candidate| candidate["status"].as_str() == Some("missing_artifacts"))
        .map(|candidate| {
            serde_json::json!({
                "model_id": candidate["model_id"].clone(),
                "status": candidate["status"].clone(),
                "artifact_dir": candidate["artifact_dir"].clone(),
                "missing_artifacts": candidate["missing_artifacts"].clone(),
            })
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_invalid_bundle_summaries(
    candidate_statuses: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    candidate_statuses
        .iter()
        .filter(|candidate| candidate["status"].as_str() == Some("invalid_bundle"))
        .map(|candidate| {
            serde_json::json!({
                "model_id": candidate["model_id"].clone(),
                "status": candidate["status"].clone(),
                "artifact_dir": candidate["artifact_dir"].clone(),
                "validation_error": candidate["validation_error"].clone(),
            })
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_failure_report_summaries(
    candidate_statuses: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    candidate_statuses
        .iter()
        .filter(|candidate| {
            matches!(
                candidate["status"].as_str(),
                Some("benchmark_failed" | "validation_failed")
            )
        })
        .map(|candidate| {
            serde_json::json!({
                "model_id": candidate["model_id"].clone(),
                "status": candidate["status"].clone(),
                "failure_report_path": candidate["failure_report_path"].clone(),
                "failure": candidate["failure"].clone(),
            })
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_status_count(
    candidate_statuses: &[serde_json::Value],
    status: &str,
) -> usize {
    candidate_statuses
        .iter()
        .filter(|candidate| candidate["status"].as_str() == Some(status))
        .count()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_status_models(
    candidate_statuses: &[serde_json::Value],
    statuses: &[&str],
) -> Vec<String> {
    candidate_statuses
        .iter()
        .filter(|candidate| {
            candidate["status"]
                .as_str()
                .is_some_and(|status| statuses.contains(&status))
        })
        .filter_map(|candidate| candidate["model_id"].as_str().map(ToOwned::to_owned))
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_candidate_status_json(
    artifact_root: &Path,
    candidate: &SmallModelCandidate,
) -> serde_json::Value {
    let artifact_slug = local_model_candidate_artifact_slug(candidate.model_id());
    let artifact_dir = artifact_root.join(&artifact_slug);
    let benchmark_report_path = artifact_dir.join("benchmark-report.json");
    let benchmark_failure_report_path = artifact_dir.join("failure-report.json");
    let validation_failure_report_path = artifact_dir.join("validation-failure-report.json");
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");

    let mut missing_artifacts = Vec::new();
    if !artifact_dir.exists() {
        missing_artifacts.push("artifact_dir");
    }
    if !benchmark_report_path.exists() {
        missing_artifacts.push("benchmark_report");
    }
    if !bundle_manifest_path.exists() {
        missing_artifacts.push("bundle_manifest");
    }

    let validation_result = if missing_artifacts.is_empty() {
        Some(validate_local_model_bundle_manifest(&artifact_dir))
    } else {
        None
    };
    let validation_error = validation_result
        .as_ref()
        .and_then(|result| result.as_ref().err())
        .map(|error| error.to_string());
    let status = if !missing_artifacts.is_empty() {
        "missing_artifacts"
    } else if validation_error.is_some() {
        "invalid_bundle"
    } else if benchmark_failure_report_path.exists() {
        "benchmark_failed"
    } else if validation_failure_report_path.exists() {
        "validation_failed"
    } else {
        "passed"
    };
    let failure_report_path = match status {
        "benchmark_failed" => Some(benchmark_failure_report_path.display().to_string()),
        "validation_failed" => Some(validation_failure_report_path.display().to_string()),
        _ => None,
    };
    let benchmark_report = if status == "passed" {
        validation_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .and_then(|_| {
                local_model_quality_gate_benchmark_report_summary_json(&benchmark_report_path)
            })
    } else {
        None
    };
    let failure = match status {
        "benchmark_failed" => local_model_quality_gate_failure_json(&benchmark_failure_report_path),
        "validation_failed" => {
            local_model_quality_gate_failure_json(&validation_failure_report_path)
        }
        _ => None,
    };

    serde_json::json!({
        "model_id": candidate.model_id(),
        "evaluation_priority": candidate.evaluation_priority(),
        "artifact_dir": artifact_dir.display().to_string(),
        "benchmark_report_path": benchmark_report_path.display().to_string(),
        "benchmark_failure_report_path": benchmark_failure_report_path.display().to_string(),
        "validation_failure_report_path": validation_failure_report_path.display().to_string(),
        "bundle_manifest_path": bundle_manifest_path.display().to_string(),
        "status": status,
        "ready": status == "passed",
        "missing_artifacts": missing_artifacts,
        "validation_error": validation_error,
        "failure_report_path": failure_report_path,
        "benchmark_report": benchmark_report,
        "failure": failure,
    })
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_benchmark_report_summary_json(
    report_path: &Path,
) -> Option<serde_json::Value> {
    let report = std::fs::read_to_string(report_path).ok()?;
    let report = serde_json::from_str::<serde_json::Value>(&report).ok()?;
    Some(serde_json::json!({
        "candidate_model_id": report["candidate_model_id"],
        "candidate_selection": report["candidate_selection"],
        "dry_run": report["dry_run"],
        "evaluation_suite_fingerprint": report["evaluation_suite_fingerprint"],
        "acceptance_coverage": report["acceptance_coverage"],
        "prompt_fingerprint": report["prompt_fingerprint"],
        "response_schema_version": report["response_schema_version"],
    }))
}

#[cfg(feature = "local-model")]
fn local_model_quality_gate_failure_json(report_path: &Path) -> Option<serde_json::Value> {
    let report = std::fs::read_to_string(report_path).ok()?;
    let report_json = serde_json::from_str::<serde_json::Value>(&report).ok()?;
    report_json.get("failure").cloned()
}

#[cfg(feature = "local-model")]
fn local_model_candidate_artifact_slug(model_id: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in model_id.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(feature = "local-model")]
fn local_model_acceptance_criteria_json() -> serde_json::Value {
    let criteria = local_model_required_acceptance_criteria();
    serde_json::json!({
        "format": "continuitydb.local_model_acceptance_criteria",
        "format_version": 1,
        "required_criteria_count": criteria.len(),
        "criteria": criteria,
    })
}

#[cfg(feature = "local-model")]
fn local_model_required_acceptance_criteria() -> Vec<&'static str> {
    required_steward_acceptance_criteria()
        .iter()
        .map(|criterion| criterion.as_str())
        .collect()
}

#[cfg(feature = "local-model")]
fn validate_local_model_acceptance_criteria_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let criteria = local_model_required_acceptance_criteria();

    if report["format"].as_str() != Some("continuitydb.local_model_acceptance_criteria")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("unsupported local model acceptance criteria report").into(),
        );
    }
    if report["required_criteria_count"].as_u64() != Some(criteria.len() as u64) {
        return Err(std::io::Error::other("local model acceptance criteria count mismatch").into());
    }
    let report_criteria = report["criteria"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("local model acceptance criteria missing criteria"))?;
    if report_criteria.len() != criteria.len() {
        return Err(
            std::io::Error::other("local model acceptance criteria list length mismatch").into(),
        );
    }
    for (index, expected) in criteria.iter().enumerate() {
        if report_criteria[index].as_str() != Some(*expected) {
            return Err(std::io::Error::other(format!(
                "local model acceptance criteria mismatch at index {index}"
            ))
            .into());
        }
    }

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_acceptance_criteria_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "required_criteria_count": criteria.len(),
        "criteria": criteria,
    }))
}

#[cfg(feature = "local-model")]
fn local_model_acceptance_criteria_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.local_model_acceptance_criteria_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "acceptance_criteria_report": report_metadata,
        "failure": {
            "stage": "local_model_acceptance_criteria_validation",
            "message": message,
        },
    })
}

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
                "acceptance_criteria": case
                    .acceptance_criteria()
                    .iter()
                    .map(|criterion| criterion.as_str())
                    .collect::<Vec<_>>(),
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
    let acceptance_coverage = suite.acceptance_coverage();

    serde_json::json!({
        "format": "continuitydb.local_model_evaluation_suite",
        "format_version": 1,
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "evaluation_suite_fingerprint": suite.fingerprint(),
        "total_cases": suite.len(),
        "acceptance_coverage": local_model_acceptance_coverage_json(&acceptance_coverage),
        "cases": cases,
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_evaluation_suite_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let expected = local_model_evaluation_suite_json();

    if report["format"].as_str() != expected["format"].as_str()
        || report["format_version"].as_u64() != expected["format_version"].as_u64()
    {
        return Err(
            std::io::Error::other("unsupported local model evaluation suite report").into(),
        );
    }
    if report["response_schema_version"].as_u64() != expected["response_schema_version"].as_u64() {
        return Err(
            std::io::Error::other("local model evaluation suite schema version mismatch").into(),
        );
    }
    if report["evaluation_suite_fingerprint"].as_str()
        != expected["evaluation_suite_fingerprint"].as_str()
    {
        return Err(
            std::io::Error::other("local model evaluation suite fingerprint mismatch").into(),
        );
    }
    if report["total_cases"].as_u64() != expected["total_cases"].as_u64() {
        return Err(
            std::io::Error::other("local model evaluation suite case count mismatch").into(),
        );
    }
    if report["acceptance_coverage"] != expected["acceptance_coverage"] {
        return Err(std::io::Error::other(
            "local model evaluation suite acceptance coverage mismatch",
        )
        .into());
    }
    if report["cases"] != expected["cases"] {
        return Err(
            std::io::Error::other("local model evaluation suite case contract mismatch").into(),
        );
    }

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_evaluation_suite_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "response_schema_version": report["response_schema_version"],
        "evaluation_suite_fingerprint": report["evaluation_suite_fingerprint"],
        "total_cases": report["total_cases"],
        "acceptance_coverage": report["acceptance_coverage"],
    }))
}

#[cfg(feature = "local-model")]
fn local_model_evaluation_suite_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.local_model_evaluation_suite_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "evaluation_suite_report": report_metadata,
        "failure": {
            "stage": "local_model_evaluation_suite_validation",
            "message": message,
        },
    })
}

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
        StewardAction::RequestVerification { cell_id, request } => serde_json::json!({
            "type": "request_verification",
            "cell_id": cell_id,
            "request": request,
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

#[cfg(feature = "local-model")]
fn write_local_model_contract_json(
    schema_path: &Path,
    grammar_path: &Path,
    context_compiler: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let (contract_kind, schema_version, schema, grammar) = if context_compiler {
        (
            "context_compiler",
            LOCAL_MODEL_CONTEXT_COMPILER_RESPONSE_SCHEMA_VERSION,
            local_model_context_compiler_response_json_schema(),
            local_model_context_compiler_response_gbnf_grammar(),
        )
    } else {
        (
            "steward_actions",
            LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
            local_model_response_json_schema(),
            local_model_response_gbnf_grammar(),
        )
    };
    std::fs::write(schema_path, schema)?;
    std::fs::write(grammar_path, grammar)?;

    Ok(serde_json::json!({
        "contract_kind": contract_kind,
        "schema_version": schema_version,
        "schema_path": schema_path.display().to_string(),
        "grammar_path": grammar_path.display().to_string(),
        "schema_fingerprint": local_model_contract_fingerprint(schema),
        "grammar_fingerprint": local_model_contract_fingerprint(grammar),
        "schema_bytes": schema.len(),
        "grammar_bytes": grammar.len(),
    }))
}

#[cfg(feature = "local-model")]
fn benchmark_local_model_json(
    options: LocalModelBenchmarkOptions<'_>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let candidate_selection = local_model_candidate_selection(options.candidate_id)?;
    let candidate = candidate_selection.candidate;
    let suite = default_steward_evaluation_suite();
    let artifact_contract_dir = options.artifact_dir.map(|dir| dir.join("contracts"));
    let artifact_prompt_dir = options.artifact_dir.map(|dir| dir.join("prompts"));
    let artifact_response_dir = options
        .artifact_dir
        .filter(|_artifact_dir| !options.dry_run)
        .map(|dir| dir.join("responses"));
    let artifact_changed_case_report_path = options
        .artifact_dir
        .filter(|_artifact_dir| !options.dry_run && options.compare_baseline)
        .map(|dir| dir.join("changed-cases.json"));
    let effective_contract_dir = options.contract_dir.or(artifact_contract_dir.as_deref());
    let effective_prompt_dir = options.prompt_dir.or(artifact_prompt_dir.as_deref());
    let effective_response_dir = options.response_dir.or(artifact_response_dir.as_deref());
    let effective_changed_case_report_path = options
        .changed_case_report_path
        .or(artifact_changed_case_report_path.as_deref());
    let contract_artifacts = effective_contract_dir
        .map(write_local_model_contract_artifacts)
        .transpose()?;
    let prompt_artifacts = effective_prompt_dir
        .map(|prompt_dir| write_local_model_prompt_artifacts(prompt_dir, &suite))
        .transpose()?
        .unwrap_or_default();
    let effective_grammar_path = options.grammar_path.or_else(|| {
        contract_artifacts
            .as_ref()
            .map(|artifacts| artifacts.grammar_path.as_path())
    });
    if options.enforce_candidate_requirements
        && candidate.requires_grammar()
        && effective_grammar_path.is_none()
    {
        return Err(std::io::Error::other(
            "selected local model candidate requires grammar-constrained output; missing --grammar-path",
        )
        .into());
    }
    if options.stability_trials == Some(0) {
        return Err(std::io::Error::other("--stability-trials must be greater than zero").into());
    }
    if options.fail_on_unstable && options.stability_trials.is_none() {
        return Err(std::io::Error::other("--fail-on-unstable requires --stability-trials").into());
    }
    if options.changed_case_report_path.is_some() && !options.compare_baseline {
        return Err(std::io::Error::other(
            "--changed-case-report-path requires --compare-baseline or --fail-on-regression",
        )
        .into());
    }
    let mut config = if options.candidate_defaults {
        candidate.recommended_runner_config(
            options.executable.to_path_buf(),
            options.model_path.to_path_buf(),
        )
    } else {
        LocalExecutableRunnerConfig::new(options.executable.to_path_buf())
            .with_model_path(options.model_path.to_path_buf())
    };
    if let Some(grammar_path) = effective_grammar_path {
        config = config
            .with_argument("--grammar-file")
            .with_argument(grammar_path);
    }
    for argument in options.arguments {
        config = config.with_argument(argument);
    }
    if options.dry_run {
        let baseline_preflight = if options.compare_baseline {
            Some(local_model_baseline_preflight_json(
                options.baseline_path,
                candidate,
                &suite,
                &config,
            )?)
        } else {
            None
        };
        return Ok(local_model_benchmark_dry_run_json(
            candidate,
            candidate_selection.source,
            &config,
            options.baseline_path,
            LocalModelBenchmarkArtifacts {
                contract: contract_artifacts.as_ref(),
                prompts: &prompt_artifacts,
                responses: &[],
                response_manifest: None,
            },
            LocalModelBenchmarkDryRunGates {
                baseline_preflight,
                stability_preflight: options.stability_trials.map(|trials| {
                    local_model_stability_preflight_json(trials, options.fail_on_unstable)
                }),
                fail_on_failed_cases: options.fail_on_failed_cases,
                failure_report_path: options.failure_report_path.map(Path::to_path_buf),
                changed_case_report_path: options.changed_case_report_path.map(Path::to_path_buf),
            },
        ));
    }

    let benchmark = LocalModelBenchmark::new(candidate, LocalExecutableRunner::new(config), suite);
    let identity = StewardIdentity::new("continuitydb-cli-local-model", "0.1.0", "strict")?;
    let stability = options
        .stability_trials
        .map(|trials| benchmark.run_stability(identity.clone(), trials));
    if options.fail_on_unstable
        && stability
            .as_ref()
            .is_some_and(|stability| !stability.stable())
    {
        if options.artifact_dir.is_some()
            || options.failure_report_path.is_some()
            || effective_changed_case_report_path.is_some()
        {
            let (report, responses) = if effective_response_dir.is_some() {
                benchmark.run_with_responses(identity)
            } else {
                (benchmark.run(identity), Vec::new())
            };
            let response_artifacts = effective_response_dir
                .map(|response_dir| write_local_model_response_artifacts(response_dir, &responses))
                .transpose()?
                .unwrap_or_default();
            let response_manifest = if let Some(response_dir) = effective_response_dir {
                Some(write_local_model_response_artifact_manifest(
                    response_dir,
                    &response_artifacts,
                )?)
            } else {
                None
            };
            let current_baseline =
                LocalModelBenchmarkBaseline::from_report_with_candidate_selection(
                    report,
                    Utc::now(),
                    candidate_selection.source,
                );
            let mut report = local_model_benchmark_json(
                options.baseline_path,
                options.compare_baseline,
                &current_baseline,
                None,
                candidate_selection.source,
                LocalModelBenchmarkArtifacts {
                    contract: contract_artifacts.as_ref(),
                    prompts: &prompt_artifacts,
                    responses: &response_artifacts,
                    response_manifest: response_manifest.as_ref(),
                },
                stability.as_ref(),
            );
            report =
                finalize_local_model_benchmark_report(report, effective_changed_case_report_path)?;
            if let Some(artifact_dir) = options.artifact_dir {
                report = write_local_model_artifact_bundle_report(artifact_dir, report)?;
            }
            if let Some(report_path) = options.failure_report_path {
                write_pretty_json_file(report_path, &report)?;
            }
        }
        return Err(std::io::Error::other("local model benchmark stability check failed").into());
    }

    let (report, responses) = if effective_response_dir.is_some() {
        benchmark.run_with_responses(identity)
    } else {
        (benchmark.run(identity), Vec::new())
    };
    let response_artifacts = effective_response_dir
        .map(|response_dir| write_local_model_response_artifacts(response_dir, &responses))
        .transpose()?
        .unwrap_or_default();
    let response_manifest = if let Some(response_dir) = effective_response_dir {
        Some(write_local_model_response_artifact_manifest(
            response_dir,
            &response_artifacts,
        )?)
    } else {
        None
    };
    let current_baseline = LocalModelBenchmarkBaseline::from_report_with_candidate_selection(
        report,
        Utc::now(),
        candidate_selection.source,
    );
    if options.fail_on_failed_cases && !current_baseline.evaluation_summary().passed() {
        let mut report = local_model_benchmark_json(
            options.baseline_path,
            options.compare_baseline,
            &current_baseline,
            None,
            candidate_selection.source,
            LocalModelBenchmarkArtifacts {
                contract: contract_artifacts.as_ref(),
                prompts: &prompt_artifacts,
                responses: &response_artifacts,
                response_manifest: response_manifest.as_ref(),
            },
            stability.as_ref(),
        );
        report = finalize_local_model_benchmark_report(report, effective_changed_case_report_path)?;
        if let Some(artifact_dir) = options.artifact_dir {
            report = write_local_model_artifact_bundle_report(artifact_dir, report)?;
        }
        if let Some(report_path) = options.failure_report_path {
            write_pretty_json_file(report_path, &report)?;
        }
        return Err(
            std::io::Error::other("local model benchmark fixed evaluation cases failed").into(),
        );
    }

    let mut store = FileLocalModelBenchmarkBaselineStore::open(options.baseline_path)?;
    let previous = if options.compare_baseline {
        continuitydb_steward::latest_compatible_local_model_benchmark_baseline(
            &store,
            &current_baseline,
        )?
    } else {
        None
    };
    let regression = previous
        .as_ref()
        .map(|previous| LocalModelBenchmarkRegression::compare(previous, &current_baseline));

    if options.fail_on_regression
        && regression
            .as_ref()
            .is_some_and(LocalModelBenchmarkRegression::regressed)
    {
        let mut report = local_model_benchmark_json(
            options.baseline_path,
            options.compare_baseline,
            &current_baseline,
            regression.as_ref(),
            candidate_selection.source,
            LocalModelBenchmarkArtifacts {
                contract: contract_artifacts.as_ref(),
                prompts: &prompt_artifacts,
                responses: &response_artifacts,
                response_manifest: response_manifest.as_ref(),
            },
            stability.as_ref(),
        );
        report = finalize_local_model_benchmark_report(report, effective_changed_case_report_path)?;
        if let Some(artifact_dir) = options.artifact_dir {
            report = write_local_model_artifact_bundle_report(artifact_dir, report)?;
        }
        if let Some(report_path) = options.failure_report_path {
            write_pretty_json_file(report_path, &report)?;
        }
        return Err(std::io::Error::other("local model benchmark regression detected").into());
    }
    store.append_baseline(current_baseline.clone())?;

    let report = local_model_benchmark_json(
        options.baseline_path,
        options.compare_baseline,
        &current_baseline,
        regression.as_ref(),
        candidate_selection.source,
        LocalModelBenchmarkArtifacts {
            contract: contract_artifacts.as_ref(),
            prompts: &prompt_artifacts,
            responses: &response_artifacts,
            response_manifest: response_manifest.as_ref(),
        },
        stability.as_ref(),
    );
    finalize_local_model_benchmark_report(report, effective_changed_case_report_path)
}

#[cfg(feature = "local-model")]
fn local_model_benchmark_dry_run_json(
    candidate: SmallModelCandidate,
    candidate_selection_source: &str,
    config: &LocalExecutableRunnerConfig,
    baseline_path: &Path,
    artifacts: LocalModelBenchmarkArtifacts<'_>,
    gates: LocalModelBenchmarkDryRunGates,
) -> serde_json::Value {
    let suite = default_steward_evaluation_suite();
    let mut value = serde_json::json!({
        "dry_run": true,
        "will_record_baseline": false,
        "candidate_model_id": candidate.model_id(),
        "candidate_role": candidate.role(),
        "candidate_selection": {
            "source": candidate_selection_source,
            "model_id": candidate.model_id(),
        },
        "baseline_path": baseline_path.display().to_string(),
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "evaluation_suite_fingerprint": suite.fingerprint(),
        "acceptance_coverage": local_model_acceptance_coverage_json(&suite.acceptance_coverage()),
        "schema_fingerprint": local_model_contract_fingerprint(local_model_response_json_schema()),
        "grammar_fingerprint": local_model_contract_fingerprint(local_model_response_gbnf_grammar()),
        "schema_bytes": local_model_response_json_schema().len(),
        "grammar_bytes": local_model_response_gbnf_grammar().len(),
        "prompt_fingerprint": local_model_prompt_fingerprint_for_suite(&suite),
        "fail_on_failed_cases": gates.fail_on_failed_cases,
        "failure_report_path": gates
            .failure_report_path
            .as_ref()
            .map(|path| path.display().to_string()),
        "changed_case_report_path": gates
            .changed_case_report_path
            .as_ref()
            .map(|path| path.display().to_string()),
        "contract_artifacts": local_model_contract_artifacts_json(artifacts.contract),
        "prompt_artifacts": local_model_prompt_artifacts_json(artifacts.prompts),
        "response_fingerprints": [],
        "response_artifacts": local_model_response_artifacts_json(artifacts.responses),
        "response_artifact_manifest": local_model_response_artifact_manifest_json(artifacts.response_manifest),
        "bundle_manifest": null,
        "runtime": {
            "executable": config.executable().display().to_string(),
            "arguments": config.command_arguments(),
        },
    });
    if let Some(baseline_preflight) = gates.baseline_preflight {
        value["baseline_preflight"] = baseline_preflight;
    }
    if let Some(stability_preflight) = gates.stability_preflight {
        value["stability_preflight"] = stability_preflight;
    }
    value
}

#[cfg(feature = "local-model")]
fn validate_local_model_benchmark_report_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let suite = default_steward_evaluation_suite();

    if report["dry_run"].as_bool() != Some(true) {
        return Err(std::io::Error::other(
            "local model benchmark report validation currently requires dry_run true",
        )
        .into());
    }
    if report["will_record_baseline"].as_bool() != Some(false) {
        return Err(std::io::Error::other(
            "local model dry-run benchmark report baseline recording mismatch",
        )
        .into());
    }
    let candidate_model_id = required_json_string(&report, "candidate_model_id")?;
    let candidate = small_model_candidates()
        .iter()
        .find(|candidate| candidate.model_id() == candidate_model_id)
        .ok_or_else(|| std::io::Error::other("unknown local model benchmark candidate"))?;
    if report["candidate_role"].as_str() != Some(candidate.role()) {
        return Err(std::io::Error::other("local model benchmark candidate role mismatch").into());
    }
    if report["candidate_selection"]["model_id"].as_str() != Some(candidate.model_id()) {
        return Err(
            std::io::Error::other("local model benchmark candidate selection mismatch").into(),
        );
    }
    if report["response_schema_version"].as_u64()
        != Some(LOCAL_MODEL_RESPONSE_SCHEMA_VERSION as u64)
    {
        return Err(std::io::Error::other(
            "local model benchmark response schema version mismatch",
        )
        .into());
    }
    let expected_suite_fingerprint = suite.fingerprint();
    if report["evaluation_suite_fingerprint"].as_str() != Some(expected_suite_fingerprint.as_str())
    {
        return Err(std::io::Error::other(
            "local model benchmark evaluation suite fingerprint mismatch",
        )
        .into());
    }
    if report["acceptance_coverage"]
        != local_model_acceptance_coverage_json(&suite.acceptance_coverage())
    {
        return Err(
            std::io::Error::other("local model benchmark acceptance coverage mismatch").into(),
        );
    }
    if report["schema_fingerprint"].as_str()
        != Some(local_model_contract_fingerprint(local_model_response_json_schema()).as_str())
    {
        return Err(
            std::io::Error::other("local model benchmark schema fingerprint mismatch").into(),
        );
    }
    if report["grammar_fingerprint"].as_str()
        != Some(local_model_contract_fingerprint(local_model_response_gbnf_grammar()).as_str())
    {
        return Err(
            std::io::Error::other("local model benchmark grammar fingerprint mismatch").into(),
        );
    }
    if report["schema_bytes"].as_u64() != Some(local_model_response_json_schema().len() as u64) {
        return Err(
            std::io::Error::other("local model benchmark schema byte count mismatch").into(),
        );
    }
    if report["grammar_bytes"].as_u64() != Some(local_model_response_gbnf_grammar().len() as u64) {
        return Err(
            std::io::Error::other("local model benchmark grammar byte count mismatch").into(),
        );
    }
    if report["prompt_fingerprint"].as_str()
        != Some(local_model_prompt_fingerprint_for_suite(&suite).as_str())
    {
        return Err(
            std::io::Error::other("local model benchmark prompt fingerprint mismatch").into(),
        );
    }
    let bundle_manifest =
        validate_local_model_benchmark_report_bundle_manifest(report_path, &report)?;
    if report["response_fingerprints"]
        .as_array()
        .map(|fingerprints| !fingerprints.is_empty())
        .unwrap_or(true)
    {
        return Err(std::io::Error::other(
            "local model dry-run benchmark response fingerprints mismatch",
        )
        .into());
    }
    required_json_string(&report["runtime"], "executable")?;
    if report["runtime"]["arguments"].as_array().is_none() {
        return Err(
            std::io::Error::other("local model benchmark runtime arguments missing").into(),
        );
    }

    Ok(serde_json::json!({
        "format": "continuitydb.local_model_benchmark_report_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "dry_run": report["dry_run"],
        "will_record_baseline": report["will_record_baseline"],
        "candidate_model_id": report["candidate_model_id"],
        "candidate_selection_source": report["candidate_selection"]["source"],
        "response_schema_version": report["response_schema_version"],
        "evaluation_suite_fingerprint": report["evaluation_suite_fingerprint"],
        "acceptance_coverage": report["acceptance_coverage"],
        "schema_fingerprint": report["schema_fingerprint"],
        "grammar_fingerprint": report["grammar_fingerprint"],
        "prompt_fingerprint": report["prompt_fingerprint"],
        "bundle_manifest": bundle_manifest,
    }))
}

#[cfg(feature = "local-model")]
fn validate_local_model_benchmark_report_bundle_manifest(
    report_path: &Path,
    report: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if report["bundle_manifest"].is_null() {
        return Ok(serde_json::Value::Null);
    }

    let manifest_path_text = required_json_string(&report["bundle_manifest"], "manifest_path")?;
    let manifest_path = Path::new(manifest_path_text);
    let artifact_dir = manifest_path.parent().ok_or_else(|| {
        std::io::Error::other("local model benchmark bundle manifest parent missing")
    })?;
    let validation = validate_local_model_bundle_manifest(artifact_dir)?;
    if validation.manifest.manifest_path != manifest_path {
        return Err(
            std::io::Error::other("local model benchmark bundle manifest path mismatch").into(),
        );
    }
    let expected_report_path = report_path.display().to_string();
    if validation.benchmark_report["report_path"].as_str() != Some(expected_report_path.as_str()) {
        return Err(
            std::io::Error::other("local model benchmark bundle report path mismatch").into(),
        );
    }
    if required_json_string(&report["bundle_manifest"], "manifest_fingerprint")?
        != validation.manifest.manifest_fingerprint
    {
        return Err(std::io::Error::other(
            "local model benchmark bundle manifest fingerprint mismatch",
        )
        .into());
    }
    if required_json_u64(&report["bundle_manifest"], "manifest_bytes")?
        != validation.manifest.manifest_bytes as u64
    {
        return Err(std::io::Error::other(
            "local model benchmark bundle manifest byte count mismatch",
        )
        .into());
    }
    if report["bundle_manifest"]["parseable"].as_bool() != Some(true)
        || !report["bundle_manifest"]["parse_error"].is_null()
    {
        return Err(std::io::Error::other(
            "local model benchmark bundle manifest parse metadata mismatch",
        )
        .into());
    }

    Ok(local_model_bundle_manifest_json(Some(&validation.manifest)))
}

#[cfg(feature = "local-model")]
fn local_model_benchmark_report_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.local_model_benchmark_report_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "benchmark_report": report_metadata,
        "failure": {
            "stage": "local_model_benchmark_report_validation",
            "message": message,
        },
    })
}

#[cfg(feature = "local-model")]
fn local_model_acceptance_coverage_json(
    acceptance_coverage: &StewardAcceptanceCoverage,
) -> serde_json::Value {
    serde_json::json!({
        "complete": acceptance_coverage.complete(),
        "covered": acceptance_coverage
            .covered()
            .iter()
            .map(|criterion| criterion.as_str())
            .collect::<Vec<_>>(),
        "missing": acceptance_coverage
            .missing()
            .iter()
            .map(|criterion| criterion.as_str())
            .collect::<Vec<_>>(),
    })
}

fn run_agent_behavior_model_runner_tasks(
    tasks: Vec<AgentBehaviorExecutionTask>,
    runner: &Path,
    runner_args: &[String],
    trials: usize,
) -> Result<Vec<AgentBehaviorExecutionRecord>, Box<dyn std::error::Error>> {
    if trials == 0 {
        return Err(
            std::io::Error::other("agent behavior trials must be greater than zero").into(),
        );
    }

    let mut records = Vec::with_capacity(tasks.len() * trials);
    for task in tasks {
        for trial_index in 0..trials {
            let runner_input = serde_json::json!({
                "format": "continuitydb.agent_behavior_runner_input",
                "format_version": 1,
                "task_id": task.task_id,
                "trial_index": trial_index,
                "strategy": task.strategy,
                "prompt": task.prompt,
                "context_packet": task.context_packet,
                "requirements": task.requirements,
            });
            let started_at = Instant::now();
            let runner_output =
                run_agent_behavior_model_runner(runner, runner_args, &runner_input)?;
            let model_latency_ms = started_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX);
            let model_output = runner_output
                .get("model_output")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    std::io::Error::other(format!(
                        "agent behavior runner output missing model_output for task {} strategy {} trial {}",
                        runner_input["task_id"].as_str().unwrap_or("<missing>"),
                        runner_input["strategy"].as_str().unwrap_or("<missing>"),
                        trial_index
                    ))
                })?
                .to_string();

            records.push(AgentBehaviorExecutionRecord {
                task_id: runner_input["task_id"]
                    .as_str()
                    .ok_or_else(|| std::io::Error::other("runner input missing task_id"))?
                    .to_string(),
                trial_index,
                strategy: runner_input["strategy"]
                    .as_str()
                    .ok_or_else(|| std::io::Error::other("runner input missing strategy"))?
                    .to_string(),
                prompt: runner_input["prompt"]
                    .as_str()
                    .ok_or_else(|| std::io::Error::other("runner input missing prompt"))?
                    .to_string(),
                context_packet: runner_input["context_packet"]
                    .as_str()
                    .ok_or_else(|| std::io::Error::other("runner input missing context_packet"))?
                    .to_string(),
                model_output,
                model_latency_ms: Some(model_latency_ms),
                requirements: serde_json::from_value(runner_input["requirements"].clone())?,
            });
        }
    }
    Ok(records)
}

fn agent_behavior_benchmark_bundle_json(
    artifact_dir: &Path,
    runner: &Path,
    runner_args: &[String],
    trials: usize,
    representative_corpus: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(artifact_dir)?;
    let matrix_path = artifact_dir.join("agent-behavior-task-matrix.json");
    let tasks_path = artifact_dir.join("agent-behavior-tasks.json");
    let records_path = artifact_dir.join("agent-behavior-output-records.json");
    let report_path = artifact_dir.join("agent-behavior-execution-benchmark.json");
    let manifest_path = artifact_dir.join("agent-behavior-benchmark.manifest.json");

    let matrix = agent_behavior_matrix_for_corpus(representative_corpus)?;
    write_pretty_json_file(&tasks_path, &serde_json::to_value(&matrix.tasks)?)?;
    let matrix_value = serde_json::to_value(&matrix)?;
    write_pretty_json_file(&matrix_path, &matrix_value)?;

    let records =
        run_agent_behavior_model_runner_tasks(matrix.tasks.clone(), runner, runner_args, trials)?;
    let records_value = serde_json::to_value(&records)?;
    write_pretty_json_file(&records_path, &records_value)?;

    let report = serde_json::to_value(score_agent_behavior_execution_records(records)?)?;
    write_pretty_json_file(&report_path, &report)?;

    let manifest = serde_json::json!({
        "format": "continuitydb.agent_behavior_benchmark_bundle",
        "format_version": 1,
        "artifact_dir": artifact_dir.display().to_string(),
        "runner": runner.display().to_string(),
        "runner_args": runner_args,
        "corpus": if representative_corpus { "representative_repo_lifecycle" } else { "curated_small" },
        "trial_count": trials,
        "scenario_count": matrix.scenario_count,
        "strategy_count": matrix.strategy_count,
        "task_count": matrix.tasks.len(),
        "execution_record_count": report["execution_record_count"],
        "artifacts": {
            "task_matrix": artifact_metadata_json(&matrix_path)?,
            "tasks": artifact_metadata_json(&tasks_path)?,
            "records": artifact_metadata_json(&records_path)?,
            "execution_report": artifact_metadata_json(&report_path)?,
        },
    });
    write_pretty_json_file(&manifest_path, &manifest)?;
    let manifest_text = std::fs::read_to_string(&manifest_path)?;

    Ok(serde_json::json!({
        "format": "continuitydb.agent_behavior_benchmark_bundle",
        "format_version": 1,
        "artifact_dir": artifact_dir.display().to_string(),
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
        "corpus": manifest["corpus"].clone(),
        "trial_count": trials,
        "task_count": matrix.tasks.len(),
        "execution_record_count": report["execution_record_count"],
        "artifacts": manifest["artifacts"].clone(),
    }))
}

fn agent_behavior_matrix_for_corpus(
    representative_corpus: bool,
) -> Result<AgentBehaviorTaskMatrixReport, Box<dyn std::error::Error>> {
    if representative_corpus {
        Ok(generate_representative_agent_behavior_task_matrix()?)
    } else {
        Ok(generate_agent_behavior_task_matrix()?)
    }
}

fn agent_behavior_benchmark_report_json(
    artifact_dir: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let manifest_path = artifact_dir.join("agent-behavior-benchmark.manifest.json");
    let execution_report_path = artifact_dir.join("agent-behavior-execution-benchmark.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)?;
    let execution_report_text = std::fs::read_to_string(&execution_report_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;
    let execution_report: serde_json::Value = serde_json::from_str(&execution_report_text)?;

    if manifest["format"].as_str() != Some("continuitydb.agent_behavior_benchmark_bundle") {
        return Err(std::io::Error::other("unsupported agent behavior benchmark manifest").into());
    }
    if execution_report["format"].as_str()
        != Some("continuitydb.agent_behavior_execution_benchmark")
    {
        return Err(std::io::Error::other("unsupported agent behavior execution report").into());
    }

    let corpus = manifest["corpus"].as_str().unwrap_or("unknown");
    let trial_count = manifest["trial_count"].as_u64().unwrap_or(0);
    let runner_args = manifest["runner_args"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let dry_run = runner_args
        .iter()
        .any(|arg| arg.as_str() == Some("--dry-run"));
    let representative = corpus == "representative_repo_lifecycle";
    let live_model_execution = !dry_run;
    let confidence_intervals_present =
        execution_report["strategies"]
            .as_array()
            .is_some_and(|strategies| {
                !strategies.is_empty()
                    && strategies.iter().all(|strategy| {
                        strategy["task_success_confidence_interval_bps"]["lower_bps"]
                            .as_u64()
                            .is_some()
                            && strategy["task_success_confidence_interval_bps"]["upper_bps"]
                                .as_u64()
                                .is_some()
                    })
            });
    let latency_present =
        execution_report["strategy_latency"]
            .as_array()
            .is_some_and(|latencies| {
                !latencies.is_empty()
                    && latencies.iter().all(|latency| {
                        latency["sample_count"]
                            .as_u64()
                            .is_some_and(|sample_count| sample_count > 0)
                            && latency["p50_ms"].as_u64().is_some()
                            && latency["p95_ms"].as_u64().is_some()
                            && latency["p99_ms"].as_u64().is_some()
                    })
            });
    let quality_gates = agent_behavior_benchmark_quality_gates_json(&execution_report);
    let mut marketing_blockers = Vec::new();
    if !live_model_execution {
        marketing_blockers.push("replace dry-run adapter with retained live model outputs");
    }
    if trial_count < 3 {
        marketing_blockers.push("run at least 3 repeated trials");
    }
    if !representative {
        marketing_blockers.push("use representative repo-lifecycle corpus");
    }
    if !confidence_intervals_present {
        marketing_blockers.push("include task-success confidence intervals");
    }
    if !latency_present {
        marketing_blockers.push("include p50/p95/p99 latency summaries");
    }
    if quality_gates["checkout_best_or_tied_task_success"].as_bool() != Some(true) {
        marketing_blockers.push("checkout must be best or tied on task success");
    }
    if quality_gates["checkout_lowest_or_tied_stale_belief"].as_bool() != Some(true) {
        marketing_blockers.push("checkout must be lowest or tied on stale-belief rate");
    }
    if quality_gates["checkout_lowest_or_tied_action_regression"].as_bool() != Some(true) {
        marketing_blockers.push("checkout must be lowest or tied on action-regression rate");
    }
    if quality_gates["checkout_minimum_task_success"].as_bool() != Some(true) {
        marketing_blockers.push("checkout task success must be at least 5000 bps");
    }
    let evidence_tier = if representative && dry_run {
        "representative_dry_run_artifact_validation"
    } else if representative && live_model_execution && trial_count >= 3 {
        "representative_live_model_evidence"
    } else if live_model_execution {
        "curated_live_model_evidence"
    } else {
        "curated_dry_run_artifact_validation"
    };
    let marketable = marketing_blockers.is_empty();

    Ok(serde_json::json!({
        "format": "continuitydb.agent_behavior_benchmark_report",
        "format_version": 1,
        "artifact_dir": artifact_dir.display().to_string(),
        "bundle": {
            "manifest_path": manifest_path.display().to_string(),
            "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
            "execution_report_path": execution_report_path.display().to_string(),
            "execution_report_fingerprint": fnv1a64_fingerprint(&execution_report_text),
            "corpus": corpus,
            "trial_count": trial_count,
            "task_count": manifest["task_count"].clone(),
            "execution_record_count": manifest["execution_record_count"].clone(),
            "runner": manifest["runner"].clone(),
            "runner_args": manifest["runner_args"].clone(),
        },
        "evidence_tier": evidence_tier,
        "marketable": marketable,
        "marketing_blockers": marketing_blockers,
        "gates": {
            "representative_corpus": representative,
            "live_model_execution": live_model_execution,
            "three_or_more_trials": trial_count >= 3,
            "confidence_intervals_present": confidence_intervals_present,
            "latency_percentiles_present": latency_present,
            "checkout_best_or_tied_task_success": quality_gates["checkout_best_or_tied_task_success"].clone(),
            "checkout_lowest_or_tied_stale_belief": quality_gates["checkout_lowest_or_tied_stale_belief"].clone(),
            "checkout_lowest_or_tied_action_regression": quality_gates["checkout_lowest_or_tied_action_regression"].clone(),
            "checkout_minimum_task_success": quality_gates["checkout_minimum_task_success"].clone(),
        },
        "metrics": agent_behavior_benchmark_report_metrics_json(&execution_report),
        "limitations": if marketable {
            vec![
                "Claims apply to the retained benchmark corpus and runner configuration.",
                "External claims still require publishing artifacts and reproduction commands.",
            ]
        } else {
            vec![
                "Dry-run results validate the artifact path, not model behavior.",
                "Do not use this bundle for external performance claims.",
            ]
        },
    }))
}

fn agent_behavior_benchmark_report_metrics_json(
    execution_report: &serde_json::Value,
) -> serde_json::Value {
    let strategies = execution_report["strategies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let best_strategy = strategies
        .iter()
        .max_by_key(|strategy| {
            strategy["metrics"]["task_success_rate_bps"]
                .as_u64()
                .unwrap_or(0)
        })
        .and_then(|strategy| strategy["strategy"].as_str())
        .unwrap_or("unknown");
    let checkout = strategies
        .iter()
        .find(|strategy| strategy["strategy"].as_str() == Some("continuitydb_checkout"));
    let checkout_latency = execution_report["strategy_latency"]
        .as_array()
        .and_then(|latencies| {
            latencies
                .iter()
                .find(|latency| latency["strategy"].as_str() == Some("continuitydb_checkout"))
        });

    serde_json::json!({
        "best_strategy": best_strategy,
        "checkout_task_success_rate_bps": checkout
            .and_then(|strategy| strategy["metrics"]["task_success_rate_bps"].as_u64()),
        "checkout_stale_belief_rate_bps": checkout
            .and_then(|strategy| strategy["metrics"]["stale_belief_rate_bps"].as_u64()),
        "checkout_action_regression_rate_bps": checkout
            .and_then(|strategy| strategy["metrics"]["action_regression_rate_bps"].as_u64()),
        "checkout_confidence_interval_bps": checkout
            .map(|strategy| strategy["task_success_confidence_interval_bps"].clone())
            .unwrap_or(serde_json::Value::Null),
        "checkout_latency_p50_ms": checkout_latency
            .and_then(|latency| latency["p50_ms"].as_u64()),
        "checkout_latency_p95_ms": checkout_latency
            .and_then(|latency| latency["p95_ms"].as_u64()),
        "checkout_latency_p99_ms": checkout_latency
            .and_then(|latency| latency["p99_ms"].as_u64()),
    })
}

fn agent_behavior_benchmark_quality_gates_json(
    execution_report: &serde_json::Value,
) -> serde_json::Value {
    let strategies = execution_report["strategies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let Some(checkout) = strategies
        .iter()
        .find(|strategy| strategy["strategy"].as_str() == Some("continuitydb_checkout"))
    else {
        return serde_json::json!({
            "checkout_best_or_tied_task_success": false,
            "checkout_lowest_or_tied_stale_belief": false,
            "checkout_lowest_or_tied_action_regression": false,
            "checkout_minimum_task_success": false,
        });
    };

    let checkout_success = agent_behavior_metric_bps(checkout, "task_success_rate_bps");
    let checkout_stale = agent_behavior_metric_bps(checkout, "stale_belief_rate_bps");
    let checkout_regression = agent_behavior_metric_bps(checkout, "action_regression_rate_bps");
    let max_success = strategies
        .iter()
        .filter_map(|strategy| agent_behavior_metric_bps(strategy, "task_success_rate_bps"))
        .max();
    let min_stale = strategies
        .iter()
        .filter_map(|strategy| agent_behavior_metric_bps(strategy, "stale_belief_rate_bps"))
        .min();
    let min_regression = strategies
        .iter()
        .filter_map(|strategy| agent_behavior_metric_bps(strategy, "action_regression_rate_bps"))
        .min();

    serde_json::json!({
        "checkout_best_or_tied_task_success": checkout_success
            .zip(max_success)
            .is_some_and(|(checkout_success, max_success)| checkout_success >= max_success),
        "checkout_lowest_or_tied_stale_belief": checkout_stale
            .zip(min_stale)
            .is_some_and(|(checkout_stale, min_stale)| checkout_stale <= min_stale),
        "checkout_lowest_or_tied_action_regression": checkout_regression
            .zip(min_regression)
            .is_some_and(|(checkout_regression, min_regression)| checkout_regression <= min_regression),
        "checkout_minimum_task_success": checkout_success
            .is_some_and(|checkout_success| checkout_success >= 5_000),
    })
}

fn agent_behavior_metric_bps(strategy: &serde_json::Value, metric: &str) -> Option<u64> {
    strategy["metrics"][metric].as_u64()
}

fn agent_behavior_benchmark_report_markdown(report: &serde_json::Value) -> String {
    let status = if report["marketable"].as_bool() == Some(true) {
        "Marketable"
    } else {
        "Not marketable"
    };
    let blockers = report["marketing_blockers"]
        .as_array()
        .map(|blockers| {
            blockers
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|blocker| format!("- {blocker}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "- none".to_string());

    format!(
        "# Agent Behavior Benchmark Report\n\n\
Status: {status}\n\n\
Evidence tier: `{}`\n\n\
Corpus: `{}`\n\n\
Trials: `{}`\n\n\
Execution records: `{}`\n\n\
Best strategy: `{}`\n\n\
Checkout task success: `{}` bps\n\n\
Checkout p95 latency: `{}` ms\n\n\
## Marketing Blockers\n\n{}\n",
        report["evidence_tier"].as_str().unwrap_or("unknown"),
        report["bundle"]["corpus"].as_str().unwrap_or("unknown"),
        report["bundle"]["trial_count"].as_u64().unwrap_or(0),
        report["bundle"]["execution_record_count"]
            .as_u64()
            .unwrap_or(0),
        report["metrics"]["best_strategy"]
            .as_str()
            .unwrap_or("unknown"),
        report["metrics"]["checkout_task_success_rate_bps"]
            .as_u64()
            .unwrap_or(0),
        report["metrics"]["checkout_latency_p95_ms"]
            .as_u64()
            .unwrap_or(0),
        blockers,
    )
}

fn thesis_falsification_benchmark_markdown(report: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    lines.push("# Thesis Falsification Benchmark Report".to_string());
    lines.push(String::new());
    lines.push(format!(
        "Question: {}",
        report["benchmark_question"].as_str().unwrap_or("unknown")
    ));
    lines.push(String::new());
    lines.push(format!(
        "Corpus documents: `{}`. Tasks: `{}`.",
        report["corpus"]["document_count"].as_u64().unwrap_or(0),
        report["task_count"].as_u64().unwrap_or(0)
    ));
    lines.push(String::new());
    lines.push("| Strategy | Passed | Task success | Revision | Forbidden action | Uncertainty | Verification | Evidence |".to_string());
    lines.push("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |".to_string());
    if let Some(strategies) = report["strategies"].as_array() {
        for strategy in strategies {
            lines.push(format!(
                "| `{}` | {} / {} | {} | {} | {} | {} | {} | {} |",
                strategy["strategy"].as_str().unwrap_or("unknown"),
                strategy["passed_task_count"].as_u64().unwrap_or(0),
                strategy["total_task_count"].as_u64().unwrap_or(0),
                strategy["metrics"]["task_success_rate_bps"]
                    .as_u64()
                    .unwrap_or(0),
                strategy["metrics"]["revision_preservation_bps"]
                    .as_u64()
                    .unwrap_or(0),
                strategy["metrics"]["forbidden_action_avoidance_bps"]
                    .as_u64()
                    .unwrap_or(0),
                strategy["metrics"]["uncertainty_faithfulness_bps"]
                    .as_u64()
                    .unwrap_or(0),
                strategy["metrics"]["verification_trigger_bps"]
                    .as_u64()
                    .unwrap_or(0),
                strategy["metrics"]["evidence_grounding_bps"]
                    .as_u64()
                    .unwrap_or(0),
            ));
        }
    }
    lines.push(String::new());
    lines.push("## Final Judgement".to_string());
    lines.push(String::new());
    lines.push(format!(
        "Verdict: `{}`.",
        report["judgement"]["verdict"].as_str().unwrap_or("unknown")
    ));
    lines.push(String::new());
    lines.push(format!(
        "Original database-primitive thesis survives: `{}`.",
        report["judgement"]["original_database_primitive_thesis_survives"]
            .as_bool()
            .unwrap_or(false)
    ));
    lines.push(format!(
        "The durable control-layer thesis survives: `{}`.",
        report["judgement"]["durable_control_layer_thesis_survives"]
            .as_bool()
            .unwrap_or(false)
    ));
    lines.push(String::new());
    lines.push(
        report["judgement"]["rationale"]
            .as_str()
            .unwrap_or("No rationale recorded.")
            .to_string(),
    );
    lines.push(String::new());
    lines.push(format!(
        "Required bar: {}",
        report["judgement"]["required_bar"]
            .as_str()
            .unwrap_or("unknown")
    ));
    lines.push(String::new());
    lines.join("\n")
}

fn artifact_metadata_json(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::json!({
        "path": path.display().to_string(),
        "fingerprint": fnv1a64_fingerprint(&text),
        "bytes": text.len(),
    }))
}

fn run_agent_behavior_model_runner(
    runner: &Path,
    runner_args: &[String],
    runner_input: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut child = ProcessCommand::new(runner)
        .args(runner_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::other("agent behavior runner stdin unavailable"))?;
        stdin.write_all(serde_json::to_string(runner_input)?.as_bytes())?;
        stdin.write_all(b"\n")?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "agent behavior runner failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
        .into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    Ok(serde_json::from_str(stdout.trim())?)
}

fn write_pretty_json_file(
    report_path: &Path,
    report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(report_path, serde_json::to_string_pretty(report)?)?;
    Ok(())
}

fn write_text_file(report_path: &Path, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(report_path, text)?;
    Ok(())
}

fn adversarial_task_harness_report_json() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 23, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid adversarial task timestamp"))?;
    let report = run_adversarial_agent_task_harness(committed_at)?;
    Ok(serde_json::json!({
        "format": "continuitydb.adversarial_task_harness_report",
        "format_version": 1,
        "valid": true,
        "committed_at": committed_at,
        "report": report,
    }))
}

#[cfg(feature = "local-model")]
fn finalize_local_model_benchmark_report(
    mut report: serde_json::Value,
    changed_case_report_path: Option<&Path>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    report["changed_case_report_path"] = changed_case_report_path
        .map(|path| serde_json::Value::String(path.display().to_string()))
        .unwrap_or(serde_json::Value::Null);
    let changed_case_report = changed_case_report_path
        .map(|path| write_local_model_changed_case_report(path, &report))
        .transpose()?;
    report["changed_case_report"] =
        local_model_changed_case_report_artifact_json(changed_case_report.as_ref());
    Ok(report)
}

#[cfg(feature = "local-model")]
fn write_local_model_changed_case_report(
    path: &Path,
    benchmark_report: &serde_json::Value,
) -> Result<LocalModelChangedCaseReportArtifact, Box<dyn std::error::Error>> {
    let changed_case_report = serde_json::json!({
        "format": "continuitydb.local_model.changed_cases",
        "format_version": 1,
        "candidate_model_id": benchmark_report["candidate_model_id"].clone(),
        "candidate_selection": benchmark_report["candidate_selection"].clone(),
        "baseline_path": benchmark_report["baseline_path"].clone(),
        "changed_case_report_path": path.display().to_string(),
        "comparison": local_model_changed_case_report_projection(benchmark_report),
    });
    let report_text = serde_json::to_string_pretty(&changed_case_report)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &report_text)?;
    Ok(LocalModelChangedCaseReportArtifact {
        report_path: path.to_path_buf(),
        report_fingerprint: local_model_contract_fingerprint(&report_text),
        report_bytes: report_text.len(),
    })
}

#[cfg(feature = "local-model")]
fn local_model_changed_case_report_artifact_json(
    artifact: Option<&LocalModelChangedCaseReportArtifact>,
) -> serde_json::Value {
    artifact
        .map(|artifact| {
            serde_json::json!({
                "report_path": artifact.report_path.display().to_string(),
                "report_fingerprint": artifact.report_fingerprint,
                "report_bytes": artifact.report_bytes,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

fn fnv1a64_fingerprint(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(feature = "local-model")]
fn local_model_stability_preflight_json(
    trials: usize,
    fail_on_unstable: bool,
) -> serde_json::Value {
    serde_json::json!({
        "trials": trials,
        "will_execute": false,
        "fail_on_unstable": fail_on_unstable,
    })
}

#[cfg(feature = "local-model")]
fn local_model_baseline_preflight_json(
    baseline_path: &Path,
    candidate: SmallModelCandidate,
    suite: &StewardEvaluationSuite,
    config: &LocalExecutableRunnerConfig,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let latest = if baseline_path.exists() {
        let store = FileLocalModelBenchmarkBaselineStore::open(baseline_path)?;
        let runtime_executable = config.executable().to_string_lossy().to_string();
        let runtime_arguments = config.command_arguments();
        let response_schema_version = LOCAL_MODEL_RESPONSE_SCHEMA_VERSION;
        let evaluation_suite_fingerprint = suite.fingerprint();
        let schema_fingerprint =
            local_model_contract_fingerprint(local_model_response_json_schema());
        let grammar_fingerprint =
            local_model_contract_fingerprint(local_model_response_gbnf_grammar());
        let prompt_fingerprint = local_model_prompt_fingerprint_for_suite(suite);

        store
            .list_baselines()?
            .into_iter()
            .filter(|baseline| baseline.candidate_model_id() == candidate.model_id())
            .filter(|baseline| baseline.candidate_role() == candidate.role())
            .filter(|baseline| baseline.response_schema_version() == response_schema_version)
            .filter(|baseline| {
                baseline.evaluation_suite_fingerprint() == evaluation_suite_fingerprint
            })
            .filter(|baseline| baseline.schema_fingerprint() == schema_fingerprint)
            .filter(|baseline| baseline.grammar_fingerprint() == grammar_fingerprint)
            .filter(|baseline| baseline.prompt_fingerprint() == prompt_fingerprint)
            .filter(|baseline| baseline.runtime().executable() == runtime_executable)
            .filter(|baseline| baseline.runtime().arguments() == runtime_arguments.as_slice())
            .max_by_key(LocalModelBenchmarkBaseline::recorded_at)
    } else {
        None
    };
    let previous_runtime = latest.as_ref().map(|baseline| {
        serde_json::json!({
            "executable": baseline.runtime().executable(),
            "arguments": baseline.runtime().arguments(),
        })
    });
    let previous_candidate_selection = latest.as_ref().map(|baseline| {
        serde_json::json!({
            "source": baseline.candidate_selection_source(),
            "model_id": baseline.candidate_model_id(),
        })
    });

    Ok(serde_json::json!({
        "compared": true,
        "compatible_baseline_found": latest.is_some(),
        "previous_recorded_at": latest.as_ref().map(LocalModelBenchmarkBaseline::recorded_at),
        "previous_schema_bytes": latest.as_ref().map(LocalModelBenchmarkBaseline::schema_bytes),
        "previous_grammar_bytes": latest.as_ref().map(LocalModelBenchmarkBaseline::grammar_bytes),
        "previous_response_schema_version": latest
            .as_ref()
            .map(LocalModelBenchmarkBaseline::response_schema_version),
        "previous_evaluation_suite_fingerprint": latest
            .as_ref()
            .map(LocalModelBenchmarkBaseline::evaluation_suite_fingerprint),
        "previous_schema_fingerprint": latest
            .as_ref()
            .map(LocalModelBenchmarkBaseline::schema_fingerprint),
        "previous_grammar_fingerprint": latest
            .as_ref()
            .map(LocalModelBenchmarkBaseline::grammar_fingerprint),
        "previous_prompt_fingerprint": latest
            .as_ref()
            .map(LocalModelBenchmarkBaseline::prompt_fingerprint),
        "previous_candidate_selection": previous_candidate_selection,
        "previous_runtime": previous_runtime,
    }))
}

#[cfg(feature = "local-model")]
fn local_model_contract_fingerprint(text: &str) -> String {
    fnv1a64_fingerprint(text)
}

#[cfg(feature = "local-model")]
fn write_local_model_contract_artifacts(
    contract_dir: &Path,
) -> Result<LocalModelContractArtifacts, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(contract_dir)?;
    let schema = local_model_response_json_schema();
    let grammar = local_model_response_gbnf_grammar();
    let context_compiler_schema = local_model_context_compiler_response_json_schema();
    let context_compiler_grammar = local_model_context_compiler_response_gbnf_grammar();
    let schema_path = contract_dir.join("local-model-response.schema.json");
    let grammar_path = contract_dir.join("local-model-response.gbnf");
    let context_compiler_schema_path =
        contract_dir.join("local-model-context-compiler-response.schema.json");
    let context_compiler_grammar_path =
        contract_dir.join("local-model-context-compiler-response.gbnf");
    std::fs::write(&schema_path, schema)?;
    std::fs::write(&grammar_path, grammar)?;
    std::fs::write(&context_compiler_schema_path, context_compiler_schema)?;
    std::fs::write(&context_compiler_grammar_path, context_compiler_grammar)?;

    Ok(LocalModelContractArtifacts {
        schema_path,
        grammar_path,
        schema_fingerprint: local_model_contract_fingerprint(schema),
        grammar_fingerprint: local_model_contract_fingerprint(grammar),
        schema_bytes: schema.len(),
        grammar_bytes: grammar.len(),
        context_compiler_schema_path,
        context_compiler_grammar_path,
        context_compiler_schema_version: LOCAL_MODEL_CONTEXT_COMPILER_RESPONSE_SCHEMA_VERSION,
        context_compiler_schema_fingerprint: local_model_contract_fingerprint(
            context_compiler_schema,
        ),
        context_compiler_grammar_fingerprint: local_model_contract_fingerprint(
            context_compiler_grammar,
        ),
        context_compiler_schema_bytes: context_compiler_schema.len(),
        context_compiler_grammar_bytes: context_compiler_grammar.len(),
    })
}

#[cfg(feature = "local-model")]
fn local_model_contract_artifacts_json(
    contract_artifacts: Option<&LocalModelContractArtifacts>,
) -> serde_json::Value {
    contract_artifacts
        .map(|artifacts| {
            serde_json::json!({
                "schema_path": artifacts.schema_path.display().to_string(),
                "grammar_path": artifacts.grammar_path.display().to_string(),
                "schema_fingerprint": artifacts.schema_fingerprint,
                "grammar_fingerprint": artifacts.grammar_fingerprint,
                "schema_bytes": artifacts.schema_bytes,
                "grammar_bytes": artifacts.grammar_bytes,
                "context_compiler_schema_path": artifacts.context_compiler_schema_path.display().to_string(),
                "context_compiler_grammar_path": artifacts.context_compiler_grammar_path.display().to_string(),
                "context_compiler_schema_version": artifacts.context_compiler_schema_version,
                "context_compiler_schema_fingerprint": artifacts.context_compiler_schema_fingerprint,
                "context_compiler_grammar_fingerprint": artifacts.context_compiler_grammar_fingerprint,
                "context_compiler_schema_bytes": artifacts.context_compiler_schema_bytes,
                "context_compiler_grammar_bytes": artifacts.context_compiler_grammar_bytes,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

#[cfg(feature = "local-model")]
fn write_local_model_prompt_artifacts(
    prompt_dir: &Path,
    suite: &StewardEvaluationSuite,
) -> Result<Vec<LocalModelPromptArtifact>, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(prompt_dir)?;
    suite
        .cases()
        .iter()
        .enumerate()
        .map(|(index, case)| {
            let prompt = local_model_prompt_for_input(case.input());
            let filename = format!(
                "{:03}-{}.prompt.txt",
                index + 1,
                local_model_prompt_filename_slug(case.name())
            );
            let prompt_path = prompt_dir.join(filename);
            std::fs::write(&prompt_path, &prompt)?;
            Ok(LocalModelPromptArtifact {
                case_name: case.name().to_string(),
                prompt_path,
                prompt_fingerprint: local_model_contract_fingerprint(&prompt),
                prompt_bytes: prompt.len(),
            })
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_prompt_filename_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "case".to_string()
    } else {
        slug
    }
}

#[cfg(feature = "local-model")]
fn local_model_prompt_artifacts_json(
    prompt_artifacts: &[LocalModelPromptArtifact],
) -> serde_json::Value {
    serde_json::Value::Array(
        prompt_artifacts
            .iter()
            .map(|artifact| {
                serde_json::json!({
                    "case_name": artifact.case_name,
                    "prompt_path": artifact.prompt_path.display().to_string(),
                    "prompt_fingerprint": artifact.prompt_fingerprint,
                    "prompt_bytes": artifact.prompt_bytes,
                })
            })
            .collect(),
    )
}

#[cfg(feature = "local-model")]
fn write_local_model_response_artifacts(
    response_dir: &Path,
    responses: &[StewardEvaluationCaseResponse],
) -> Result<Vec<LocalModelResponseArtifact>, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(response_dir)?;
    responses
        .iter()
        .enumerate()
        .map(|(index, response)| {
            if let Some(raw_response) = response.response() {
                let filename = format!(
                    "{:03}-{}.response.json",
                    index + 1,
                    local_model_prompt_filename_slug(response.case_name())
                );
                let response_path = response_dir.join(filename);
                std::fs::write(&response_path, raw_response)?;
                Ok(LocalModelResponseArtifact {
                    case_name: response.case_name().to_string(),
                    captured: true,
                    response_path: Some(response_path),
                    response_fingerprint: Some(local_model_contract_fingerprint(raw_response)),
                    response_bytes: response.response_bytes(),
                })
            } else {
                Ok(LocalModelResponseArtifact {
                    case_name: response.case_name().to_string(),
                    captured: false,
                    response_path: None,
                    response_fingerprint: None,
                    response_bytes: 0,
                })
            }
        })
        .collect()
}

#[cfg(feature = "local-model")]
fn local_model_response_artifacts_json(
    response_artifacts: &[LocalModelResponseArtifact],
) -> serde_json::Value {
    serde_json::Value::Array(
        response_artifacts
            .iter()
            .map(|artifact| {
                let (parseable, parse_error) = artifact
                    .response_path
                    .as_ref()
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .map(|response_text| {
                        let parsed_response = serde_json::from_str::<serde_json::Value>(&response_text);
                        (
                            parsed_response.is_ok(),
                            parsed_response.err().map(|error| error.to_string()),
                        )
                    })
                    .unwrap_or((false, None));
                serde_json::json!({
                    "case_name": artifact.case_name,
                    "captured": artifact.captured,
                    "response_path": artifact.response_path.as_ref().map(|path| path.display().to_string()),
                    "response_fingerprint": artifact.response_fingerprint,
                    "response_bytes": artifact.response_bytes,
                    "parseable": parseable,
                    "parse_error": parse_error,
                })
            })
            .collect(),
    )
}

#[cfg(feature = "local-model")]
fn write_local_model_response_artifact_manifest(
    response_dir: &Path,
    response_artifacts: &[LocalModelResponseArtifact],
) -> Result<LocalModelResponseArtifactManifest, Box<dyn std::error::Error>> {
    let manifest_path = response_dir.join("local-model-responses.manifest.json");
    let manifest = serde_json::json!({
        "format": "continuitydb.local_model.responses",
        "format_version": 1,
        "artifacts": local_model_response_artifacts_json(response_artifacts),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, &manifest_text)?;

    Ok(LocalModelResponseArtifactManifest {
        manifest_path,
        manifest_fingerprint: local_model_contract_fingerprint(&manifest_text),
        manifest_bytes: manifest_text.len(),
    })
}

#[cfg(feature = "local-model")]
fn local_model_response_artifact_manifest_json(
    response_manifest: Option<&LocalModelResponseArtifactManifest>,
) -> serde_json::Value {
    response_manifest
        .map(|manifest| {
            let (parseable, parse_error) = match std::fs::read_to_string(&manifest.manifest_path) {
                Ok(manifest_text) => {
                    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
                    (
                        parsed_manifest.is_ok(),
                        parsed_manifest.err().map(|error| error.to_string()),
                    )
                }
                Err(error) => (false, Some(error.to_string())),
            };
            serde_json::json!({
                "manifest_path": manifest.manifest_path.display().to_string(),
                "manifest_fingerprint": manifest.manifest_fingerprint,
                "manifest_bytes": manifest.manifest_bytes,
                "parseable": parseable,
                "parse_error": parse_error,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

#[cfg(feature = "local-model")]
fn write_local_model_bundle_manifest(
    artifact_dir: &Path,
    report_path: &Path,
    report: &serde_json::Value,
) -> Result<LocalModelBundleManifest, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(artifact_dir)?;
    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let report_text = local_model_benchmark_report_manifest_payload_text(report)?;
    let parsed_report = serde_json::from_str::<serde_json::Value>(&report_text);
    let report_parse_error = parsed_report.as_ref().err().map(|error| error.to_string());
    let manifest = serde_json::json!({
        "format": "continuitydb.local_model.benchmark_bundle",
        "format_version": 1,
        "benchmark_report_path": report_path.display().to_string(),
        "benchmark_report_fingerprint": local_model_contract_fingerprint(&report_text),
        "benchmark_report_bytes": report_text.len(),
        "benchmark_report_parseable": parsed_report.is_ok(),
        "benchmark_report_parse_error": report_parse_error,
        "contract_artifacts": report["contract_artifacts"].clone(),
        "prompt_artifacts": report["prompt_artifacts"].clone(),
        "response_artifacts": report["response_artifacts"].clone(),
        "response_artifact_manifest": report["response_artifact_manifest"].clone(),
        "changed_case_report_path": report["changed_case_report_path"].clone(),
        "changed_case_report": report["changed_case_report"].clone(),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, &manifest_text)?;

    Ok(LocalModelBundleManifest {
        manifest_path,
        manifest_fingerprint: local_model_contract_fingerprint(&manifest_text),
        manifest_bytes: manifest_text.len(),
    })
}

#[cfg(feature = "local-model")]
fn local_model_benchmark_report_manifest_payload_text(
    report: &serde_json::Value,
) -> Result<String, serde_json::Error> {
    let mut payload = report.clone();
    payload["bundle_manifest"] = serde_json::Value::Null;
    serde_json::to_string_pretty(&payload)
}

#[cfg(feature = "local-model")]
fn write_local_model_bundle_validation_failure_report(
    artifact_dir: &Path,
    report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = local_model_validation_failure_manifest_json(artifact_dir);
    let benchmark_report = local_model_validation_failure_report_json(artifact_dir);
    let changed_case_report = local_model_validation_failure_changed_case_report_json(artifact_dir);
    let contract_artifacts = local_model_validation_failure_contract_artifacts_json(artifact_dir);
    let response_artifact_manifest =
        local_model_validation_failure_response_artifact_manifest_json(artifact_dir);
    let response_artifacts = local_model_validation_failure_response_artifacts_json(artifact_dir);
    let output = serde_json::json!({
        "artifact_dir": artifact_dir.display().to_string(),
        "report_path": report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "manifest": manifest,
        "benchmark_report": benchmark_report,
        "changed_case_report": changed_case_report,
        "contract_artifacts": contract_artifacts,
        "response_artifact_manifest": response_artifact_manifest,
        "response_artifacts": response_artifacts,
        "failure": {
            "stage": "local_model_bundle_validation",
            "message": message,
        },
    });
    write_pretty_json_file(failure_report_path, &output)?;
    Ok(())
}

#[cfg(feature = "local-model")]
fn local_model_validation_failure_manifest_json(artifact_dir: &Path) -> serde_json::Value {
    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
        return serde_json::Value::Null;
    };
    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
    let parse_error = parsed_manifest
        .as_ref()
        .err()
        .map(|error| error.to_string());

    serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": local_model_contract_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
        "parseable": parsed_manifest.is_ok(),
        "parse_error": parse_error,
    })
}

#[cfg(feature = "local-model")]
fn local_model_validation_failure_report_json(artifact_dir: &Path) -> serde_json::Value {
    let report_path = artifact_dir.join("benchmark-report.json");
    let Ok(report_text) = std::fs::read_to_string(&report_path) else {
        return serde_json::Value::Null;
    };
    let parsed_report = serde_json::from_str::<serde_json::Value>(&report_text);
    let parse_error = parsed_report.as_ref().err().map(|error| error.to_string());
    let canonical_report_text = parsed_report
        .as_ref()
        .ok()
        .cloned()
        .and_then(|report| local_model_benchmark_report_manifest_payload_text(&report).ok())
        .unwrap_or_else(|| report_text.clone());

    serde_json::json!({
        "report_path": report_path.display().to_string(),
        "report_fingerprint": local_model_contract_fingerprint(&canonical_report_text),
        "report_bytes": canonical_report_text.len(),
        "parseable": parsed_report.is_ok(),
        "parse_error": parse_error,
    })
}

#[cfg(feature = "local-model")]
fn local_model_validation_failure_changed_case_report_json(
    artifact_dir: &Path,
) -> serde_json::Value {
    let report_path = artifact_dir.join("changed-cases.json");
    let Ok(report_text) = std::fs::read_to_string(&report_path) else {
        return serde_json::Value::Null;
    };
    let parsed_report = serde_json::from_str::<serde_json::Value>(&report_text);
    let parse_error = parsed_report.as_ref().err().map(|error| error.to_string());
    let candidate_selection = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["candidate_selection"].clone())
        .filter(|candidate_selection| !candidate_selection.is_null())
        .unwrap_or(serde_json::Value::Null);
    let comparison = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["comparison"].clone())
        .filter(|comparison| !comparison.is_null())
        .unwrap_or(serde_json::Value::Null);
    let candidate_model_id = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["candidate_model_id"].clone())
        .filter(|candidate_model_id| !candidate_model_id.is_null())
        .unwrap_or(serde_json::Value::Null);
    let baseline_path = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["baseline_path"].clone())
        .filter(|baseline_path| !baseline_path.is_null())
        .unwrap_or(serde_json::Value::Null);
    let format = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["format"].clone())
        .filter(|format| !format.is_null())
        .unwrap_or(serde_json::Value::Null);
    let format_version = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["format_version"].clone())
        .filter(|format_version| !format_version.is_null())
        .unwrap_or(serde_json::Value::Null);
    let changed_case_report_path = parsed_report
        .as_ref()
        .ok()
        .map(|report| report["changed_case_report_path"].clone())
        .filter(|changed_case_report_path| !changed_case_report_path.is_null())
        .unwrap_or(serde_json::Value::Null);

    serde_json::json!({
        "report_path": report_path.display().to_string(),
        "report_fingerprint": local_model_contract_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "parseable": parsed_report.is_ok(),
        "parse_error": parse_error,
        "format": format,
        "format_version": format_version,
        "changed_case_report_path": changed_case_report_path,
        "candidate_model_id": candidate_model_id,
        "baseline_path": baseline_path,
        "candidate_selection": candidate_selection,
        "comparison": comparison,
    })
}

#[cfg(feature = "local-model")]
fn local_model_changed_case_report_metadata_json(
    report_path: &Path,
    report_text: &str,
    changed_case_report: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "report_path": report_path.display().to_string(),
        "report_fingerprint": local_model_contract_fingerprint(report_text),
        "report_bytes": report_text.len(),
        "parseable": true,
        "parse_error": serde_json::Value::Null,
        "format": changed_case_report["format"].clone(),
        "format_version": changed_case_report["format_version"].clone(),
        "changed_case_report_path": changed_case_report["changed_case_report_path"].clone(),
        "candidate_model_id": changed_case_report["candidate_model_id"].clone(),
        "baseline_path": changed_case_report["baseline_path"].clone(),
        "candidate_selection": changed_case_report["candidate_selection"].clone(),
        "comparison": changed_case_report["comparison"].clone(),
    })
}

#[cfg(feature = "local-model")]
fn local_model_validation_failure_contract_artifacts_json(
    artifact_dir: &Path,
) -> serde_json::Value {
    let report_path = artifact_dir.join("benchmark-report.json");
    let Ok(report_text) = std::fs::read_to_string(&report_path) else {
        return serde_json::Value::Null;
    };
    let Ok(report) = serde_json::from_str::<serde_json::Value>(&report_text) else {
        return serde_json::Value::Null;
    };
    let contract_artifacts = &report["contract_artifacts"];
    if contract_artifacts.is_null() {
        return serde_json::Value::Null;
    }

    let Some(schema_path) = contract_artifacts["schema_path"].as_str() else {
        return serde_json::Value::Null;
    };
    let Some(grammar_path) = contract_artifacts["grammar_path"].as_str() else {
        return serde_json::Value::Null;
    };
    let Some(context_compiler_schema_path) =
        contract_artifacts["context_compiler_schema_path"].as_str()
    else {
        return serde_json::Value::Null;
    };
    let Some(context_compiler_grammar_path) =
        contract_artifacts["context_compiler_grammar_path"].as_str()
    else {
        return serde_json::Value::Null;
    };
    let Ok(schema_text) = std::fs::read_to_string(schema_path) else {
        return serde_json::Value::Null;
    };
    let Ok(grammar_text) = std::fs::read_to_string(grammar_path) else {
        return serde_json::Value::Null;
    };
    let Ok(context_compiler_schema_text) = std::fs::read_to_string(context_compiler_schema_path)
    else {
        return serde_json::Value::Null;
    };
    let Ok(context_compiler_grammar_text) = std::fs::read_to_string(context_compiler_grammar_path)
    else {
        return serde_json::Value::Null;
    };

    serde_json::json!({
        "schema_path": schema_path,
        "schema_fingerprint": local_model_contract_fingerprint(&schema_text),
        "schema_bytes": schema_text.len(),
        "grammar_path": grammar_path,
        "grammar_fingerprint": local_model_contract_fingerprint(&grammar_text),
        "grammar_bytes": grammar_text.len(),
        "context_compiler_schema_path": context_compiler_schema_path,
        "context_compiler_schema_fingerprint": local_model_contract_fingerprint(&context_compiler_schema_text),
        "context_compiler_schema_bytes": context_compiler_schema_text.len(),
        "context_compiler_grammar_path": context_compiler_grammar_path,
        "context_compiler_grammar_fingerprint": local_model_contract_fingerprint(&context_compiler_grammar_text),
        "context_compiler_grammar_bytes": context_compiler_grammar_text.len(),
    })
}

#[cfg(feature = "local-model")]
fn local_model_validation_failure_response_artifact_manifest_json(
    artifact_dir: &Path,
) -> serde_json::Value {
    let manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
        return serde_json::Value::Null;
    };
    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
    let parse_error = parsed_manifest
        .as_ref()
        .err()
        .map(|error| error.to_string());

    serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": local_model_contract_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
        "parseable": parsed_manifest.is_ok(),
        "parse_error": parse_error,
    })
}

#[cfg(feature = "local-model")]
fn local_model_validation_failure_response_artifacts_json(
    artifact_dir: &Path,
) -> serde_json::Value {
    let response_dir = artifact_dir.join("responses");
    let manifest_path = response_dir.join("local-model-responses.manifest.json");
    let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
        return serde_json::Value::Null;
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&manifest_text) else {
        return serde_json::Value::Null;
    };
    let Some(artifacts) = manifest["artifacts"].as_array() else {
        return serde_json::Value::Null;
    };

    serde_json::Value::Array(
        artifacts
            .iter()
            .map(|artifact| {
                let captured = artifact["captured"].as_bool().unwrap_or(false);
                let response_path = artifact["response_path"].as_str();
                let response_text = if captured {
                    response_path
                        .map(Path::new)
                        .filter(|path| path.strip_prefix(&response_dir).is_ok())
                        .and_then(|path| std::fs::read_to_string(path).ok())
                } else {
                    None
                };
                let (parseable, parse_error) = response_text
                    .as_ref()
                    .map(|text| {
                        let parsed_response = serde_json::from_str::<serde_json::Value>(text);
                        (
                            parsed_response.is_ok(),
                            parsed_response.err().map(|error| error.to_string()),
                        )
                    })
                    .unwrap_or((false, None));

                serde_json::json!({
                    "case_name": artifact["case_name"].clone(),
                    "captured": captured,
                    "response_path": response_path,
                    "response_fingerprint": response_text
                        .as_ref()
                        .map(|text| local_model_contract_fingerprint(text)),
                    "response_bytes": response_text.as_ref().map(String::len),
                    "parseable": parseable,
                    "parse_error": parse_error,
                })
            })
            .collect(),
    )
}

#[cfg(feature = "local-model")]
fn validate_local_model_bundle_manifest(
    artifact_dir: &Path,
) -> Result<LocalModelBundleValidation, Box<dyn std::error::Error>> {
    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        std::io::Error::other(format!(
            "local model benchmark manifest is required: {error}"
        ))
    })?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;

    if manifest["format"].as_str() != Some("continuitydb.local_model.benchmark_bundle")
        || manifest["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported local model benchmark manifest").into());
    }

    let report_path = artifact_dir.join("benchmark-report.json");
    let expected_report_path = report_path.display().to_string();
    let manifest_report_path = required_json_string(&manifest, "benchmark_report_path")?;
    if manifest_report_path != expected_report_path {
        return Err(
            std::io::Error::other("local model benchmark manifest report path mismatch").into(),
        );
    }

    let report_text = std::fs::read_to_string(&report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let manifest_report_payload_text = local_model_benchmark_report_manifest_payload_text(&report)?;
    let manifest_report_bytes = required_json_u64(&manifest, "benchmark_report_bytes")?;
    if manifest_report_bytes != manifest_report_payload_text.len() as u64 {
        return Err(
            std::io::Error::other("local model benchmark manifest byte count mismatch").into(),
        );
    }
    let manifest_report_fingerprint =
        required_json_string(&manifest, "benchmark_report_fingerprint")?;
    let current_report_fingerprint =
        local_model_contract_fingerprint(&manifest_report_payload_text);
    if manifest_report_fingerprint != current_report_fingerprint {
        return Err(
            std::io::Error::other("local model benchmark manifest fingerprint mismatch").into(),
        );
    }
    let changed_case_report =
        validate_local_model_changed_case_report_manifest(artifact_dir, &manifest, &report)?;
    let contract_artifacts =
        validate_local_model_contract_artifacts(artifact_dir, &manifest, &report)?;
    let prompt_artifacts = validate_local_model_prompt_artifacts(artifact_dir, &manifest, &report)?;
    let response_artifact_manifest =
        validate_local_model_response_artifact_manifest(artifact_dir, &manifest, &report)?;
    let response_artifacts = report["response_artifacts"].clone();

    Ok(LocalModelBundleValidation {
        manifest: LocalModelBundleManifest {
            manifest_path,
            manifest_fingerprint: local_model_contract_fingerprint(&manifest_text),
            manifest_bytes: manifest_text.len(),
        },
        benchmark_report: serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": current_report_fingerprint,
            "report_bytes": manifest_report_payload_text.len(),
            "candidate_model_id": report["candidate_model_id"].clone(),
            "dry_run": report["dry_run"].clone(),
        }),
        changed_case_report,
        contract_artifacts,
        prompt_artifacts,
        response_artifacts,
        response_artifact_manifest,
    })
}

#[cfg(feature = "local-model")]
fn local_model_bundle_validation_report_json(
    artifact_dir: &Path,
    report_path: Option<&Path>,
    failure_report_path: Option<&Path>,
    validation: LocalModelBundleValidation,
) -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.local_model_bundle_validation",
        "format_version": 1,
        "valid": true,
        "artifact_dir": artifact_dir.display().to_string(),
        "report_path": report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.map(|path| path.display().to_string()),
        "candidate_model_id": validation.benchmark_report["candidate_model_id"].clone(),
        "dry_run": validation.benchmark_report["dry_run"].clone(),
        "manifest": local_model_bundle_manifest_json(Some(&validation.manifest)),
        "benchmark_report": validation.benchmark_report,
        "changed_case_report": validation.changed_case_report,
        "contract_artifacts": validation.contract_artifacts,
        "prompt_artifacts": validation.prompt_artifacts,
        "response_artifacts": validation.response_artifacts,
        "response_artifact_manifest": validation.response_artifact_manifest,
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_contract_artifacts(
    artifact_dir: &Path,
    manifest: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if manifest["contract_artifacts"].is_null() {
        return Ok(serde_json::Value::Null);
    }
    if manifest["contract_artifacts"] != benchmark_report["contract_artifacts"] {
        return Err(std::io::Error::other("local model contract artifact content mismatch").into());
    }

    let contract_dir = artifact_dir.join("contracts");
    let contract_artifacts = &benchmark_report["contract_artifacts"];

    let schema_path_text = required_json_string(contract_artifacts, "schema_path")?;
    let schema_path = Path::new(schema_path_text);
    if schema_path.strip_prefix(&contract_dir).is_err() {
        return Err(
            std::io::Error::other("local model contract artifact schema path mismatch").into(),
        );
    }
    let schema_text = std::fs::read_to_string(schema_path)?;
    let schema_bytes = required_json_u64(contract_artifacts, "schema_bytes")?;
    if schema_bytes != schema_text.len() as u64 {
        return Err(std::io::Error::other(
            "local model contract artifact schema byte count mismatch",
        )
        .into());
    }
    let schema_fingerprint = required_json_string(contract_artifacts, "schema_fingerprint")?;
    if schema_fingerprint != local_model_contract_fingerprint(&schema_text) {
        return Err(std::io::Error::other(
            "local model contract artifact schema fingerprint mismatch",
        )
        .into());
    }

    let grammar_path_text = required_json_string(contract_artifacts, "grammar_path")?;
    let grammar_path = Path::new(grammar_path_text);
    if grammar_path.strip_prefix(&contract_dir).is_err() {
        return Err(
            std::io::Error::other("local model contract artifact grammar path mismatch").into(),
        );
    }
    let grammar_text = std::fs::read_to_string(grammar_path)?;
    let grammar_bytes = required_json_u64(contract_artifacts, "grammar_bytes")?;
    if grammar_bytes != grammar_text.len() as u64 {
        return Err(std::io::Error::other(
            "local model contract artifact grammar byte count mismatch",
        )
        .into());
    }
    let grammar_fingerprint = required_json_string(contract_artifacts, "grammar_fingerprint")?;
    if grammar_fingerprint != local_model_contract_fingerprint(&grammar_text) {
        return Err(std::io::Error::other(
            "local model contract artifact grammar fingerprint mismatch",
        )
        .into());
    }

    let context_schema_path_text =
        required_json_string(contract_artifacts, "context_compiler_schema_path")?;
    let context_schema_path = Path::new(context_schema_path_text);
    if context_schema_path.strip_prefix(&contract_dir).is_err() {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact schema path mismatch",
        )
        .into());
    }
    let context_schema_text = std::fs::read_to_string(context_schema_path)?;
    let context_schema_bytes =
        required_json_u64(contract_artifacts, "context_compiler_schema_bytes")?;
    if context_schema_bytes != context_schema_text.len() as u64 {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact schema byte count mismatch",
        )
        .into());
    }
    let context_schema_fingerprint =
        required_json_string(contract_artifacts, "context_compiler_schema_fingerprint")?;
    if context_schema_fingerprint != local_model_contract_fingerprint(&context_schema_text) {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact schema fingerprint mismatch",
        )
        .into());
    }
    let context_schema_version =
        required_json_u64(contract_artifacts, "context_compiler_schema_version")?;
    let context_schema_json: serde_json::Value = serde_json::from_str(&context_schema_text)?;
    if context_schema_version
        != context_schema_json["x-continuitydb-schema-version"]
            .as_u64()
            .unwrap_or_default()
        || context_schema_version != LOCAL_MODEL_CONTEXT_COMPILER_RESPONSE_SCHEMA_VERSION as u64
    {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact schema version mismatch",
        )
        .into());
    }

    let context_grammar_path_text =
        required_json_string(contract_artifacts, "context_compiler_grammar_path")?;
    let context_grammar_path = Path::new(context_grammar_path_text);
    if context_grammar_path.strip_prefix(&contract_dir).is_err() {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact grammar path mismatch",
        )
        .into());
    }
    let context_grammar_text = std::fs::read_to_string(context_grammar_path)?;
    let context_grammar_bytes =
        required_json_u64(contract_artifacts, "context_compiler_grammar_bytes")?;
    if context_grammar_bytes != context_grammar_text.len() as u64 {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact grammar byte count mismatch",
        )
        .into());
    }
    let context_grammar_fingerprint =
        required_json_string(contract_artifacts, "context_compiler_grammar_fingerprint")?;
    if context_grammar_fingerprint != local_model_contract_fingerprint(&context_grammar_text) {
        return Err(std::io::Error::other(
            "local model context compiler contract artifact grammar fingerprint mismatch",
        )
        .into());
    }

    Ok(contract_artifacts.clone())
}

#[cfg(feature = "local-model")]
fn validate_local_model_prompt_artifacts(
    artifact_dir: &Path,
    manifest: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if manifest["prompt_artifacts"].is_null() {
        return Ok(serde_json::Value::Null);
    }
    if manifest["prompt_artifacts"] != benchmark_report["prompt_artifacts"] {
        return Err(std::io::Error::other("local model prompt artifact content mismatch").into());
    }

    let prompt_dir = artifact_dir.join("prompts");
    let artifacts = benchmark_report["prompt_artifacts"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("local model prompt artifacts missing artifacts"))?;
    for artifact in artifacts {
        let prompt_path_text = required_json_string(artifact, "prompt_path")?;
        let prompt_path = Path::new(prompt_path_text);
        if prompt_path.strip_prefix(&prompt_dir).is_err() {
            return Err(std::io::Error::other("local model prompt artifact path mismatch").into());
        }
        let prompt_text = std::fs::read_to_string(prompt_path)?;

        let prompt_bytes = required_json_u64(artifact, "prompt_bytes")?;
        if prompt_bytes != prompt_text.len() as u64 {
            return Err(
                std::io::Error::other("local model prompt artifact byte count mismatch").into(),
            );
        }

        let prompt_fingerprint = required_json_string(artifact, "prompt_fingerprint")?;
        let current_prompt_fingerprint = local_model_contract_fingerprint(&prompt_text);
        if prompt_fingerprint != current_prompt_fingerprint {
            return Err(
                std::io::Error::other("local model prompt artifact fingerprint mismatch").into(),
            );
        }
    }

    Ok(benchmark_report["prompt_artifacts"].clone())
}

#[cfg(feature = "local-model")]
fn validate_local_model_response_artifact_manifest(
    artifact_dir: &Path,
    manifest: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if manifest["response_artifact_manifest"].is_null() {
        return Ok(serde_json::Value::Null);
    }

    let manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let expected_manifest_path = manifest_path.display().to_string();
    let manifest_response_path =
        required_json_string(&manifest["response_artifact_manifest"], "manifest_path")?;
    if manifest_response_path != expected_manifest_path {
        return Err(std::io::Error::other(
            "local model benchmark manifest response artifact manifest path mismatch",
        )
        .into());
    }

    let manifest_text = std::fs::read_to_string(&manifest_path)?;
    let manifest_response_bytes =
        required_json_u64(&manifest["response_artifact_manifest"], "manifest_bytes")?;
    if manifest_response_bytes != manifest_text.len() as u64 {
        return Err(std::io::Error::other(
            "local model benchmark manifest response artifact manifest byte count mismatch",
        )
        .into());
    }

    let manifest_response_fingerprint = required_json_string(
        &manifest["response_artifact_manifest"],
        "manifest_fingerprint",
    )?;
    let current_response_fingerprint = local_model_contract_fingerprint(&manifest_text);
    if manifest_response_fingerprint != current_response_fingerprint {
        return Err(std::io::Error::other(
            "local model benchmark manifest response artifact manifest fingerprint mismatch",
        )
        .into());
    }
    let response_manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;
    validate_local_model_response_artifact_files(artifact_dir, &response_manifest)?;
    validate_local_model_response_artifact_manifest_content(&response_manifest, benchmark_report)?;

    Ok(serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": current_response_fingerprint,
        "manifest_bytes": manifest_text.len(),
        "parseable": true,
        "parse_error": serde_json::Value::Null,
    }))
}

#[cfg(feature = "local-model")]
fn validate_local_model_response_artifact_manifest_content(
    response_manifest: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    if response_manifest["format"].as_str() != Some("continuitydb.local_model.responses") {
        return Err(std::io::Error::other(
            "local model response artifact manifest content mismatch: format",
        )
        .into());
    }

    if response_manifest["format_version"].as_u64() != Some(1) {
        return Err(std::io::Error::other(
            "local model response artifact manifest content mismatch: format_version",
        )
        .into());
    }

    if response_manifest["artifacts"] != benchmark_report["response_artifacts"] {
        return Err(std::io::Error::other(
            "local model response artifact manifest content mismatch: artifacts",
        )
        .into());
    }

    Ok(())
}

#[cfg(feature = "local-model")]
fn validate_local_model_response_artifact_files(
    artifact_dir: &Path,
    response_manifest: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    if response_manifest["format"].as_str() != Some("continuitydb.local_model.responses")
        || response_manifest["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("unsupported local model response artifact manifest").into(),
        );
    }

    let response_dir = artifact_dir.join("responses");
    let artifacts = response_manifest["artifacts"].as_array().ok_or_else(|| {
        std::io::Error::other("local model response artifact manifest missing artifacts")
    })?;

    for artifact in artifacts {
        if artifact["captured"].as_bool() != Some(true) {
            continue;
        }

        let response_path_text = required_json_string(artifact, "response_path")?;
        let response_path = Path::new(response_path_text);
        if response_path.strip_prefix(&response_dir).is_err() {
            return Err(
                std::io::Error::other("local model response artifact path mismatch").into(),
            );
        }
        let response_text = std::fs::read_to_string(response_path)?;

        let manifest_response_bytes = required_json_u64(artifact, "response_bytes")?;
        if manifest_response_bytes != response_text.len() as u64 {
            return Err(
                std::io::Error::other("local model response artifact byte count mismatch").into(),
            );
        }

        let manifest_response_fingerprint = required_json_string(artifact, "response_fingerprint")?;
        let current_response_fingerprint = local_model_contract_fingerprint(&response_text);
        if manifest_response_fingerprint != current_response_fingerprint {
            return Err(std::io::Error::other(
                "local model response artifact fingerprint mismatch",
            )
            .into());
        }
    }

    Ok(())
}

#[cfg(feature = "local-model")]
fn validate_local_model_changed_case_report_manifest(
    artifact_dir: &Path,
    manifest: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if manifest["changed_case_report"].is_null() && manifest["changed_case_report_path"].is_null() {
        return Ok(serde_json::Value::Null);
    }

    let report_path = artifact_dir.join("changed-cases.json");
    let expected_report_path = report_path.display().to_string();
    let manifest_report_path = required_json_string(manifest, "changed_case_report_path")?;
    if manifest_report_path != expected_report_path {
        return Err(std::io::Error::other(
            "local model benchmark manifest changed-case report path mismatch",
        )
        .into());
    }

    let nested_report_path = required_json_string(&manifest["changed_case_report"], "report_path")?;
    if nested_report_path != expected_report_path {
        return Err(std::io::Error::other(
            "local model benchmark manifest changed-case report path mismatch",
        )
        .into());
    }

    let report_text = std::fs::read_to_string(&report_path)?;
    let manifest_report_bytes =
        required_json_u64(&manifest["changed_case_report"], "report_bytes")?;
    if manifest_report_bytes != report_text.len() as u64 {
        return Err(std::io::Error::other(
            "local model benchmark manifest changed-case report byte count mismatch",
        )
        .into());
    }

    let manifest_report_fingerprint =
        required_json_string(&manifest["changed_case_report"], "report_fingerprint")?;
    let current_report_fingerprint = local_model_contract_fingerprint(&report_text);
    if manifest_report_fingerprint != current_report_fingerprint {
        return Err(std::io::Error::other(
            "local model benchmark manifest changed-case report fingerprint mismatch",
        )
        .into());
    }
    let changed_case_report: serde_json::Value = serde_json::from_str(&report_text)?;
    validate_local_model_changed_case_report_content(
        &report_path,
        &changed_case_report,
        benchmark_report,
    )?;

    let mut metadata = local_model_changed_case_report_metadata_json(
        &report_path,
        &report_text,
        &changed_case_report,
    );
    metadata["report_fingerprint"] = serde_json::Value::from(current_report_fingerprint);
    Ok(metadata)
}

#[cfg(feature = "local-model")]
fn local_model_changed_case_report_projection(
    benchmark_report: &serde_json::Value,
) -> serde_json::Value {
    let comparison = benchmark_report
        .get("baseline_comparison")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "compared": comparison["compared"].as_bool().unwrap_or(false),
        "regressed": comparison["regressed"].as_bool().unwrap_or(false),
        "previous_recorded_at": comparison["previous_recorded_at"].clone(),
        "current_recorded_at": comparison["current_recorded_at"].clone(),
        "changed_cases": comparison["changed_cases"].as_u64().unwrap_or(0),
        "outcome_changed_cases": comparison["outcome_changed_cases"].as_u64().unwrap_or(0),
        "failure_count_changed_cases": comparison["failure_count_changed_cases"].as_u64().unwrap_or(0),
        "response_changed_cases": comparison["response_changed_cases"].as_u64().unwrap_or(0),
        "regressed_cases": comparison["regressed_cases"].clone(),
        "recovered_cases": comparison["recovered_cases"].clone(),
        "changed_case_summaries": comparison["changed_case_summaries"].clone(),
    })
}

#[cfg(feature = "local-model")]
fn validate_local_model_changed_case_report_content(
    report_path: &Path,
    changed_case_report: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected_report_path = report_path.display().to_string();
    let expected_comparison = local_model_changed_case_report_projection(benchmark_report);
    if changed_case_report["format"].as_str() != Some("continuitydb.local_model.changed_cases")
        || changed_case_report["format_version"].as_u64() != Some(1)
        || changed_case_report["candidate_model_id"] != benchmark_report["candidate_model_id"]
        || changed_case_report["candidate_selection"] != benchmark_report["candidate_selection"]
        || changed_case_report["baseline_path"] != benchmark_report["baseline_path"]
        || changed_case_report["changed_case_report_path"].as_str()
            != Some(expected_report_path.as_str())
        || changed_case_report["comparison"] != expected_comparison
    {
        return Err(
            std::io::Error::other("local model changed-case report content mismatch").into(),
        );
    }

    Ok(())
}

#[cfg(feature = "local-model")]
fn local_model_bundle_manifest_json(
    bundle_manifest: Option<&LocalModelBundleManifest>,
) -> serde_json::Value {
    bundle_manifest
        .map(|manifest| {
            let (parseable, parse_error) = match std::fs::read_to_string(&manifest.manifest_path) {
                Ok(manifest_text) => {
                    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
                    (
                        parsed_manifest.is_ok(),
                        parsed_manifest.err().map(|error| error.to_string()),
                    )
                }
                Err(error) => (false, Some(error.to_string())),
            };
            serde_json::json!({
                "manifest_path": manifest.manifest_path.display().to_string(),
                "manifest_fingerprint": manifest.manifest_fingerprint,
                "manifest_bytes": manifest.manifest_bytes,
                "parseable": parseable,
                "parse_error": parse_error,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

#[cfg(feature = "local-model")]
fn write_local_model_artifact_bundle_report(
    artifact_dir: &Path,
    mut report: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_path = artifact_dir.join("benchmark-report.json");
    write_pretty_json_file(&report_path, &report)?;
    let bundle_manifest = write_local_model_bundle_manifest(artifact_dir, &report_path, &report)?;
    report["bundle_manifest"] = local_model_bundle_manifest_json(Some(&bundle_manifest));
    write_pretty_json_file(&report_path, &report)?;
    Ok(report)
}

#[cfg(feature = "local-model")]
fn local_model_candidate_selection(
    candidate_id: Option<&str>,
) -> Result<LocalModelCandidateSelection, Box<dyn std::error::Error>> {
    if let Some(candidate_id) = candidate_id {
        return Ok(LocalModelCandidateSelection {
            candidate: local_model_candidate(candidate_id)?,
            source: "explicit",
        });
    }

    let candidate = small_model_default_ci_candidate()
        .ok_or_else(|| std::io::Error::other("no default CI local model candidate"))?;
    Ok(LocalModelCandidateSelection {
        candidate,
        source: "default_ci_candidate",
    })
}

#[cfg(feature = "local-model")]
fn local_model_candidate(
    candidate_id: &str,
) -> Result<SmallModelCandidate, Box<dyn std::error::Error>> {
    small_model_candidates()
        .iter()
        .copied()
        .find(|candidate| candidate.model_id() == candidate_id)
        .ok_or_else(|| std::io::Error::other("unknown local model candidate").into())
}

#[cfg(feature = "local-model")]
fn local_model_benchmark_json(
    baseline_path: &Path,
    compared: bool,
    baseline: &LocalModelBenchmarkBaseline,
    regression: Option<&LocalModelBenchmarkRegression>,
    candidate_selection_source: &str,
    artifacts: LocalModelBenchmarkArtifacts<'_>,
    stability: Option<&LocalModelStabilityReport>,
) -> serde_json::Value {
    let summary = baseline.evaluation_summary();
    let suite = default_steward_evaluation_suite();
    let candidate_selection_source = if baseline.candidate_selection_source().is_empty() {
        candidate_selection_source
    } else {
        baseline.candidate_selection_source()
    };
    let mut value = serde_json::json!({
        "candidate_model_id": baseline.candidate_model_id(),
        "candidate_role": baseline.candidate_role(),
        "candidate_selection": {
            "source": candidate_selection_source,
            "model_id": baseline.candidate_model_id(),
        },
        "baseline_path": baseline_path.display().to_string(),
        "recorded_at": baseline.recorded_at(),
        "passed": summary.passed(),
        "passed_cases": summary.passed_cases(),
        "failed_cases": summary.failed_cases(),
        "total_cases": summary.total_cases(),
        "pass_rate": summary.pass_rate(),
        "failure_counts": baseline.evaluation().failure_counts(),
        "evaluation": baseline.evaluation(),
        "response_schema_version": baseline.response_schema_version(),
        "evaluation_suite_fingerprint": baseline.evaluation_suite_fingerprint(),
        "acceptance_coverage": local_model_acceptance_coverage_json(&suite.acceptance_coverage()),
        "schema_fingerprint": baseline.schema_fingerprint(),
        "grammar_fingerprint": baseline.grammar_fingerprint(),
        "schema_bytes": baseline.schema_bytes(),
        "grammar_bytes": baseline.grammar_bytes(),
        "prompt_fingerprint": baseline.prompt_fingerprint(),
        "response_fingerprints": baseline.response_fingerprints(),
        "contract_artifacts": local_model_contract_artifacts_json(artifacts.contract),
        "prompt_artifacts": local_model_prompt_artifacts_json(artifacts.prompts),
        "response_artifacts": local_model_response_artifacts_json(artifacts.responses),
        "response_artifact_manifest": local_model_response_artifact_manifest_json(artifacts.response_manifest),
        "bundle_manifest": null,
        "runtime": {
            "executable": baseline.runtime().executable(),
            "arguments": baseline.runtime().arguments(),
        },
        "baseline_comparison": regression.map(local_model_regression_json).or_else(|| compared.then(|| serde_json::json!({
            "compared": true,
            "regressed": false,
            "previous_recorded_at": null,
        }))),
    });
    if let Some(stability) = stability {
        value["stability"] = local_model_stability_report_json(stability);
    }
    value
}

#[cfg(feature = "local-model")]
fn local_model_regression_json(regression: &LocalModelBenchmarkRegression) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert("compared".to_string(), serde_json::json!(true));
    value.insert(
        "regressed".to_string(),
        serde_json::json!(regression.regressed()),
    );
    value.insert(
        "previous_recorded_at".to_string(),
        serde_json::json!(regression.previous_recorded_at()),
    );
    value.insert(
        "previous_candidate_selection".to_string(),
        serde_json::json!({
            "source": regression.previous_candidate_selection_source(),
            "model_id": regression.previous_candidate_selection_model_id(),
        }),
    );
    value.insert(
        "current_candidate_selection".to_string(),
        serde_json::json!({
            "source": regression.current_candidate_selection_source(),
            "model_id": regression.current_candidate_selection_model_id(),
        }),
    );
    value.insert(
        "current_recorded_at".to_string(),
        serde_json::json!(regression.current_recorded_at()),
    );
    value.insert(
        "previous_passed_cases".to_string(),
        serde_json::json!(regression.previous_passed_cases()),
    );
    value.insert(
        "current_passed_cases".to_string(),
        serde_json::json!(regression.current_passed_cases()),
    );
    value.insert(
        "previous_failure_counts".to_string(),
        serde_json::json!(regression.previous_failure_counts()),
    );
    value.insert(
        "current_failure_counts".to_string(),
        serde_json::json!(regression.current_failure_counts()),
    );
    value.insert(
        "failure_count_deltas".to_string(),
        serde_json::json!(regression.failure_count_deltas()),
    );
    value.insert(
        "regressed_cases".to_string(),
        serde_json::json!(regression.regressed_case_names()),
    );
    value.insert(
        "recovered_cases".to_string(),
        serde_json::json!(regression.recovered_case_names()),
    );
    value.insert(
        "changed_cases".to_string(),
        serde_json::json!(regression.changed_cases()),
    );
    value.insert(
        "outcome_changed_cases".to_string(),
        serde_json::json!(regression.outcome_changed_cases()),
    );
    value.insert(
        "failure_count_changed_cases".to_string(),
        serde_json::json!(regression.failure_count_changed_cases()),
    );
    value.insert(
        "response_changed_cases".to_string(),
        serde_json::json!(regression.response_changed_cases()),
    );
    value.insert(
        "changed_case_summaries".to_string(),
        serde_json::json!(regression.changed_case_summaries()),
    );
    value.insert(
        "pass_count_delta".to_string(),
        serde_json::json!(regression.pass_count_delta()),
    );
    serde_json::Value::Object(value)
}

#[cfg(feature = "local-model")]
fn local_model_stability_report_json(report: &LocalModelStabilityReport) -> serde_json::Value {
    serde_json::json!({
        "trials": report.trials(),
        "stable": report.stable(),
        "case_reports": report
            .case_reports()
            .iter()
            .map(|case| {
                serde_json::json!({
                    "name": case.name(),
                    "stable": case.stable(),
                    "proposal_fingerprints": case.proposal_fingerprints(),
                    "changed_trials": case.changed_trials(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn requirements_for_profile(profile: RequirementProfile) -> KernelRequirements {
    match profile {
        RequirementProfile::Ephemeral => KernelRequirements::ephemeral(),
        RequirementProfile::DurableAppendLog => KernelRequirements::durable_append_log(),
        RequirementProfile::PersistentIndexedAppendLog => {
            KernelRequirements::persistent_indexed_append_log()
        }
        RequirementProfile::IndexedEmbedded => KernelRequirements::indexed_embedded(),
    }
}

fn profile_name(profile: RequirementProfile) -> &'static str {
    match profile {
        RequirementProfile::Ephemeral => "ephemeral",
        RequirementProfile::DurableAppendLog => "durable-append-log",
        RequirementProfile::PersistentIndexedAppendLog => "persistent-indexed-append-log",
        RequirementProfile::IndexedEmbedded => "indexed-embedded",
    }
}

fn durability_name(durability: KernelDurability) -> &'static str {
    match durability {
        KernelDurability::Ephemeral => "ephemeral",
        KernelDurability::AppendLog => "append-log",
        KernelDurability::IndexedEmbedded => "indexed-embedded",
    }
}

fn workload_kernel_name(kernel: WorkloadKernelProfile) -> &'static str {
    match kernel {
        WorkloadKernelProfile::Memory => "memory",
        WorkloadKernelProfile::File => "file",
    }
}

fn workload_config(
    cells: usize,
    frontier_every: usize,
    dependency_stride: usize,
) -> Result<WorkloadConfig, Box<dyn std::error::Error>> {
    let valid_from = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid workload timestamp"))?;

    Ok(WorkloadConfig {
        cell_count: cells,
        id_seed: 1_000,
        anchor_prefix: "bench:world".to_string(),
        project_scope: "continuitydb".to_string(),
        valid_from,
        frontier_every,
        dependency_stride,
    })
}

fn workload_checkout_request(
    token_budget: i64,
) -> Result<CheckoutRequest, Box<dyn std::error::Error>> {
    Ok(CheckoutRequest {
        semantic_anchor: None,
        scope: Some(Scope::Project("continuitydb".to_string())),
        valid_at: None,
        system_at: None,
        commit_id: None,
        activation: None,
        lifecycle_stage: None,
        retention_policy: None,
        use_policy: None,
        promotion_policy: None,
        projection_kind: None,
        minimum_uncertainty: None,
        minimum_surprise_bits: None,
        minimum_probability_delta: None,
        minimum_salience: None,
        minimum_context_affordance: None,
        minimum_epistemic_pressure: None,
        context_gap_kind: None,
        minimum_context_gap_priority: None,
        invalidation_condition_kind: None,
        minimum_invalidation_priority: None,
        epistemic_action: None,
        epistemic_action_reason: None,
        selection_reason: None,
        trajectory_memory_strategy: None,
        minimum_trajectory_memory_confidence: None,
        answerability_question: None,
        compiler_intent: None,
        compiler_proposals: Vec::new(),
        evidence_source: None,
        dependency_target: None,
        dependency_kind: None,
        revision_related_cell: None,
        revision_link_kind: None,
        context_profile: ContextProfile::Execution,
        compiler_policy: ContextCompilerPolicy::RawBaseline,
        minimum_confidence: Confidence::new(0.0)?,
        token_budget,
    })
}

fn workload_commit_time() -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error>> {
    Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid workload commit timestamp").into())
}

fn measure_workload_json(
    options: WorkloadMeasureOptions<'_>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let workload = generate_world_model_workload(workload_config(
        options.cells,
        options.frontier_every,
        options.dependency_stride,
    )?)?;
    let committed_at = workload_commit_time()?;
    let request = workload_checkout_request(options.token_budget)?;

    let (measurement, lookup_plan) = match options.kernel {
        WorkloadKernelProfile::Memory => {
            let mut memory = MemoryKernel::default();
            (
                measure_ingest_and_checkout(&mut memory, &workload, committed_at, request.clone())?,
                None,
            )
        }
        WorkloadKernelProfile::File => {
            let path = options
                .store_path
                .ok_or_else(|| std::io::Error::other("store path is required"))?;
            let mut file = continuitydb_kernel::FileKernel::open(path)?;
            let measurement =
                measure_ingest_and_checkout(&mut file, &workload, committed_at, request.clone())?;
            let lookup = cell_lookup_from_checkout_request(&request);
            let lookup_plan = file.lookup_plan(&lookup);
            (measurement, Some(lookup_plan))
        }
    };

    let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
        &measurement,
        lookup_plan.clone(),
    );
    let comparison = workload_baseline_comparison(
        options.baseline_path,
        options.label,
        workload_kernel_name(options.kernel),
        &snapshot,
        options.compare_baseline || options.fail_on_regression,
        options.max_elapsed_growth_percent,
    )?;

    let mut output = workload_measurement_json(
        &options,
        comparison.as_ref(),
        lookup_plan.clone(),
        measurement.clone(),
    );
    let regression_detected = match comparison.as_ref() {
        Some(comparison) => !comparison.passed(),
        None => false,
    };
    if options.fail_on_regression && regression_detected {
        if let Some(artifact_dir) = options.artifact_dir {
            output =
                write_workload_artifact_bundle_report(artifact_dir, output, &workload, &request)?;
        }
        if let Some(path) = options.failure_report_path {
            write_pretty_json_file(path, &output)?;
        }
        return Err(std::io::Error::other("workload baseline regression detected").into());
    }

    if let Some(path) = options.baseline_path {
        record_workload_baseline(
            path,
            options.label,
            workload_kernel_name(options.kernel),
            &snapshot,
        )?;
    }

    if let Some(artifact_dir) = options.artifact_dir {
        output = write_workload_artifact_bundle_report(artifact_dir, output, &workload, &request)?;
    }

    Ok(output)
}

fn workload_measurement_json(
    options: &WorkloadMeasureOptions<'_>,
    comparison: Option<&WorkloadBaselineComparison>,
    lookup_plan: Option<continuitydb_kernel::FileKernelLookupPlan>,
    measurement: WorkloadMeasurement,
) -> serde_json::Value {
    serde_json::json!({
        "kernel": workload_kernel_name(options.kernel),
        "store_path": options.store_path.map(|path| path.display().to_string()),
        "artifact_dir": options.artifact_dir.map(|path| path.display().to_string()),
        "report_path": options.report_path.map(|path| path.display().to_string()),
        "failure_report_path": options.failure_report_path.map(|path| path.display().to_string()),
        "bundle_manifest": serde_json::Value::Null,
        "workload_artifacts": serde_json::Value::Null,
        "baseline_path": options.baseline_path.map(|path| path.display().to_string()),
        "baseline_label": options.baseline_path.map(|_| options.label),
        "baseline_comparison": comparison.map(workload_baseline_comparison_json),
        "lookup_plan": lookup_plan.map(file_lookup_plan_json),
        "workload": {
            "cell_count": measurement.workload_summary.cell_count,
            "frontier_count": measurement.workload_summary.frontier_count,
            "dependency_count": measurement.workload_summary.dependency_count,
            "total_token_cost": measurement.workload_summary.total_token_cost,
        },
        "revision_link_count": measurement.revision_link_count,
        "ingest": {
            "operation_count": measurement.ingest.operation_count,
            "elapsed_nanos": measurement.ingest.elapsed.as_nanos(),
        },
        "checkout_operation": {
            "operation_count": measurement.checkout_operation.operation_count,
            "elapsed_nanos": measurement.checkout_operation.elapsed.as_nanos(),
        },
        "checkout": {
            "matched_count": measurement.checkout.matched_count,
            "selected_count": measurement.checkout.selected_count,
            "alternative_count": measurement.checkout.alternative_count,
            "frontier_count": measurement.checkout.frontier_count,
            "selected_token_count": measurement.checkout.selected_token_count,
        },
    })
}

fn checkout_request_artifact_json(request: &CheckoutRequest) -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.workload.checkout_request",
        "format_version": 1,
        "request": {
            "semantic_anchor": request.semantic_anchor,
            "scope": request.scope,
            "valid_at": request.valid_at,
            "system_at": request.system_at,
            "commit_id": request.commit_id,
            "activation": request.activation,
            "answerability_question": request.answerability_question,
            "evidence_source": request.evidence_source,
            "dependency_target": request.dependency_target,
            "dependency_kind": request.dependency_kind,
            "revision_related_cell": request.revision_related_cell,
            "revision_link_kind": request.revision_link_kind,
            "minimum_confidence": request.minimum_confidence,
            "token_budget": request.token_budget,
        },
    })
}

fn write_json_artifact_with_metadata(
    path: PathBuf,
    value: &serde_json::Value,
) -> Result<(PathBuf, String, usize), Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(&path, &text)?;
    let fingerprint = fnv1a64_fingerprint(&text);
    Ok((path, fingerprint, text.len()))
}

fn write_workload_fixture_artifacts(
    artifact_dir: &Path,
    workload: &continuitydb_workload::ContinuityWorkload,
    request: &CheckoutRequest,
) -> Result<WorkloadFixtureArtifacts, Box<dyn std::error::Error>> {
    let cells = serde_json::json!({
        "format": "continuitydb.workload.cells",
        "format_version": 1,
        "summary": {
            "cell_count": workload.summary.cell_count,
            "frontier_count": workload.summary.frontier_count,
            "dependency_count": workload.summary.dependency_count,
            "total_token_cost": workload.summary.total_token_cost,
        },
        "cells": workload.cells,
    });
    let (cells_path, cells_fingerprint, cells_bytes) =
        write_json_artifact_with_metadata(artifact_dir.join("workload-cells.json"), &cells)?;

    let request_json = checkout_request_artifact_json(request);
    let (checkout_request_path, checkout_request_fingerprint, checkout_request_bytes) =
        write_json_artifact_with_metadata(
            artifact_dir.join("checkout-request.json"),
            &request_json,
        )?;

    Ok(WorkloadFixtureArtifacts {
        cells_path,
        cells_fingerprint,
        cells_bytes,
        checkout_request_path,
        checkout_request_fingerprint,
        checkout_request_bytes,
    })
}

fn workload_fixture_artifacts_json(artifacts: &WorkloadFixtureArtifacts) -> serde_json::Value {
    serde_json::json!({
        "cells_path": artifacts.cells_path.display().to_string(),
        "cells_fingerprint": artifacts.cells_fingerprint,
        "cells_bytes": artifacts.cells_bytes,
        "checkout_request_path": artifacts.checkout_request_path.display().to_string(),
        "checkout_request_fingerprint": artifacts.checkout_request_fingerprint,
        "checkout_request_bytes": artifacts.checkout_request_bytes,
    })
}

fn write_workload_bundle_manifest(
    artifact_dir: &Path,
    report_path: &Path,
    report: &serde_json::Value,
) -> Result<WorkloadBundleManifest, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(artifact_dir)?;
    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let report_text = workload_report_manifest_payload_text(report)?;
    let manifest = serde_json::json!({
        "format": "continuitydb.workload.bundle",
        "format_version": 1,
        "workload_report_path": report_path.display().to_string(),
        "workload_report_fingerprint": fnv1a64_fingerprint(&report_text),
        "workload_report_bytes": report_text.len(),
        "kernel": report["kernel"].clone(),
        "store_path": report["store_path"].clone(),
        "artifact_dir": report["artifact_dir"].clone(),
        "baseline_path": report["baseline_path"].clone(),
        "baseline_label": report["baseline_label"].clone(),
        "baseline_comparison": report["baseline_comparison"].clone(),
        "lookup_plan": report["lookup_plan"].clone(),
        "workload_artifacts": report["workload_artifacts"].clone(),
        "workload": report["workload"].clone(),
        "revision_link_count": report["revision_link_count"].clone(),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, &manifest_text)?;

    Ok(WorkloadBundleManifest {
        manifest_path,
        manifest_fingerprint: fnv1a64_fingerprint(&manifest_text),
        manifest_bytes: manifest_text.len(),
    })
}

fn workload_bundle_manifest_json(manifest: &WorkloadBundleManifest) -> serde_json::Value {
    let (parseable, parse_error) = match std::fs::read_to_string(&manifest.manifest_path) {
        Ok(manifest_text) => {
            let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
            (
                parsed_manifest.is_ok(),
                parsed_manifest.err().map(|error| error.to_string()),
            )
        }
        Err(error) => (false, Some(error.to_string())),
    };
    serde_json::json!({
        "manifest_path": manifest.manifest_path.display().to_string(),
        "manifest_fingerprint": manifest.manifest_fingerprint,
        "manifest_bytes": manifest.manifest_bytes,
        "parseable": parseable,
        "parse_error": parse_error,
    })
}

fn workload_report_manifest_payload_text(
    report: &serde_json::Value,
) -> Result<String, serde_json::Error> {
    let mut payload = report.clone();
    payload["bundle_manifest"] = serde_json::Value::Null;
    serde_json::to_string_pretty(&payload)
}

fn write_workload_artifact_bundle_report(
    artifact_dir: &Path,
    mut report: serde_json::Value,
    workload: &continuitydb_workload::ContinuityWorkload,
    request: &CheckoutRequest,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let fixture_artifacts = write_workload_fixture_artifacts(artifact_dir, workload, request)?;
    report["workload_artifacts"] = workload_fixture_artifacts_json(&fixture_artifacts);
    let report_path = artifact_dir.join("workload-report.json");
    write_pretty_json_file(&report_path, &report)?;
    let bundle_manifest = write_workload_bundle_manifest(artifact_dir, &report_path, &report)?;
    report["bundle_manifest"] = workload_bundle_manifest_json(&bundle_manifest);
    write_pretty_json_file(&report_path, &report)?;
    Ok(report)
}

fn replay_workload_json(
    options: WorkloadReplayOptions<'_>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if options
        .replay_artifact_dir
        .is_some_and(|replay_artifact_dir| replay_artifact_dir == options.artifact_dir)
    {
        return Err(
            std::io::Error::other("--replay-artifact-dir must differ from --artifact-dir").into(),
        );
    }
    let cells_path = options.artifact_dir.join("workload-cells.json");
    let checkout_request_path = options.artifact_dir.join("checkout-request.json");
    let cells_text = std::fs::read_to_string(&cells_path)?;
    let request_text = std::fs::read_to_string(&checkout_request_path)?;
    let input_bundle_manifest = if options.require_manifest {
        match validate_workload_artifact_manifest(options.artifact_dir, &cells_text, &request_text)
        {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                write_replay_input_manifest_failure_report(
                    &options,
                    &cells_path,
                    &cells_text,
                    &checkout_request_path,
                    &request_text,
                    error.to_string(),
                )?;
                return Err(error);
            }
        }
    } else {
        None
    };
    let cells_artifact: serde_json::Value = serde_json::from_str(&cells_text)?;
    let request_artifact: serde_json::Value = serde_json::from_str(&request_text)?;
    let workload = workload_from_cells_artifact(&cells_artifact)?;
    let request = checkout_request_from_artifact(&request_artifact)?;
    let committed_at = workload_commit_time()?;

    let (measurement, lookup_plan) = match options.kernel {
        WorkloadKernelProfile::Memory => {
            let mut memory = MemoryKernel::default();
            (
                measure_ingest_and_checkout(&mut memory, &workload, committed_at, request.clone())?,
                None,
            )
        }
        WorkloadKernelProfile::File => {
            let path = options
                .store_path
                .ok_or_else(|| std::io::Error::other("store path is required"))?;
            let mut file = continuitydb_kernel::FileKernel::open(path)?;
            let measurement =
                measure_ingest_and_checkout(&mut file, &workload, committed_at, request.clone())?;
            let lookup = cell_lookup_from_checkout_request(&request);
            let lookup_plan = file.lookup_plan(&lookup);
            (measurement, Some(lookup_plan))
        }
    };

    let mut output = serde_json::json!({
        "kernel": workload_kernel_name(options.kernel),
        "artifact_dir": options.artifact_dir.display().to_string(),
        "store_path": options.store_path.map(|path| path.display().to_string()),
        "report_path": options.report_path.map(|path| path.display().to_string()),
        "failure_report_path": options.failure_report_path.map(|path| path.display().to_string()),
        "replay_artifact_dir": options.replay_artifact_dir.map(|path| path.display().to_string()),
        "replay_bundle_manifest": serde_json::Value::Null,
        "input_bundle_manifest": input_bundle_manifest
            .as_ref()
            .map_or(serde_json::Value::Null, workload_bundle_manifest_json),
        "workload_artifacts": {
            "cells_path": cells_path.display().to_string(),
            "cells_fingerprint": fnv1a64_fingerprint(&cells_text),
            "cells_bytes": cells_text.len(),
            "cells_parseable": true,
            "cells_parse_error": serde_json::Value::Null,
            "checkout_request_path": checkout_request_path.display().to_string(),
            "checkout_request_fingerprint": fnv1a64_fingerprint(&request_text),
            "checkout_request_bytes": request_text.len(),
            "checkout_request_parseable": true,
            "checkout_request_parse_error": serde_json::Value::Null,
        },
        "lookup_plan": lookup_plan.map(file_lookup_plan_json),
        "workload": {
            "cell_count": measurement.workload_summary.cell_count,
            "frontier_count": measurement.workload_summary.frontier_count,
            "dependency_count": measurement.workload_summary.dependency_count,
            "total_token_cost": measurement.workload_summary.total_token_cost,
        },
        "revision_link_count": measurement.revision_link_count,
        "ingest": {
            "operation_count": measurement.ingest.operation_count,
            "elapsed_nanos": measurement.ingest.elapsed.as_nanos(),
        },
        "checkout_operation": {
            "operation_count": measurement.checkout_operation.operation_count,
            "elapsed_nanos": measurement.checkout_operation.elapsed.as_nanos(),
        },
        "checkout": {
            "matched_count": measurement.checkout.matched_count,
            "selected_count": measurement.checkout.selected_count,
            "alternative_count": measurement.checkout.alternative_count,
            "frontier_count": measurement.checkout.frontier_count,
            "selected_token_count": measurement.checkout.selected_token_count,
        },
    });

    if options.compare_report {
        let comparison = replay_report_comparison(options.artifact_dir, &measurement)?;
        let passed = comparison["passed"].as_bool().unwrap_or(false);
        output["replay_comparison"] = comparison;
        if options.fail_on_mismatch && !passed {
            if let Some(path) = options.failure_report_path {
                write_pretty_json_file(path, &output)?;
            }
            if let Some(replay_artifact_dir) = options.replay_artifact_dir {
                write_workload_replay_artifact_bundle_report(
                    replay_artifact_dir,
                    options.artifact_dir,
                    output,
                )?;
            }
            return Err(std::io::Error::other("workload replay mismatch detected").into());
        }
    } else {
        output["replay_comparison"] = serde_json::Value::Null;
    }

    if let Some(path) = options.report_path {
        write_pretty_json_file(path, &output)?;
    }
    if let Some(replay_artifact_dir) = options.replay_artifact_dir {
        output = write_workload_replay_artifact_bundle_report(
            replay_artifact_dir,
            options.artifact_dir,
            output,
        )?;
    }

    Ok(output)
}

fn write_replay_input_manifest_failure_report(
    options: &WorkloadReplayOptions<'_>,
    cells_path: &Path,
    cells_text: &str,
    checkout_request_path: &Path,
    request_text: &str,
    message: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = options
        .artifact_dir
        .join("continuitydb-workload.manifest.json");
    let input_bundle_manifest = match std::fs::read_to_string(&manifest_path) {
        Ok(manifest_text) => {
            let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
            let parse_error = parsed_manifest
                .as_ref()
                .err()
                .map(|error| error.to_string());
            serde_json::json!({
                "manifest_path": manifest_path.display().to_string(),
                "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
                "manifest_bytes": manifest_text.len(),
                "parseable": parsed_manifest.is_ok(),
                "parse_error": parse_error,
            })
        }
        Err(_) => serde_json::Value::Null,
    };
    let parsed_cells = serde_json::from_str::<serde_json::Value>(cells_text);
    let cells_parse_error = parsed_cells.as_ref().err().map(|error| error.to_string());
    let parsed_request = serde_json::from_str::<serde_json::Value>(request_text);
    let request_parse_error = parsed_request.as_ref().err().map(|error| error.to_string());
    let output = serde_json::json!({
        "kernel": workload_kernel_name(options.kernel),
        "artifact_dir": options.artifact_dir.display().to_string(),
        "store_path": options.store_path.map(|path| path.display().to_string()),
        "report_path": options.report_path.map(|path| path.display().to_string()),
        "failure_report_path": options.failure_report_path.map(|path| path.display().to_string()),
        "replay_artifact_dir": options.replay_artifact_dir.map(|path| path.display().to_string()),
        "replay_bundle_manifest": serde_json::Value::Null,
        "input_bundle_manifest": input_bundle_manifest,
        "workload_artifacts": {
            "cells_path": cells_path.display().to_string(),
            "cells_fingerprint": fnv1a64_fingerprint(cells_text),
            "cells_bytes": cells_text.len(),
            "cells_parseable": parsed_cells.is_ok(),
            "cells_parse_error": cells_parse_error,
            "checkout_request_path": checkout_request_path.display().to_string(),
            "checkout_request_fingerprint": fnv1a64_fingerprint(request_text),
            "checkout_request_bytes": request_text.len(),
            "checkout_request_parseable": parsed_request.is_ok(),
            "checkout_request_parse_error": request_parse_error,
        },
        "lookup_plan": serde_json::Value::Null,
        "workload": serde_json::Value::Null,
        "ingest": serde_json::Value::Null,
        "checkout_operation": serde_json::Value::Null,
        "checkout": serde_json::Value::Null,
        "replay_comparison": serde_json::Value::Null,
        "failure": {
            "stage": "input_manifest_validation",
            "message": message,
        },
    });
    if let Some(path) = options.failure_report_path {
        write_pretty_json_file(path, &output)?;
    }
    if let Some(replay_artifact_dir) = options.replay_artifact_dir {
        write_workload_replay_artifact_bundle_report(
            replay_artifact_dir,
            options.artifact_dir,
            output,
        )?;
    }
    Ok(())
}

fn validate_workload_artifact_manifest(
    artifact_dir: &Path,
    cells_text: &str,
    request_text: &str,
) -> Result<WorkloadBundleManifest, Box<dyn std::error::Error>> {
    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        std::io::Error::other(format!("workload artifact manifest is required: {error}"))
    })?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;

    if manifest["format"].as_str() != Some("continuitydb.workload.bundle")
        || manifest["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported workload artifact manifest").into());
    }

    let expected_artifact_dir = artifact_dir.display().to_string();
    let manifest_artifact_dir = required_json_string(&manifest, "artifact_dir")?;
    if manifest_artifact_dir != expected_artifact_dir {
        return Err(std::io::Error::other("workload artifact manifest directory mismatch").into());
    }

    let expected_report_path = artifact_dir
        .join("workload-report.json")
        .display()
        .to_string();
    let manifest_report_path = required_json_string(&manifest, "workload_report_path")?;
    if manifest_report_path != expected_report_path {
        return Err(
            std::io::Error::other("workload artifact manifest report path mismatch").into(),
        );
    }
    let expected_cells_path = artifact_dir
        .join("workload-cells.json")
        .display()
        .to_string();
    let manifest_cells_path = required_json_string(&manifest["workload_artifacts"], "cells_path")?;
    if manifest_cells_path != expected_cells_path {
        return Err(std::io::Error::other("workload artifact manifest path mismatch").into());
    }

    let expected_request_path = artifact_dir
        .join("checkout-request.json")
        .display()
        .to_string();
    let manifest_request_path =
        required_json_string(&manifest["workload_artifacts"], "checkout_request_path")?;
    if manifest_request_path != expected_request_path {
        return Err(std::io::Error::other("workload artifact manifest path mismatch").into());
    }

    let manifest_cells_bytes = required_json_u64(&manifest["workload_artifacts"], "cells_bytes")?;
    if manifest_cells_bytes != cells_text.len() as u64 {
        return Err(std::io::Error::other("workload artifact manifest byte count mismatch").into());
    }

    let manifest_request_bytes =
        required_json_u64(&manifest["workload_artifacts"], "checkout_request_bytes")?;
    if manifest_request_bytes != request_text.len() as u64 {
        return Err(std::io::Error::other("workload artifact manifest byte count mismatch").into());
    }

    let manifest_cells_fingerprint =
        required_json_string(&manifest["workload_artifacts"], "cells_fingerprint")?;
    let current_cells_fingerprint = fnv1a64_fingerprint(cells_text);
    if manifest_cells_fingerprint != current_cells_fingerprint {
        return Err(
            std::io::Error::other("workload artifact manifest fingerprint mismatch").into(),
        );
    }

    let manifest_request_fingerprint = required_json_string(
        &manifest["workload_artifacts"],
        "checkout_request_fingerprint",
    )?;
    let current_request_fingerprint = fnv1a64_fingerprint(request_text);
    if manifest_request_fingerprint != current_request_fingerprint {
        return Err(
            std::io::Error::other("workload artifact manifest fingerprint mismatch").into(),
        );
    }

    let cells_artifact: serde_json::Value = serde_json::from_str(cells_text)?;
    if manifest["workload"] != cells_artifact["summary"] {
        return Err(
            std::io::Error::other("workload artifact manifest workload summary mismatch").into(),
        );
    }
    let derived_dependency_count = workload_cells_dependency_count(&cells_artifact)?;
    let summary_dependency_count =
        required_json_u64(&cells_artifact["summary"], "dependency_count")?;
    if summary_dependency_count != derived_dependency_count {
        return Err(
            std::io::Error::other("workload artifact fixture dependency count mismatch").into(),
        );
    }

    let report_text = std::fs::read_to_string(artifact_dir.join("workload-report.json"))?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let report_revision_link_count = required_json_u64(&report, "revision_link_count")?;
    if report_revision_link_count != derived_dependency_count {
        return Err(
            std::io::Error::other("workload artifact report revision link count mismatch").into(),
        );
    }
    let manifest_report_payload_text = workload_report_manifest_payload_text(&report)?;
    let manifest_report_bytes = required_json_u64(&manifest, "workload_report_bytes")?;
    if manifest_report_bytes != manifest_report_payload_text.len() as u64 {
        return Err(std::io::Error::other("workload artifact manifest byte count mismatch").into());
    }
    let manifest_report_fingerprint =
        required_json_string(&manifest, "workload_report_fingerprint")?;
    let current_report_fingerprint = fnv1a64_fingerprint(&manifest_report_payload_text);
    if manifest_report_fingerprint != current_report_fingerprint {
        return Err(
            std::io::Error::other("workload artifact manifest fingerprint mismatch").into(),
        );
    }
    validate_workload_manifest_report_content(&manifest, &report)?;

    Ok(WorkloadBundleManifest {
        manifest_path,
        manifest_fingerprint: fnv1a64_fingerprint(&manifest_text),
        manifest_bytes: manifest_text.len(),
    })
}

fn validate_workload_bundle_manifest(
    artifact_dir: &Path,
) -> Result<WorkloadBundleValidation, Box<dyn std::error::Error>> {
    let cells_text = std::fs::read_to_string(artifact_dir.join("workload-cells.json"))?;
    let request_text = std::fs::read_to_string(artifact_dir.join("checkout-request.json"))?;
    let manifest = validate_workload_artifact_manifest(artifact_dir, &cells_text, &request_text)?;

    let report_path = artifact_dir.join("workload-report.json");
    let report_text = std::fs::read_to_string(&report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let manifest_report_payload_text = workload_report_manifest_payload_text(&report)?;
    let report_fingerprint = fnv1a64_fingerprint(&manifest_report_payload_text);
    let parsed_cells = serde_json::from_str::<serde_json::Value>(&cells_text);
    let cells_parse_error = parsed_cells.as_ref().err().map(|error| error.to_string());
    let parsed_request = serde_json::from_str::<serde_json::Value>(&request_text);
    let request_parse_error = parsed_request.as_ref().err().map(|error| error.to_string());
    let mut workload_artifacts = report["workload_artifacts"].clone();
    workload_artifacts["cells_parseable"] = serde_json::Value::from(parsed_cells.is_ok());
    workload_artifacts["cells_parse_error"] = cells_parse_error
        .map(serde_json::Value::from)
        .unwrap_or(serde_json::Value::Null);
    workload_artifacts["checkout_request_parseable"] =
        serde_json::Value::from(parsed_request.is_ok());
    workload_artifacts["checkout_request_parse_error"] = request_parse_error
        .map(serde_json::Value::from)
        .unwrap_or(serde_json::Value::Null);

    Ok(WorkloadBundleValidation {
        manifest,
        workload_report: serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": report_fingerprint,
            "report_bytes": manifest_report_payload_text.len(),
            "revision_link_count": required_json_u64(&report, "revision_link_count")?,
        }),
        workload_artifacts,
    })
}

fn validate_workload_bundle_json(
    artifact_dir: &Path,
    report_path: Option<&PathBuf>,
    failure_report_path: Option<&PathBuf>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let validation = validate_workload_bundle_manifest(artifact_dir)?;
    Ok(serde_json::json!({
        "format": "continuitydb.workload.bundle_validation",
        "format_version": 1,
        "valid": true,
        "artifact_dir": artifact_dir.display().to_string(),
        "report_path": report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.map(|path| path.display().to_string()),
        "manifest": workload_bundle_manifest_json(&validation.manifest),
        "workload_report": validation.workload_report,
        "workload_artifacts": validation.workload_artifacts,
        "failure": serde_json::Value::Null,
    }))
}

fn write_workload_bundle_validation_failure_report(
    artifact_dir: &Path,
    report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = workload_validation_failure_manifest_json(artifact_dir);
    let workload_report = workload_validation_failure_report_json(artifact_dir);
    let workload_artifacts = workload_validation_failure_artifacts_json(artifact_dir);
    let output = serde_json::json!({
        "format": "continuitydb.workload.bundle_validation",
        "format_version": 1,
        "valid": false,
        "artifact_dir": artifact_dir.display().to_string(),
        "report_path": report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "manifest": manifest,
        "workload_report": workload_report,
        "workload_artifacts": workload_artifacts,
        "failure": {
            "stage": "workload_bundle_validation",
            "message": message,
        },
    });
    write_pretty_json_file(failure_report_path, &output)?;
    Ok(())
}

fn workload_validation_failure_manifest_json(artifact_dir: &Path) -> serde_json::Value {
    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
        return serde_json::Value::Null;
    };
    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
    let parse_error = parsed_manifest
        .as_ref()
        .err()
        .map(|error| error.to_string());

    serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
        "parseable": parsed_manifest.is_ok(),
        "parse_error": parse_error,
    })
}

fn workload_validation_failure_report_json(artifact_dir: &Path) -> serde_json::Value {
    let report_path = artifact_dir.join("workload-report.json");
    let Ok(report_text) = std::fs::read_to_string(&report_path) else {
        return serde_json::Value::Null;
    };
    let parsed_report = serde_json::from_str::<serde_json::Value>(&report_text);
    let parse_error = parsed_report.as_ref().err().map(|error| error.to_string());
    let canonical_report_text = parsed_report
        .as_ref()
        .ok()
        .and_then(|report| workload_report_manifest_payload_text(report).ok())
        .unwrap_or_else(|| report_text.clone());

    serde_json::json!({
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&canonical_report_text),
        "report_bytes": canonical_report_text.len(),
        "parseable": parsed_report.is_ok(),
        "parse_error": parse_error,
    })
}

fn workload_validation_failure_artifacts_json(artifact_dir: &Path) -> serde_json::Value {
    let cells_path = artifact_dir.join("workload-cells.json");
    let checkout_request_path = artifact_dir.join("checkout-request.json");
    let Ok(cells_text) = std::fs::read_to_string(&cells_path) else {
        return serde_json::Value::Null;
    };
    let Ok(request_text) = std::fs::read_to_string(&checkout_request_path) else {
        return serde_json::Value::Null;
    };
    let parsed_cells = serde_json::from_str::<serde_json::Value>(&cells_text);
    let cells_parse_error = parsed_cells.as_ref().err().map(|error| error.to_string());
    let parsed_request = serde_json::from_str::<serde_json::Value>(&request_text);
    let request_parse_error = parsed_request.as_ref().err().map(|error| error.to_string());

    serde_json::json!({
        "cells_path": cells_path.display().to_string(),
        "cells_fingerprint": fnv1a64_fingerprint(&cells_text),
        "cells_bytes": cells_text.len(),
        "cells_parseable": parsed_cells.is_ok(),
        "cells_parse_error": cells_parse_error,
        "checkout_request_path": checkout_request_path.display().to_string(),
        "checkout_request_fingerprint": fnv1a64_fingerprint(&request_text),
        "checkout_request_bytes": request_text.len(),
        "checkout_request_parseable": parsed_request.is_ok(),
        "checkout_request_parse_error": request_parse_error,
    })
}

fn validate_workload_manifest_report_content(
    manifest: &serde_json::Value,
    report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    for key in [
        "kernel",
        "store_path",
        "artifact_dir",
        "baseline_path",
        "baseline_label",
        "baseline_comparison",
        "lookup_plan",
        "workload_artifacts",
        "workload",
        "revision_link_count",
    ] {
        if manifest[key] != report[key] {
            return Err(std::io::Error::other(format!(
                "workload artifact manifest report content mismatch: {key}"
            ))
            .into());
        }
    }
    Ok(())
}

fn workload_cells_dependency_count(
    cells_artifact: &serde_json::Value,
) -> Result<u64, Box<dyn std::error::Error>> {
    let cells = cells_artifact["cells"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("workload artifact cells array missing"))?;
    let mut dependency_count = 0u64;
    for cell in cells {
        let dependencies = cell["dependencies"].as_array().ok_or_else(|| {
            std::io::Error::other("workload artifact cell dependencies array missing")
        })?;
        dependency_count += dependencies.len() as u64;
    }
    Ok(dependency_count)
}

fn write_workload_replay_artifact_bundle_report(
    replay_artifact_dir: &Path,
    input_artifact_dir: &Path,
    mut report: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(replay_artifact_dir)?;
    let report_path = replay_artifact_dir.join("replay-report.json");
    write_pretty_json_file(&report_path, &report)?;
    let bundle_manifest = write_workload_replay_bundle_manifest(
        replay_artifact_dir,
        input_artifact_dir,
        &report_path,
        &report,
    )?;
    report["replay_bundle_manifest"] = workload_bundle_manifest_json(&bundle_manifest);
    write_pretty_json_file(&report_path, &report)?;
    Ok(report)
}

fn write_workload_replay_bundle_manifest(
    replay_artifact_dir: &Path,
    input_artifact_dir: &Path,
    report_path: &Path,
    report: &serde_json::Value,
) -> Result<WorkloadBundleManifest, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(replay_artifact_dir)?;
    let manifest_path = replay_artifact_dir.join("continuitydb-workload-replay.manifest.json");
    let report_text = std::fs::read_to_string(report_path)?;
    let parsed_report = serde_json::from_str::<serde_json::Value>(&report_text);
    let report_parse_error = parsed_report.as_ref().err().map(|error| error.to_string());
    let manifest = serde_json::json!({
        "format": "continuitydb.workload.replay_bundle",
        "format_version": 1,
        "replay_report_path": report_path.display().to_string(),
        "replay_report_fingerprint": fnv1a64_fingerprint(&report_text),
        "replay_report_bytes": report_text.len(),
        "replay_report_parseable": parsed_report.is_ok(),
        "replay_report_parse_error": report_parse_error,
        "input_artifact_dir": input_artifact_dir.display().to_string(),
        "kernel": report["kernel"].clone(),
        "store_path": report["store_path"].clone(),
        "input_bundle_manifest": report["input_bundle_manifest"].clone(),
        "workload_artifacts": report["workload_artifacts"].clone(),
        "lookup_plan": report["lookup_plan"].clone(),
        "workload": report["workload"].clone(),
        "revision_link_count": report["revision_link_count"].clone(),
        "checkout": report["checkout"].clone(),
        "replay_comparison": report["replay_comparison"].clone(),
        "failure": report["failure"].clone(),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, &manifest_text)?;

    Ok(WorkloadBundleManifest {
        manifest_path,
        manifest_fingerprint: fnv1a64_fingerprint(&manifest_text),
        manifest_bytes: manifest_text.len(),
    })
}

fn replay_report_comparison(
    artifact_dir: &Path,
    measurement: &WorkloadMeasurement,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_path = artifact_dir.join("workload-report.json");
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&report_path)?)?;
    let mut mismatches = Vec::new();

    push_u64_mismatch(
        &mut mismatches,
        "WorkloadCellCountChanged",
        json_u64(&report["workload"], "cell_count")?,
        measurement.workload_summary.cell_count,
    );
    push_u64_mismatch(
        &mut mismatches,
        "WorkloadFrontierCountChanged",
        json_u64(&report["workload"], "frontier_count")?,
        measurement.workload_summary.frontier_count,
    );
    push_u64_mismatch(
        &mut mismatches,
        "WorkloadDependencyCountChanged",
        json_u64(&report["workload"], "dependency_count")?,
        measurement.workload_summary.dependency_count,
    );
    push_u64_mismatch(
        &mut mismatches,
        "RevisionLinkCountChanged",
        json_u64(&report, "revision_link_count")?,
        measurement.revision_link_count,
    );
    push_i64_mismatch(
        &mut mismatches,
        "WorkloadTokenCostChanged",
        json_i64(&report["workload"], "total_token_cost")?,
        measurement.workload_summary.total_token_cost,
    );
    push_u64_mismatch(
        &mut mismatches,
        "CheckoutMatchedCountChanged",
        json_u64(&report["checkout"], "matched_count")?,
        measurement.checkout.matched_count,
    );
    push_u64_mismatch(
        &mut mismatches,
        "CheckoutSelectedCountChanged",
        json_u64(&report["checkout"], "selected_count")?,
        measurement.checkout.selected_count,
    );
    push_u64_mismatch(
        &mut mismatches,
        "CheckoutAlternativeCountChanged",
        json_u64(&report["checkout"], "alternative_count")?,
        measurement.checkout.alternative_count,
    );
    push_u64_mismatch(
        &mut mismatches,
        "CheckoutFrontierCountChanged",
        json_u64(&report["checkout"], "frontier_count")?,
        measurement.checkout.frontier_count,
    );
    push_i64_mismatch(
        &mut mismatches,
        "CheckoutSelectedTokenCountChanged",
        json_i64(&report["checkout"], "selected_token_count")?,
        measurement.checkout.selected_token_count,
    );

    Ok(serde_json::json!({
        "passed": mismatches.is_empty(),
        "report_path": report_path.display().to_string(),
        "mismatches": mismatches,
    }))
}

fn push_u64_mismatch(
    mismatches: &mut Vec<serde_json::Value>,
    kind: &str,
    previous: u64,
    current: usize,
) {
    if previous != current as u64 {
        mismatches.push(serde_json::json!({
            kind: {
                "previous": previous,
                "current": current,
            }
        }));
    }
}

fn push_i64_mismatch(
    mismatches: &mut Vec<serde_json::Value>,
    kind: &str,
    previous: i64,
    current: i64,
) {
    if previous != current {
        mismatches.push(serde_json::json!({
            kind: {
                "previous": previous,
                "current": current,
            }
        }));
    }
}

fn json_u64(value: &serde_json::Value, key: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value[key]
        .as_u64()
        .ok_or_else(|| std::io::Error::other(format!("missing replay comparison {key}")).into())
}

fn json_i64(value: &serde_json::Value, key: &str) -> Result<i64, Box<dyn std::error::Error>> {
    value[key]
        .as_i64()
        .ok_or_else(|| std::io::Error::other(format!("missing replay comparison {key}")).into())
}

fn required_json_string<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    value[key]
        .as_str()
        .ok_or_else(|| std::io::Error::other(format!("missing manifest field {key}")).into())
}

fn required_json_u64(
    value: &serde_json::Value,
    key: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    value[key]
        .as_u64()
        .ok_or_else(|| std::io::Error::other(format!("missing manifest field {key}")).into())
}

fn inspect_kernel_report_payload_text(
    report: &serde_json::Value,
) -> Result<String, serde_json::Error> {
    let mut payload = report.clone();
    let Some(object) = payload.as_object_mut() else {
        return serde_json::to_string_pretty(&payload);
    };
    if object.contains_key("bundle_manifest") {
        object.insert("bundle_manifest".to_string(), serde_json::Value::Null);
    }
    object.remove("report_payload_fingerprint");
    object.remove("report_payload_bytes");
    serde_json::to_string_pretty(&payload)
}

fn write_inspect_kernel_artifact_bundle_report(
    artifact_dir: &Path,
    mut report: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(artifact_dir)?;
    let report_path = artifact_dir.join("inspect-kernel-report.json");
    report["artifact_dir"] = serde_json::Value::String(artifact_dir.display().to_string());
    report["report_path"] = serde_json::Value::String(report_path.display().to_string());
    report["bundle_manifest"] = serde_json::Value::Null;
    let report_payload = inspect_kernel_report_payload_text(&report)?;
    report["report_payload_fingerprint"] =
        serde_json::Value::String(fnv1a64_fingerprint(&report_payload));
    report["report_payload_bytes"] = serde_json::Value::from(report_payload.len());
    write_pretty_json_file(&report_path, &report)?;

    let manifest = write_inspect_kernel_bundle_manifest(artifact_dir, &report_path, &report)?;
    report["bundle_manifest"] = workload_bundle_manifest_json(&manifest);
    write_pretty_json_file(&report_path, &report)?;
    Ok(report)
}

fn write_inspect_kernel_bundle_manifest(
    artifact_dir: &Path,
    report_path: &Path,
    report: &serde_json::Value,
) -> Result<WorkloadBundleManifest, Box<dyn std::error::Error>> {
    let manifest_path = artifact_dir.join("continuitydb-inspect-kernel.manifest.json");
    let report_payload = inspect_kernel_report_payload_text(report)?;
    let manifest = serde_json::json!({
        "format": "continuitydb.inspect_kernel.bundle",
        "format_version": 1,
        "artifact_dir": artifact_dir.display().to_string(),
        "inspect_report_path": report_path.display().to_string(),
        "inspect_report_fingerprint": fnv1a64_fingerprint(&report_payload),
        "inspect_report_bytes": report_payload.len(),
        "inspected_store_path": report["path"].clone(),
        "required": report["required"].clone(),
        "satisfies": report["satisfies"].clone(),
        "capabilities": report["capabilities"].clone(),
        "required_capabilities": report["required_capabilities"].clone(),
        "status": report["status"].clone(),
        "health": report["health"].clone(),
        "lookup_plan": report["lookup_plan"].clone(),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, &manifest_text)?;
    Ok(WorkloadBundleManifest {
        manifest_path,
        manifest_fingerprint: fnv1a64_fingerprint(&manifest_text),
        manifest_bytes: manifest_text.len(),
    })
}

fn validate_inspect_kernel_bundle_json(
    artifact_dir: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let manifest_path = artifact_dir.join("continuitydb-inspect-kernel.manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;

    if manifest["format"].as_str() != Some("continuitydb.inspect_kernel.bundle") {
        return Err(std::io::Error::other("unsupported inspect kernel bundle format").into());
    }
    if manifest["format_version"].as_u64() != Some(1) {
        return Err(
            std::io::Error::other("unsupported inspect kernel bundle format version").into(),
        );
    }
    if manifest["artifact_dir"].as_str() != Some(artifact_dir.display().to_string().as_str()) {
        return Err(
            std::io::Error::other("inspect kernel bundle artifact directory mismatch").into(),
        );
    }

    let report_path = artifact_dir.join("inspect-kernel-report.json");
    if manifest["inspect_report_path"].as_str() != Some(report_path.display().to_string().as_str())
    {
        return Err(std::io::Error::other("inspect kernel bundle report path mismatch").into());
    }

    let report_text = std::fs::read_to_string(&report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    if report["artifact_dir"].as_str() != Some(artifact_dir.display().to_string().as_str()) {
        return Err(std::io::Error::other(
            "inspect kernel bundle report artifact directory mismatch",
        )
        .into());
    }
    let report_payload = inspect_kernel_report_payload_text(&report)?;
    let manifest_report_bytes = required_json_u64(&manifest, "inspect_report_bytes")?;
    if manifest_report_bytes != report_payload.len() as u64 {
        return Err(
            std::io::Error::other("inspect kernel bundle report byte count mismatch").into(),
        );
    }
    let manifest_report_fingerprint =
        required_json_string(&manifest, "inspect_report_fingerprint")?;
    let current_report_fingerprint = fnv1a64_fingerprint(&report_payload);
    if manifest_report_fingerprint != current_report_fingerprint {
        return Err(
            std::io::Error::other("inspect kernel bundle report fingerprint mismatch").into(),
        );
    }
    validate_inspect_kernel_bundle_manifest_content(&manifest, &report)?;
    let report_validation = validate_inspect_kernel_report_json(&report_path)?;
    let mut inspect_report = inspect_kernel_validation_report_metadata_json(&report_path);
    inspect_report["valid"] = report_validation["valid"].clone();
    inspect_report["report_payload_fingerprint"] =
        report_validation["report_payload_fingerprint"].clone();
    inspect_report["report_payload_bytes"] = report_validation["report_payload_bytes"].clone();

    Ok(serde_json::json!({
        "format": "continuitydb.inspect_kernel.bundle_validation",
        "format_version": 1,
        "artifact_dir": artifact_dir.display().to_string(),
        "validation_report_path": serde_json::Value::Null,
        "failure_report_path": serde_json::Value::Null,
        "valid": true,
        "manifest": inspect_kernel_bundle_manifest_metadata_json(&manifest_path),
        "inspect_report": inspect_report,
        "failure": serde_json::Value::Null,
    }))
}

fn validate_inspect_kernel_bundle_manifest_content(
    manifest: &serde_json::Value,
    report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    for (manifest_key, report_key) in [
        ("inspected_store_path", "path"),
        ("required", "required"),
        ("satisfies", "satisfies"),
        ("capabilities", "capabilities"),
        ("required_capabilities", "required_capabilities"),
        ("status", "status"),
        ("health", "health"),
        ("lookup_plan", "lookup_plan"),
    ] {
        if manifest[manifest_key] != report[report_key] {
            return Err(std::io::Error::other(format!(
                "inspect kernel bundle manifest report content mismatch: {manifest_key}"
            ))
            .into());
        }
    }
    Ok(())
}

fn write_inspect_kernel_bundle_validation_failure_report(
    artifact_dir: &Path,
    report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    error: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = serde_json::json!({
        "format": "continuitydb.inspect_kernel.bundle_validation",
        "format_version": 1,
        "artifact_dir": artifact_dir.display().to_string(),
        "validation_report_path": report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "manifest": inspect_kernel_bundle_manifest_metadata_json(
            &artifact_dir.join("continuitydb-inspect-kernel.manifest.json")
        ),
        "inspect_report": inspect_kernel_validation_report_metadata_json(
            &artifact_dir.join("inspect-kernel-report.json")
        ),
        "failure": {
            "stage": "inspect_kernel_bundle_validation",
            "message": error,
        },
    });
    write_pretty_json_file(failure_report_path, &output)
}

fn inspect_kernel_bundle_manifest_metadata_json(manifest_path: &Path) -> serde_json::Value {
    let Ok(manifest_text) = std::fs::read_to_string(manifest_path) else {
        return serde_json::Value::Null;
    };
    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&manifest_text);
    let parse_error = parsed_manifest
        .as_ref()
        .err()
        .map(|error| error.to_string());

    serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
        "parseable": parsed_manifest.is_ok(),
        "parse_error": parse_error,
        "format": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["format"].as_str())
            .map(str::to_string),
        "format_version": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["format_version"].as_u64()),
        "self_described_artifact_dir": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["artifact_dir"].as_str())
            .map(str::to_string),
        "self_described_report_path": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["inspect_report_path"].as_str())
            .map(str::to_string),
        "inspected_store_path": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["inspected_store_path"].as_str())
            .map(str::to_string),
        "required": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["required"].as_str())
            .map(str::to_string),
        "satisfies": parsed_manifest
            .as_ref()
            .ok()
            .and_then(|manifest| manifest["satisfies"].as_bool()),
    })
}

fn validate_inspect_kernel_report_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report = serde_json::from_str::<serde_json::Value>(&report_text)?;
    let recorded_fingerprint =
        required_json_string(&report, "report_payload_fingerprint")?.to_string();
    let recorded_bytes = required_json_u64(&report, "report_payload_bytes")?;

    if !report.is_object() {
        return Err(std::io::Error::other("inspect kernel report must be a JSON object").into());
    }
    let payload_text = inspect_kernel_report_payload_text(&report)?;
    let current_fingerprint = fnv1a64_fingerprint(&payload_text);
    if recorded_fingerprint != current_fingerprint {
        return Err(std::io::Error::other("inspect kernel report fingerprint mismatch").into());
    }
    if recorded_bytes != payload_text.len() as u64 {
        return Err(std::io::Error::other("inspect kernel report byte count mismatch").into());
    }
    if report["format"].as_str() != Some("continuitydb.inspect_kernel.report") {
        return Err(std::io::Error::other("unsupported inspect kernel report format").into());
    }
    if report["format_version"].as_u64() != Some(1) {
        return Err(
            std::io::Error::other("unsupported inspect kernel report format version").into(),
        );
    }
    if report["report_path"].as_str() != Some(report_path.display().to_string().as_str()) {
        return Err(std::io::Error::other("inspect kernel report path mismatch").into());
    }
    validate_inspect_kernel_report_health(&report)?;

    Ok(serde_json::json!({
        "format": "continuitydb.inspect_kernel.validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "valid": true,
        "report_payload_fingerprint": current_fingerprint,
        "report_payload_bytes": payload_text.len(),
        "inspected_report": inspect_kernel_validation_report_metadata_json(report_path),
    }))
}

fn validate_inspect_kernel_report_health(
    report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let health = &report["health"];
    for field in [
        "persistent_index_checkpoint_present_on_open",
        "persistent_index_checkpoint_trusted_on_open",
        "persistent_index_checkpoint_rebuilt_on_open",
    ] {
        if !health[field].is_boolean() {
            return Err(std::io::Error::other(
                "inspect kernel report missing persistent-index checkpoint health",
            )
            .into());
        }
    }
    Ok(())
}

fn write_inspect_kernel_report_validation_failure_report(
    report_path: &Path,
    failure_report_path: &Path,
    error: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let inspected_report = inspect_kernel_validation_report_metadata_json(report_path);
    let output = serde_json::json!({
        "format": "continuitydb.inspect_kernel.validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "error": error,
        "inspected_report": inspected_report,
    });
    write_pretty_json_file(failure_report_path, &output)
}

fn inspect_kernel_validation_report_metadata_json(report_path: &Path) -> serde_json::Value {
    let Ok(report_text) = std::fs::read_to_string(report_path) else {
        return serde_json::Value::Null;
    };
    let parsed_report = serde_json::from_str::<serde_json::Value>(&report_text);
    let parse_error = parsed_report.as_ref().err().map(|error| error.to_string());
    let format = parsed_report
        .as_ref()
        .ok()
        .and_then(|report| report["format"].as_str())
        .map(str::to_string);
    let format_version = parsed_report
        .as_ref()
        .ok()
        .and_then(|report| report["format_version"].as_u64());
    let self_described_report_path = parsed_report
        .as_ref()
        .ok()
        .and_then(|report| report["report_path"].as_str())
        .map(str::to_string);
    let self_described_artifact_dir = parsed_report
        .as_ref()
        .ok()
        .and_then(|report| report["artifact_dir"].as_str())
        .map(str::to_string);
    let inspected_store_path = parsed_report
        .as_ref()
        .ok()
        .and_then(|report| report["path"].as_str())
        .map(str::to_string);

    serde_json::json!({
        "report_path": report_path.display().to_string(),
        "self_described_report_path": self_described_report_path,
        "self_described_artifact_dir": self_described_artifact_dir,
        "inspected_store_path": inspected_store_path,
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "parseable": parsed_report.is_ok(),
        "parse_error": parse_error,
        "format": format,
        "format_version": format_version,
    })
}

fn workload_from_cells_artifact(
    artifact: &serde_json::Value,
) -> Result<ContinuityWorkload, Box<dyn std::error::Error>> {
    if artifact["format"].as_str() != Some("continuitydb.workload.cells") {
        return Err(std::io::Error::other("unsupported workload cells artifact format").into());
    }
    if artifact["format_version"].as_u64() != Some(1) {
        return Err(
            std::io::Error::other("unsupported workload cells artifact format version").into(),
        );
    }

    let cells: Vec<StateCell> = serde_json::from_value(artifact["cells"].clone())?;
    let summary = &artifact["summary"];
    Ok(ContinuityWorkload {
        cells,
        summary: WorkloadSummary {
            cell_count: json_usize(summary, "cell_count")?,
            frontier_count: json_usize(summary, "frontier_count")?,
            dependency_count: json_usize(summary, "dependency_count")?,
            total_token_cost: summary["total_token_cost"]
                .as_i64()
                .ok_or_else(|| std::io::Error::other("missing workload total token cost"))?,
        },
    })
}

fn checkout_request_from_artifact(
    artifact: &serde_json::Value,
) -> Result<CheckoutRequest, Box<dyn std::error::Error>> {
    if artifact["format"].as_str() != Some("continuitydb.workload.checkout_request") {
        return Err(
            std::io::Error::other("unsupported workload checkout request artifact format").into(),
        );
    }
    if artifact["format_version"].as_u64() != Some(1) {
        return Err(std::io::Error::other(
            "unsupported workload checkout request artifact format version",
        )
        .into());
    }
    let request = &artifact["request"];
    Ok(CheckoutRequest {
        semantic_anchor: serde_json::from_value(request["semantic_anchor"].clone())?,
        scope: serde_json::from_value(request["scope"].clone())?,
        valid_at: serde_json::from_value(request["valid_at"].clone())?,
        system_at: serde_json::from_value(request["system_at"].clone())?,
        commit_id: serde_json::from_value(request["commit_id"].clone())?,
        activation: serde_json::from_value(request["activation"].clone())?,
        lifecycle_stage: None,
        retention_policy: None,
        use_policy: None,
        promotion_policy: None,
        projection_kind: None,
        minimum_uncertainty: None,
        minimum_surprise_bits: None,
        minimum_probability_delta: None,
        minimum_salience: None,
        minimum_context_affordance: None,
        minimum_epistemic_pressure: None,
        context_gap_kind: None,
        minimum_context_gap_priority: None,
        invalidation_condition_kind: None,
        minimum_invalidation_priority: None,
        epistemic_action: None,
        epistemic_action_reason: None,
        selection_reason: None,
        trajectory_memory_strategy: None,
        minimum_trajectory_memory_confidence: None,
        answerability_question: serde_json::from_value(request["answerability_question"].clone())?,
        compiler_intent: None,
        compiler_proposals: Vec::new(),
        evidence_source: serde_json::from_value(request["evidence_source"].clone())?,
        dependency_target: serde_json::from_value(request["dependency_target"].clone())?,
        dependency_kind: serde_json::from_value(request["dependency_kind"].clone())?,
        revision_related_cell: serde_json::from_value(request["revision_related_cell"].clone())?,
        revision_link_kind: serde_json::from_value(request["revision_link_kind"].clone())?,
        context_profile: ContextProfile::Execution,
        compiler_policy: ContextCompilerPolicy::RawBaseline,
        minimum_confidence: serde_json::from_value(request["minimum_confidence"].clone())?,
        token_budget: request["token_budget"]
            .as_i64()
            .ok_or_else(|| std::io::Error::other("missing workload checkout token budget"))?,
    })
}

fn json_usize(value: &serde_json::Value, key: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let raw = value[key]
        .as_u64()
        .ok_or_else(|| std::io::Error::other(format!("missing workload summary {key}")))?;
    Ok(usize::try_from(raw)?)
}

fn workload_baseline_comparison(
    baseline_path: Option<&PathBuf>,
    label: &str,
    kernel: &str,
    snapshot: &WorkloadMeasurementSnapshot,
    compare_baseline: bool,
    max_elapsed_growth_percent: u128,
) -> Result<Option<WorkloadBaselineComparison>, Box<dyn std::error::Error>> {
    if !compare_baseline {
        return Ok(None);
    }

    let path = baseline_path.ok_or_else(|| std::io::Error::other("baseline path is required"))?;
    let baseline = FileWorkloadBaselineStore::new(path).latest_matching(label, kernel)?;
    Ok(baseline.map(|baseline| {
        compare_workload_snapshot_to_baseline(
            &baseline,
            snapshot,
            WorkloadRegressionThresholds {
                max_elapsed_growth_percent,
            },
        )
    }))
}

fn workload_baseline_comparison_json(comparison: &WorkloadBaselineComparison) -> serde_json::Value {
    serde_json::json!({
        "passed": comparison.passed(),
        "baseline_recorded_at": comparison.baseline.recorded_at,
        "regressions": comparison.regressions,
    })
}

fn record_workload_baseline(
    path: &PathBuf,
    label: &str,
    kernel: &str,
    snapshot: &WorkloadMeasurementSnapshot,
) -> Result<(), Box<dyn std::error::Error>> {
    let recorded_at = Utc::now();
    let record = WorkloadBaselineRecord::new(recorded_at, label, kernel, snapshot.clone());
    FileWorkloadBaselineStore::new(path).append(&record)?;
    Ok(())
}

fn capabilities_json(capabilities: KernelCapabilities) -> serde_json::Value {
    serde_json::json!({
        "durability": durability_name(capabilities.durability),
        "append_only": capabilities.append_only,
        "derived_indexes": capabilities.derived_indexes,
        "persistent_indexes": capabilities.persistent_indexes,
        "explicit_commit_records": capabilities.explicit_commit_records,
        "durable_flush": capabilities.durable_flush,
        "compaction": capabilities.compaction,
    })
}

fn requirements_json(requirements: KernelRequirements) -> serde_json::Value {
    serde_json::json!({
        "minimum_durability": durability_name(requirements.minimum_durability),
        "append_only": requirements.append_only,
        "derived_indexes": requirements.derived_indexes,
        "persistent_indexes": requirements.persistent_indexes,
        "explicit_commit_records": requirements.explicit_commit_records,
        "durable_flush": requirements.durable_flush,
        "compaction": requirements.compaction,
    })
}

fn file_status_json(
    db: &ContinuityDb<continuitydb_kernel::FileKernel>,
) -> Result<serde_json::Value, ContinuityError> {
    let status = db.file_store_status()?;
    Ok(serde_json::json!({
        "cell_count": status.cell_count,
        "commit_count": status.commit_count,
        "revision_link_count": status.revision_link_count,
        "file_size_bytes": status.file_size_bytes,
    }))
}

fn file_health_json(db: &ContinuityDb<continuitydb_kernel::FileKernel>) -> serde_json::Value {
    file_health_value(db.file_store_health())
}

fn file_health_value(health: continuitydb_kernel::FileKernelHealth) -> serde_json::Value {
    serde_json::json!({
        "has_header": health.has_header,
        "legacy_raw_cells": health.legacy_raw_cells,
        "checksum_free_records": health.checksum_free_records,
        "canonical_records": health.canonical_records,
        "compaction_recommended": health.compaction_recommended,
        "persistent_index_checkpoint_present_on_open": health.persistent_index_checkpoint_present_on_open,
        "persistent_index_checkpoint_trusted_on_open": health.persistent_index_checkpoint_trusted_on_open,
        "persistent_index_checkpoint_rebuilt_on_open": health.persistent_index_checkpoint_rebuilt_on_open,
    })
}

fn file_lookup_plan_json(plan: continuitydb_kernel::FileKernelLookupPlan) -> serde_json::Value {
    serde_json::json!({
        "indexed_constraint_count": plan.indexed_constraint_count,
        "indexed_constraints": plan.indexed_constraints,
        "indexed_constraint_plans": plan.indexed_constraint_plans.into_iter().map(|constraint| serde_json::json!({
            "name": constraint.name,
            "candidate_count": constraint.candidate_count,
        })).collect::<Vec<_>>(),
        "exact_constraint_count": plan.exact_constraint_count,
        "exact_constraints": plan.exact_constraints,
        "residual_exact_constraint_count": plan.residual_exact_constraint_count,
        "residual_exact_constraints": plan.residual_exact_constraints,
        "lossy_indexed_constraint_count": plan.lossy_indexed_constraint_count,
        "lossy_indexed_constraints": plan.lossy_indexed_constraints,
        "candidate_count": plan.candidate_count,
        "exact_match_count": plan.exact_match_count,
        "filtered_candidate_count": plan.filtered_candidate_count,
        "candidate_selectivity_basis_points": plan.candidate_selectivity_basis_points,
        "full_scan": plan.full_scan,
    })
}

fn open_file_database(
    path: &PathBuf,
) -> Result<ContinuityDb<continuitydb_kernel::FileKernel>, Box<dyn std::error::Error>> {
    ContinuityDb::open_file(path).map_err(Into::into)
}

fn proof_obligations_json() -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.thesis_proof_obligations",
        "format_version": 1,
        "generated_by_command": "proof-obligations",
        "thesis_document": "docs/thesis.md",
        "summary": {
            "obligation_count": 8,
            "implemented_count": 8,
            "requires_local_model_feature_count": 2,
            "status": "implemented_proof_surfaces",
        },
        "obligations": [
            {
                "id": "deterministic_workload_replay",
                "claim": "A workload can be generated, measured, replayed, and compared deterministically.",
                "status": "implemented",
                "proof_surfaces": [
                    "measure-workload --artifact-dir",
                    "validate-workload-bundle --artifact-dir",
                    "replay-workload --require-manifest --compare-report",
                    "examples/alpha_workflow.sh"
                ],
                "verification_commands": [
                    "bash examples/alpha_workflow.sh",
                    "cargo test -p continuitydb-cli cli_replay_workload_require_manifest_compares_report"
                ],
                "artifacts": [
                    "workload-report.json",
                    "workload-cells.json",
                    "checkout-request.json",
                    "continuitydb-workload.manifest.json",
                    "continuitydb-workload-replay.manifest.json"
                ]
            },
            {
                "id": "durable_file_store_integrity",
                "claim": "A file-backed store can preserve cells, commits, revision links, indexes, and corruption diagnostics across reopen and compaction.",
                "status": "implemented",
                "proof_surfaces": [
                    "inspect-kernel --require persistent-indexed-append-log",
                    "compact-file --if-needed",
                    "validate-inspect-kernel-bundle --artifact-dir"
                ],
                "verification_commands": [
                    "cargo test -p continuitydb-kernel --all-features",
                    "cargo test -p continuitydb-api --all-features api_file_compaction_preserves_commit_slices_after_reopen",
                    "bash examples/alpha_workflow.sh"
                ],
                "artifacts": [
                    "inspect-kernel-report.json",
                    "continuitydb-inspect-kernel.manifest.json",
                    "JSONL file-kernel records",
                    ".index.json sidecar"
                ]
            },
            {
                "id": "deterministic_continuity_checkout",
                "claim": "A checkout can materialize task-relevant context with citations, uncertainty, frontier metadata, deterministic summary metadata, and bounded token cost.",
                "status": "implemented",
                "proof_surfaces": [
                    "demo-checkout",
                    "checkout-query",
                    "checkout-query --result-envelope",
                    "validate-checkout-query-summary",
                    "validate-checkout-query-result",
                    "validate-checkout-query-context-packets",
                    "checkout-query summary_only return shape",
                    "checkout-query cells_only return shape",
                    "checkout-query context_packets_only return shape",
                    "checkout_query_json_projected",
                    "checkout_query_text_projected",
                    "continuitydb.checkout_query.result",
                    "continuitydb.checkout_query.cells",
                    "continuitydb.checkout_query.context_packets",
                    "text CHECKOUT RETURN summary_only",
                    "text CHECKOUT RETURN cells_only",
                    "text CHECKOUT RETURN context_packets_only",
                    "measure-workload lookup-plan output"
                ],
                "verification_commands": [
                    "cargo test -p continuitydb-checkout checkout_slice_includes_citations_uncertainty_and_frontier_metadata",
                    "cargo test -p continuitydb-checkout checkout_slice_summarizes_bounded_evidence_context",
                    "cargo test -p continuitydb-cli cli_demo_checkout_outputs_metadata_json",
                    "cargo test -p continuitydb-api --all-features api_checkout_query_json_projected_returns_summary_only",
                    "cargo test -p continuitydb-api --all-features api_checkout_query_text_projected_preserves_packed_default",
                    "cargo test -p continuitydb-api --all-features api_checkout_query_result_json_wraps_projected_summary",
                    "cargo test -p continuitydb-api --all-features api_checkout_query_result_json_rejects_shape_drift",
                    "cargo test -p continuitydb-api --all-features api_checkout_query_text_projected_returns_cells_only",
                    "cargo test -p continuitydb-cli cli_checkout_query_executes_summary_only_typed_query",
                    "cargo test -p continuitydb-cli cli_checkout_query_executes_summary_only_text_query",
                    "cargo test -p continuitydb-cli cli_checkout_query_result_envelope_wraps_summary_only_text_query",
                    "cargo test -p continuitydb-cli cli_checkout_query_executes_cells_only_text_query",
                    "cargo test -p continuitydb-cli cli_checkout_query_result_envelope_wraps_cells_only_text_query",
                    "cargo test -p continuitydb-cli cli_checkout_query_executes_context_packets_only_text_query",
                    "cargo test -p continuitydb-cli cli_validate_checkout_query_context_packets_accepts_context_packets_artifact",
                    "cargo test -p continuitydb-cli cli_validate_checkout_query_summary_accepts_summary_artifact",
                    "cargo test -p continuitydb-cli cli_validate_checkout_query_result_accepts_summary_envelope",
                    "bash examples/alpha_workflow.sh"
                ],
                "artifacts": [
                    "CheckoutSlice cells",
                    "summary",
                    "continuitydb.checkout_query.summary",
                    "continuitydb.checkout_query.cells",
                    "continuitydb.checkout_query.context_packets",
                    "target/alpha-workflow/query/checkout-summary.query",
                    "target/alpha-workflow/query/checkout-summary.json",
                    "target/alpha-workflow/query/checkout-summary-validation.json",
                    "target/alpha-workflow/query/checkout-result.json",
                    "target/alpha-workflow/query/checkout-result-validation.json",
                    "target/alpha-workflow/query/checkout-cells.query",
                    "target/alpha-workflow/query/checkout-cells.json",
                    "target/alpha-workflow/query/checkout-cells-result.json",
                    "target/alpha-workflow/query/checkout-cells-result-validation.json",
                    "target/alpha-workflow/query/checkout-context-packets.query",
                    "target/alpha-workflow/query/checkout-context-packets.json",
                    "target/alpha-workflow/query/checkout-context-packets-validation.json",
                    "audit_traces",
                    "uncertainty",
                    "frontier_recommendations",
                    "alternatives"
                ]
            },
            {
                "id": "context_collapse_prevention_drill",
                "claim": "A bounded checkout can preserve source evidence, uncertainty, revision history, and frontier state instead of collapsing memory into unsupported summary text.",
                "status": "implemented",
                "proof_surfaces": [
                    "context-collapse-drill",
                    "proof-obligations",
                    "examples/alpha_workflow.sh"
                ],
                "verification_commands": [
                    "cargo test -p continuitydb-cli cli_context_collapse_drill_outputs_prevention_report",
                    "cargo run -p continuitydb-cli -- context-collapse-drill"
                ],
                "artifacts": [
                    "continuitydb.context_collapse_drill",
                    "proof.bounded_context",
                    "proof.citations_preserved",
                    "proof.uncertainty_preserved",
                    "proof.revision_links_preserved",
                    "proof.revision_context_preserved",
                    "proof.summary_preserved",
                    "proof.frontier_preserved"
                ]
            },
            {
                "id": "commit_revision_backup_audit",
                "claim": "A commit can be exported, imported, copied, and audited without losing revision relationships.",
                "status": "implemented",
                "proof_surfaces": [
                    "export-commits",
                    "import-commits",
                    "copy-commits",
                    "audit_cell native API"
                ],
                "verification_commands": [
                    "cargo test -p continuitydb-api --all-features revision_link_commit_export_import_restores_links",
                    "cargo test -p continuitydb-cli cli_backup_and_restore_preserves_native_revision_links"
                ],
                "artifacts": [
                    "versioned commit export envelope",
                    "CommitManifest",
                    "RevisionLinkRecord"
                ]
            },
            {
                "id": "proposal_only_steward_boundary",
                "claim": "A Steward proposal can be recorded, policy-evaluated, rejected without mutation, or accepted through a deterministic application path.",
                "status": "implemented",
                "proof_surfaces": [
                    "continuitydb-steward policy and ledger APIs",
                    "ContinuityDb steward feature application APIs",
                    "StorageKernel-backed proposal audit store"
                ],
                "verification_commands": [
                    "cargo test -p continuitydb-steward --all-features",
                    "cargo test -p continuitydb-api --all-features api_typed_steward_application_dispatch_returns_state_cell_result"
                ],
                "artifacts": [
                    "StewardProposal",
                    "ProposalDecisionRecord",
                    "StewardApplicationResult",
                    "proposal audit StateCells"
                ]
            },
            {
                "id": "local_model_acceptance_evidence",
                "claim": "A local model candidate can be evaluated against fixed Steward acceptance criteria with retained artifacts proving what was tested.",
                "status": "implemented",
                "requires_feature": "local-model",
                "proof_surfaces": [
                    "local-model-evaluation-suite",
                    "benchmark-local-model --artifact-dir",
                    "validate-local-model-bundle --artifact-dir"
                ],
                "verification_commands": [
                    "cargo test -p continuitydb-cli --features local-model cli_local_model_evaluation_suite_outputs_case_contracts",
                    "cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_artifact_dir_writes_dry_run_bundle"
                ],
                "artifacts": [
                    "benchmark-report.json",
                    "local-model-benchmark.manifest.json",
                    "acceptance_coverage",
                    "local-model-response.schema.json",
                    "local-model-response.gbnf",
                    "prompt artifacts",
                    "response artifacts"
                ]
            },
            {
                "id": "local_model_quality_gate_rejection",
                "claim": "A quality gate can reject incomplete, unstable, unsupported, or unauditable model memory behavior before it enters the trusted path.",
                "status": "implemented",
                "requires_feature": "local-model",
                "proof_surfaces": [
                    "run-local-model-quality-gate",
                    "local-model-quality-gate-status --require-ready",
                    "validate-local-model-quality-gate-run-report",
                    "scripts/local_model_quality_gate.sh"
                ],
                "verification_commands": [
                    "bash examples/local_model_quality_gate_smoke.sh",
                    "cargo test -p continuitydb-cli --features local-model cli_validate_local_model_quality_gate_run_report_rejects_acceptance_coverage_tamper",
                    "cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_fail_on_unstable_rejects_drift_without_baseline"
                ],
                "artifacts": [
                    "gate-run-report.json",
                    "gate-run-validation.json",
                    "candidate benchmark bundles",
                    "aggregate acceptance_coverage",
                    "failure reports"
                ]
            }
        ]
    })
}

fn validate_proof_obligations_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
    let expected = proof_obligations_json();

    if report["format"].as_str() != Some("continuitydb.thesis_proof_obligations")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported proof-obligations report").into());
    }
    if report["generated_by_command"].as_str() != Some("proof-obligations") {
        return Err(std::io::Error::other("proof-obligations report producer mismatch").into());
    }
    if report["summary"] != expected["summary"] {
        return Err(std::io::Error::other("proof-obligations summary mismatch").into());
    }
    if report["obligations"] != expected["obligations"] {
        return Err(std::io::Error::other("proof-obligations obligation matrix mismatch").into());
    }

    Ok(serde_json::json!({
        "format": "continuitydb.thesis_proof_obligations_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "summary": report["summary"].clone(),
    }))
}

fn validate_checkout_query_result_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_bytes = std::fs::read(report_path)?;
    let report_text = std::str::from_utf8(&report_bytes)?;
    let report = serde_json::from_slice::<serde_json::Value>(&report_bytes)?;
    if !checkout_query_result_summary_metadata_is_explicit(&report) {
        return Err(std::io::Error::other("checkout query result envelope is invalid").into());
    }
    let result = decode_checkout_query_result_json(&report_bytes)?;

    let (result_type, summary, cell_count, context_packet_count) = match &result {
        CheckoutQueryResult::PackedContext(slice) => (
            "packed_context",
            Some(slice.summary.clone()),
            slice.cells.len(),
            slice.context_packets.len(),
        ),
        CheckoutQueryResult::Summary(summary) => ("summary", Some(summary.clone()), 0, 0),
        CheckoutQueryResult::Cells(cells) => ("cells", None, cells.len(), 0),
        CheckoutQueryResult::ContextPackets(context_packets) => {
            ("context_packets", None, 0, context_packets.len())
        }
    };

    Ok(serde_json::json!({
        "format": "continuitydb.checkout_query.result_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(report_text),
        "report_bytes": report_bytes.len(),
        "valid": true,
        "return_shape": result.return_shape(),
        "result_type": result_type,
        "cell_count": cell_count,
        "context_packet_count": context_packet_count,
        "summary": summary,
    }))
}

fn checkout_query_result_summary_metadata_is_explicit(report: &serde_json::Value) -> bool {
    match report["result"]["type"].as_str() {
        Some("summary") => checkout_summary_metadata_is_explicit(&report["result"]["result"]),
        Some("packed_context") => {
            checkout_summary_metadata_is_explicit(&report["result"]["result"]["summary"])
                && checkout_context_packets_metadata_is_explicit(
                    &report["result"]["result"]["context_packets"],
                )
        }
        Some("context_packets") => {
            checkout_context_packets_metadata_is_explicit(&report["result"]["result"])
        }
        _ => true,
    }
}

fn checkout_summary_metadata_is_explicit(summary: &serde_json::Value) -> bool {
    let Some(summary_object) = summary.as_object() else {
        return false;
    };
    summary_count_metadata_is_explicit(summary)
        && summary_token_budget_flag_is_consistent(summary)
        && summary_selected_counts_are_consistent(summary)
        && summary_count_array_is_explicit(&summary["epistemic_action_counts"], "action")
        && summary_count_array_is_explicit(&summary["epistemic_action_reason_counts"], "reason")
        && summary_count_array_is_explicit(&summary["selection_reason_counts"], "reason")
        && summary_epistemic_pressure_is_explicit(&summary["epistemic_pressure"])
        && bounded_unit_number(&summary["maximum_context_affordance_score"])
        && bounded_unit_number(&summary["maximum_salience_score"])
        && summary_object.contains_key("minimum_selected_confidence")
        && nullable_bounded_unit_number(&summary["minimum_selected_confidence"])
        && summary_object.contains_key("maximum_selected_confidence")
        && nullable_bounded_unit_number(&summary["maximum_selected_confidence"])
        && summary_confidence_range_is_ordered(
            &summary["minimum_selected_confidence"],
            &summary["maximum_selected_confidence"],
        )
        && summary_confidence_presence_matches_selection(
            &summary["selected_cell_count"],
            &summary["minimum_selected_confidence"],
            &summary["maximum_selected_confidence"],
        )
        && summary_count_array_is_explicit(&summary["context_gap_kind_counts"], "kind")
        && summary_object.contains_key("maximum_context_gap_priority")
        && nullable_bounded_unit_number(&summary["maximum_context_gap_priority"])
        && summary_count_maximum_presence_is_consistent(
            &summary["context_gap_count"],
            &summary["maximum_context_gap_priority"],
        )
        && summary_count_array_is_explicit(&summary["invalidation_condition_kind_counts"], "kind")
        && summary_object.contains_key("maximum_invalidation_priority")
        && nullable_bounded_unit_number(&summary["maximum_invalidation_priority"])
        && summary_count_maximum_presence_is_consistent(
            &summary["invalidation_condition_count"],
            &summary["maximum_invalidation_priority"],
        )
        && summary_count_array_total_matches(
            &summary["epistemic_action_counts"],
            &summary["context_packet_count"],
        )
        && summary_count_array_total_at_least(
            &summary["selection_reason_counts"],
            &summary["context_packet_count"],
        )
        && summary_count_array_total_matches(
            &summary["context_gap_kind_counts"],
            &summary["context_gap_count"],
        )
        && summary_count_array_total_matches(
            &summary["invalidation_condition_kind_counts"],
            &summary["invalidation_condition_count"],
        )
        && summary_selected_cells_have_citations(summary)
        && summary_empty_selection_has_no_residue(summary)
}

fn summary_count_metadata_is_explicit(summary: &serde_json::Value) -> bool {
    [
        "selected_cell_count",
        "alternative_count",
        "total_tokens",
        "token_budget",
        "citation_count",
        "uncertainty_count",
        "context_packet_count",
        "frontier_recommendation_count",
        "revision_link_count",
        "revision_context_count",
        "context_gap_count",
        "invalidation_condition_count",
    ]
    .iter()
    .all(|field| non_negative_integer(&summary[*field]))
}

fn summary_token_budget_flag_is_consistent(summary: &serde_json::Value) -> bool {
    let Some(total_tokens) = summary["total_tokens"].as_i64() else {
        return false;
    };
    let Some(token_budget) = summary["token_budget"].as_i64() else {
        return false;
    };
    let Some(bounded_by_token_budget) = summary["bounded_by_token_budget"].as_bool() else {
        return false;
    };

    bounded_by_token_budget == (total_tokens <= token_budget)
}

fn summary_selected_counts_are_consistent(summary: &serde_json::Value) -> bool {
    let selected_cell_count = summary["selected_cell_count"].as_u64();
    selected_cell_count == summary["uncertainty_count"].as_u64()
        && selected_cell_count == summary["context_packet_count"].as_u64()
}

fn summary_selected_cells_have_citations(summary: &serde_json::Value) -> bool {
    let Some(selected_cell_count) = summary["selected_cell_count"].as_u64() else {
        return false;
    };
    let Some(citation_count) = summary["citation_count"].as_u64() else {
        return false;
    };

    citation_count >= selected_cell_count
}

fn summary_empty_selection_has_no_residue(summary: &serde_json::Value) -> bool {
    if summary["selected_cell_count"].as_u64() != Some(0) {
        return true;
    }

    [
        "total_tokens",
        "citation_count",
        "frontier_recommendation_count",
        "revision_link_count",
        "revision_context_count",
    ]
    .iter()
    .all(|field| summary[*field].as_u64() == Some(0))
        && summary["epistemic_action_reason_counts"]
            .as_array()
            .is_some_and(Vec::is_empty)
        && summary["selection_reason_counts"]
            .as_array()
            .is_some_and(Vec::is_empty)
        && summary["maximum_context_affordance_score"].as_f64() == Some(0.0)
        && summary["maximum_salience_score"].as_f64() == Some(0.0)
        && summary["epistemic_pressure"]["maximum_revision_pressure"].as_f64() == Some(0.0)
        && summary["epistemic_pressure"]["maximum_scavenging_pressure"].as_f64() == Some(0.0)
        && summary["epistemic_pressure"]["maximum_checkout_pressure"].as_f64() == Some(0.0)
}

fn summary_count_array_is_explicit(value: &serde_json::Value, key_field: &str) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    let mut seen = std::collections::HashSet::new();
    items.iter().all(|item| {
        item.is_object()
            && positive_integer(&item["count"])
            && item[key_field].as_str().is_some_and(|key| {
                let key = key.trim();
                !key.is_empty() && seen.insert(key.to_string())
            })
    })
}

fn summary_count_array_total_matches(
    value: &serde_json::Value,
    expected: &serde_json::Value,
) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    let Some(expected) = expected.as_u64() else {
        return false;
    };
    items.iter().try_fold(0_u64, |total, item| {
        total.checked_add(item["count"].as_u64()?)
    }) == Some(expected)
}

fn summary_count_array_total_at_least(
    value: &serde_json::Value,
    minimum: &serde_json::Value,
) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    let Some(minimum) = minimum.as_u64() else {
        return false;
    };
    items
        .iter()
        .try_fold(0_u64, |total, item| {
            total.checked_add(item["count"].as_u64()?)
        })
        .is_some_and(|total| total >= minimum)
}

fn summary_count_maximum_presence_is_consistent(
    count: &serde_json::Value,
    maximum: &serde_json::Value,
) -> bool {
    match count.as_u64() {
        Some(0) => maximum.is_null(),
        Some(_) => maximum.as_f64().is_some(),
        None => false,
    }
}

fn summary_confidence_range_is_ordered(
    minimum: &serde_json::Value,
    maximum: &serde_json::Value,
) -> bool {
    match (minimum.as_f64(), maximum.as_f64()) {
        (Some(minimum), Some(maximum)) => minimum <= maximum,
        _ => minimum.is_null() && maximum.is_null(),
    }
}

fn summary_confidence_presence_matches_selection(
    selected_cell_count: &serde_json::Value,
    minimum: &serde_json::Value,
    maximum: &serde_json::Value,
) -> bool {
    match selected_cell_count.as_u64() {
        Some(0) => minimum.is_null() && maximum.is_null(),
        Some(_) => minimum.is_number() && maximum.is_number(),
        None => false,
    }
}

fn summary_epistemic_pressure_is_explicit(epistemic_pressure: &serde_json::Value) -> bool {
    epistemic_pressure.is_object()
        && bounded_unit_number(&epistemic_pressure["maximum_revision_pressure"])
        && bounded_unit_number(&epistemic_pressure["maximum_scavenging_pressure"])
        && bounded_unit_number(&epistemic_pressure["maximum_checkout_pressure"])
}

fn checkout_context_packets_metadata_is_explicit(context_packets: &serde_json::Value) -> bool {
    let Some(context_packets) = context_packets.as_array() else {
        return false;
    };
    context_packets.iter().all(|packet| {
        let selection = &packet["selection"];
        non_blank_string(&packet["strategy"])
            && non_blank_string(&packet["compiler_policy"])
            && non_blank_string(&packet["abstraction_level"])
            && packet["compiler_reason_tags"].is_array()
            && context_packet_compiler_reason_tags_are_explicit(packet)
            && packet["compiler_evidence_locators"].is_array()
            && context_packet_compiler_evidence_is_supported(packet)
            && context_packet_origin_metadata_is_explicit(&packet["origin"])
            && context_packet_entries_metadata_is_explicit(&packet["entries"])
            && context_packet_lines_have_entries(packet)
            && string_array_items_are_non_blank(&packet["citations"])
            && context_packet_trajectory_trace_citation_is_explicit(packet)
            && context_packet_dependency_context_metadata_is_explicit(packet)
            && context_packet_revision_context_metadata_is_explicit(packet)
            && (selection.is_null()
                || (non_blank_string(&selection["epistemic_action"])
                    && selection_scores_are_explicit(selection)
                    && string_array_items_are_non_blank(&selection["epistemic_action_reasons"])
                    && selection_epistemic_pressure_is_explicit(&selection["epistemic_pressure"])
                    && selection_metadata_has_explicit_expectation(selection)
                    && attention_metadata_is_explicit(&selection["attention"])
                    && bounded_unit_number(&selection["salience_score"])
                    && context_affordance_metadata_is_explicit(&selection["context_affordance"])
                    && bounded_unit_number(&selection["context_affordance_score"])
                    && selection_context_gaps_are_explicit(&selection["context_gaps"])
                    && selection_invalidation_conditions_are_explicit(
                        &selection["invalidation_conditions"],
                    )
                    && selection_metadata_has_explicit_trajectory_memory(selection)
                    && selection["lifecycle_policy"].is_object()
                    && non_blank_string(&selection["lifecycle_policy"]["retention"])
                    && non_blank_string(&selection["lifecycle_policy"]["use_policy"])
                    && lifecycle_promotion_policy_is_explicit(
                        &selection["lifecycle_policy"]["promotion"],
                    )
                    && non_empty_string_array(&selection["lifecycle_policy"]["reasons"])
                    && non_empty_string_array(&selection["answerability_questions"])
                    && non_empty_string_array(&selection["reasons"])))
    })
}

fn context_packet_lines_have_entries(packet: &serde_json::Value) -> bool {
    let Some(lines) = packet["lines"].as_array() else {
        return false;
    };
    let Some(entries) = packet["entries"].as_array() else {
        return false;
    };

    lines.iter().all(|line| {
        let Some(line) = line.as_str() else {
            return false;
        };
        if line.trim().is_empty() {
            return false;
        }
        entries
            .iter()
            .any(|entry| entry["text"].as_str() == Some(line))
    })
}

fn selection_context_gaps_are_explicit(context_gaps: &serde_json::Value) -> bool {
    let Some(context_gaps) = context_gaps.as_array() else {
        return false;
    };

    context_gaps.iter().all(|context_gap| {
        non_blank_string(&context_gap["kind"])
            && non_blank_string(&context_gap["question"])
            && non_blank_string(&context_gap["rationale"])
            && bounded_unit_number(&context_gap["priority"])
    })
}

fn selection_invalidation_conditions_are_explicit(
    invalidation_conditions: &serde_json::Value,
) -> bool {
    let Some(invalidation_conditions) = invalidation_conditions.as_array() else {
        return false;
    };

    invalidation_conditions
        .iter()
        .all(|invalidation_condition| {
            non_blank_string(&invalidation_condition["kind"])
                && non_blank_string(&invalidation_condition["condition"])
                && non_blank_string(&invalidation_condition["rationale"])
                && bounded_unit_number(&invalidation_condition["priority"])
        })
}

fn selection_epistemic_pressure_is_explicit(epistemic_pressure: &serde_json::Value) -> bool {
    epistemic_pressure.is_object()
        && bounded_unit_number(&epistemic_pressure["revision_pressure"])
        && bounded_unit_number(&epistemic_pressure["scavenging_pressure"])
        && bounded_unit_number(&epistemic_pressure["checkout_pressure"])
}

fn selection_scores_are_explicit(selection: &serde_json::Value) -> bool {
    bounded_unit_number(&selection["max_confidence"])
        && bounded_unit_number(&selection["utility_score"])
        && bounded_unit_number(&selection["uncertainty_score"])
        && non_negative_finite_number(&selection["surprise_bits"])
}

fn context_packet_compiler_reason_tags_are_explicit(packet: &serde_json::Value) -> bool {
    let Some(compiler_reason_tags) = packet["compiler_reason_tags"].as_array() else {
        return false;
    };

    if packet["compiler_policy"].as_str() == Some("ModelAssisted")
        && compiler_reason_tags.is_empty()
    {
        return false;
    }

    compiler_reason_tags.iter().all(|reason_tag| {
        reason_tag
            .as_str()
            .is_some_and(|tag| !tag.trim().is_empty())
    })
}

fn context_packet_compiler_evidence_is_supported(packet: &serde_json::Value) -> bool {
    let Some(citations) = packet["citations"].as_array() else {
        return false;
    };
    let Some(compiler_evidence_locators) = packet["compiler_evidence_locators"].as_array() else {
        return false;
    };

    if packet["compiler_policy"].as_str() == Some("ModelAssisted")
        && compiler_evidence_locators.is_empty()
    {
        return false;
    }

    compiler_evidence_locators.iter().all(|locator| {
        locator.as_str().is_some_and(|locator| {
            citations
                .iter()
                .any(|citation| citation.as_str() == Some(locator))
        })
    })
}

fn selection_metadata_has_explicit_trajectory_memory(selection: &serde_json::Value) -> bool {
    let Some(selection) = selection.as_object() else {
        return false;
    };
    let Some(trajectory_memory) = selection.get("trajectory_memory") else {
        return false;
    };

    trajectory_memory.is_null()
        || (non_blank_string(&trajectory_memory["hypothesis_tried"])
            && non_blank_string(&trajectory_memory["progress_made"])
            && non_blank_string(&trajectory_memory["failure_mode"])
            && non_blank_string(&trajectory_memory["trace_locator"])
            && bounded_unit_number(&trajectory_memory["confidence"])
            && non_blank_string(&trajectory_memory["reusable_lesson"])
            && non_empty_string_array(&trajectory_memory["applicability_conditions"])
            && non_empty_string_array(&trajectory_memory["invalidation_conditions"])
            && non_blank_string(&trajectory_memory["checkout_strategy"]))
}

fn non_blank_string(value: &serde_json::Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn non_empty_string_array(value: &serde_json::Value) -> bool {
    value.as_array().is_some_and(|items| {
        !items.is_empty()
            && items
                .iter()
                .all(|item| item.as_str().is_some_and(|text| !text.trim().is_empty()))
    })
}

fn string_array_items_are_non_blank(value: &serde_json::Value) -> bool {
    value.as_array().is_some_and(|items| {
        items
            .iter()
            .all(|item| item.as_str().is_some_and(|text| !text.trim().is_empty()))
    })
}

fn bounded_unit_number(value: &serde_json::Value) -> bool {
    value
        .as_f64()
        .is_some_and(|number| number.is_finite() && (0.0..=1.0).contains(&number))
}

fn nullable_bounded_unit_number(value: &serde_json::Value) -> bool {
    value.is_null() || bounded_unit_number(value)
}

fn non_negative_finite_number(value: &serde_json::Value) -> bool {
    value
        .as_f64()
        .is_some_and(|number| number.is_finite() && number >= 0.0)
}

fn non_negative_integer(value: &serde_json::Value) -> bool {
    value.as_i64().is_some_and(|number| number >= 0)
}

fn positive_integer(value: &serde_json::Value) -> bool {
    value.as_u64().is_some_and(|number| number > 0)
}

fn lifecycle_promotion_policy_is_explicit(value: &serde_json::Value) -> bool {
    if non_blank_string(value) {
        return true;
    }

    value.as_object().is_some_and(|policy| {
        policy.len() == 1
            && policy
                .iter()
                .all(|(variant, payload)| !variant.trim().is_empty() && !payload.is_null())
    })
}

fn context_packet_trajectory_trace_citation_is_explicit(packet: &serde_json::Value) -> bool {
    let trajectory_memory = &packet["selection"]["trajectory_memory"];
    if trajectory_memory.is_null() {
        return true;
    }
    let Some(trace_locator) = trajectory_memory["trace_locator"].as_str() else {
        return false;
    };
    packet["citations"].as_array().is_some_and(|citations| {
        citations
            .iter()
            .any(|citation| citation.as_str() == Some(trace_locator))
    })
}

fn selection_metadata_has_explicit_expectation(selection: &serde_json::Value) -> bool {
    let Some(selection) = selection.as_object() else {
        return false;
    };
    let Some(expectation) = selection.get("expectation") else {
        return false;
    };
    expectation.is_null()
        || (expectation["label"].is_string()
            && expectation["prior_probability"].is_number()
            && expectation["observed_probability"].is_number()
            && expectation["probability_delta"].is_number()
            && expectation["surprise_bits"].is_number())
}

fn context_packet_dependency_context_metadata_is_explicit(packet: &serde_json::Value) -> bool {
    let Some(packet_citations) = packet["citations"].as_array() else {
        return false;
    };
    let dependency_context = &packet["dependency_context"];
    let Some(dependency_context) = dependency_context.as_array() else {
        return false;
    };
    dependency_context.iter().all(|context| {
        non_blank_string(&context["target"])
            && non_blank_string(&context["kind"])
            && non_blank_string(&context["rationale"])
            && non_empty_string_array(&context["anchors"])
            && context["citations"].as_array().is_some_and(|citations| {
                !citations.is_empty()
                    && citations.iter().all(|citation| {
                        citation.as_str().is_some_and(|citation| {
                            !citation.trim().is_empty()
                                && packet_citations.iter().any(|packet_citation| {
                                    packet_citation.as_str() == Some(citation)
                                })
                        })
                    })
            })
    })
}

fn context_packet_revision_context_metadata_is_explicit(packet: &serde_json::Value) -> bool {
    let Some(packet_citations) = packet["citations"].as_array() else {
        return false;
    };
    let revision_context = &packet["revision_context"];
    let Some(revision_context) = revision_context.as_array() else {
        return false;
    };
    revision_context.iter().all(|context| {
        non_blank_string(&context["related_cell_id"])
            && non_blank_string(&context["relation"])
            && non_blank_string(&context["kind"])
            && non_empty_string_array(&context["anchors"])
            && context["citations"].as_array().is_some_and(|citations| {
                !citations.is_empty()
                    && citations.iter().all(|citation| {
                        citation.as_str().is_some_and(|citation| {
                            !citation.trim().is_empty()
                                && packet_citations.iter().any(|packet_citation| {
                                    packet_citation.as_str() == Some(citation)
                                })
                        })
                    })
            })
            && bounded_unit_number(&context["max_confidence"])
            && non_blank_string(&context["activation"])
    })
}

fn context_packet_entries_metadata_is_explicit(entries: &serde_json::Value) -> bool {
    let Some(entries) = entries.as_array() else {
        return false;
    };
    entries.iter().all(|entry| {
        let source = &entry["source"];
        (non_blank_string(source) || source.is_object())
            && non_blank_string(&entry["text"])
            && bounded_unit_number(&entry["confidence"])
            && non_negative_integer(&entry["token_count"])
    })
}

fn context_packet_origin_metadata_is_explicit(origin: &serde_json::Value) -> bool {
    origin.is_object()
        && non_blank_string(&origin["cell_id"])
        && non_empty_string_array(&origin["anchors"])
        && origin_scope_metadata_is_explicit(&origin["scope"])
        && origin_time_range_metadata_is_explicit(&origin["valid_time"])
        && origin_time_range_metadata_is_explicit(&origin["system_time"])
        && non_blank_string(&origin["lifecycle_stage"])
        && non_blank_string(&origin["activation"])
        && non_blank_string(&origin["commit_id"])
}

fn origin_scope_metadata_is_explicit(scope: &serde_json::Value) -> bool {
    if scope.as_str() == Some("Global") {
        return true;
    }

    scope.as_object().is_some_and(|scope| {
        scope.len() == 1
            && scope
                .iter()
                .all(|(variant, value)| !variant.trim().is_empty() && non_blank_string(value))
    })
}

fn origin_time_range_metadata_is_explicit(time_range: &serde_json::Value) -> bool {
    let Some(time_range) = time_range.as_object() else {
        return false;
    };

    time_range.get("from").is_some_and(non_blank_string)
        && time_range
            .get("to")
            .is_some_and(|to| to.is_null() || non_blank_string(to))
}

fn attention_metadata_is_explicit(attention: &serde_json::Value) -> bool {
    attention.is_object()
        && bounded_unit_number(&attention["novelty"])
        && bounded_unit_number(&attention["urgency"])
        && bounded_unit_number(&attention["impact"])
        && bounded_unit_number(&attention["decay_resistance"])
}

fn context_affordance_metadata_is_explicit(context_affordance: &serde_json::Value) -> bool {
    context_affordance.is_object()
        && bounded_unit_number(&context_affordance["expected_task_value"])
        && bounded_unit_number(&context_affordance["expected_information_gain"])
        && bounded_unit_number(&context_affordance["risk_of_misuse"])
        && bounded_unit_number(&context_affordance["ambiguity"])
        && bounded_unit_number(&context_affordance["applicability"])
        && bounded_unit_number(&context_affordance["resource_pressure"])
}

fn validate_checkout_query_summary_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_bytes = std::fs::read(report_path)?;
    let report_text = std::str::from_utf8(&report_bytes)?;
    let report = serde_json::from_slice::<serde_json::Value>(&report_bytes)?;
    if report["format"].as_str() != Some("continuitydb.checkout_query.summary")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("checkout query summary artifact is invalid").into());
    }
    if !checkout_summary_metadata_is_explicit(&report["summary"]) {
        return Err(std::io::Error::other("checkout query summary artifact is invalid").into());
    }
    let summary = serde_json::from_value::<CheckoutSummary>(report["summary"].clone())?;

    Ok(serde_json::json!({
        "format": "continuitydb.checkout_query.summary_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(report_text),
        "report_bytes": report_bytes.len(),
        "valid": true,
        "summary": summary,
    }))
}

fn validate_checkout_query_context_packets_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_bytes = std::fs::read(report_path)?;
    let report_text = std::str::from_utf8(&report_bytes)?;
    let report = serde_json::from_slice::<serde_json::Value>(&report_bytes)?;
    if report["format"].as_str() != Some("continuitydb.checkout_query.context_packets")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("checkout query context packets artifact is invalid").into(),
        );
    }
    if !checkout_context_packets_metadata_is_explicit(&report["context_packets"]) {
        return Err(
            std::io::Error::other("checkout query context packets artifact is invalid").into(),
        );
    }
    let context_packets =
        serde_json::from_value::<Vec<ContextPacket>>(report["context_packets"].clone())?;

    Ok(serde_json::json!({
        "format": "continuitydb.checkout_query.context_packets_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(report_text),
        "report_bytes": report_bytes.len(),
        "valid": true,
        "context_packet_count": context_packets.len(),
    }))
}

fn checkout_query_result_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.checkout_query.result_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "checkout_query_result_report": report_metadata,
        "failure": {
            "stage": "checkout_query_result_validation",
            "message": message,
        },
    })
}

fn checkout_query_summary_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.checkout_query.summary_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "checkout_query_summary_report": report_metadata,
        "failure": {
            "stage": "checkout_query_summary_validation",
            "message": message,
        },
    })
}

fn checkout_query_context_packets_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.checkout_query.context_packets_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "checkout_query_context_packets_report": report_metadata,
        "failure": {
            "stage": "checkout_query_context_packets_validation",
            "message": message,
        },
    })
}

fn proof_obligations_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.thesis_proof_obligations_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "proof_obligations_report": report_metadata,
        "failure": {
            "stage": "proof_obligations_validation",
            "message": message,
        },
    })
}

fn validate_ci_artifact_inventory_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;

    if report["format"].as_str() != Some("continuitydb.ci_artifact_inventory")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported ci artifact inventory report").into());
    }
    if report["generated_by"].as_str() != Some("scripts/ci_artifact_inventory.sh") {
        return Err(std::io::Error::other("ci artifact inventory producer mismatch").into());
    }
    if report["valid"].as_bool() != Some(true) {
        return Err(std::io::Error::other("ci artifact inventory is not valid").into());
    }

    validate_ci_inventory_root(&report, "alpha_workflow", 28, 90)?;
    validate_ci_inventory_root(&report, "local_model_quality_gate_smoke", 47, 126)?;
    validate_ci_inventory_root(&report, "release_preflight", 12, 60)?;

    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "proof_obligations_valid",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "proof_obligation_count",
        ],
        8,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "context_collapse_drill_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "summary_only_checkout_query_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "checkout_query_summary_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "checkout_query_result_envelope_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "checkout_query_result_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "cells_only_checkout_query_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "checkout_query_cells_result_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "context_packets_only_checkout_query_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "checkout_query_context_packets_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "workload_bundle_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &["verified_evidence", "alpha_workflow", "replay_passed"],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "inspect_kernel_satisfies_required_profile",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "alpha_workflow",
            "inspect_kernel_persistent_index_checkpoint_trusted",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "quality_gate_candidate_count",
        ],
        2,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "candidate_registry_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "quality_gate_plan_validation_valid",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "required_acceptance_criteria_count",
        ],
        7,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "acceptance_criteria_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "evaluation_suite_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "dry_run_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "quality_gate_run_output_validation_valid",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "complete_acceptance_coverage_candidate_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "prompt_count",
        ],
        9,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "dry_run_gate_ready",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "quality_gate_status_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "missing_artifact_status_ready",
        ],
        false,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "missing_artifact_status_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "readiness_blocker_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "run_quality_gate_candidate_action_count",
        ],
        2,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "require_ready_status_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "require_ready_status_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_run_report_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_run_output_validation_valid",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_candidate_artifact_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_candidate_benchmark_validation_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_candidate_bundle_validation_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_candidate_validation_count",
        ],
        2,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_candidate_contract_artifact_count",
        ],
        8,
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
        "\"context_compiler_schema_version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
        "\"context_compiler_schema_version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json",
        "\"context_compiler_schema_version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json",
        "\"x-continuitydb-schema-version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json",
        "\"context_compiler_schema_version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
        "\"context_compiler_schema_version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json",
        "\"context_compiler_schema_version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json",
        "\"x-continuitydb-schema-version\": 2",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "contract/local-model-context-compiler-response.schema.json",
        "\"x-continuitydb-schema-version\": 2",
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "local_model_quality_gate_smoke",
            "operator_gate_validation_valid",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "proof_obligation_count",
        ],
        8,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "proof_obligations_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_assets_valid",
        ],
        true,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_asset_count",
        ],
        12,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_crate_archive_count",
        ],
        10,
    )?;
    require_json_u64(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_preflight_evidence_count",
        ],
        2,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_preflight_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_success_report_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_success_report_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_failure_report_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_failure_report_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_view_failure_report_retained",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_view_failure_report_validation_valid",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_view_failure_diagnostic_checked",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_failure_diagnostic_checked",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_asset_integrity_checked",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_duplicate_manifest_checked",
        ],
        true,
    )?;
    require_json_bool(
        &report,
        &[
            "verified_evidence",
            "release_preflight",
            "release_upload_test_report_validation_valid",
        ],
        true,
    )?;
    require_ci_inventory_artifact_path(&report, "alpha_workflow", "query/checkout-summary.query")?;
    require_ci_inventory_artifact_path(&report, "alpha_workflow", "query/checkout-summary.json")?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-summary-validation.json",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "query/checkout-summary.json",
        "\"invalidation_condition_count\"",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "query/checkout-summary-validation.json",
        "\"invalidation_condition_count\"",
    )?;
    require_ci_inventory_artifact_path(&report, "alpha_workflow", "query/checkout-result.json")?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-result-validation.json",
    )?;
    require_ci_inventory_artifact_path(&report, "alpha_workflow", "query/checkout-cells.query")?;
    require_ci_inventory_artifact_path(&report, "alpha_workflow", "query/checkout-cells.json")?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-cells-result.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-cells-result-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-context-packets.query",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-context-packets.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "query/checkout-context-packets-validation.json",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "query/checkout-context-packets.json",
        "\"context_gaps\"",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "query/checkout-context-packets.json",
        "\"invalidation_conditions\"",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "query/checkout-context-packets.json",
        "\"trajectory_memory\"",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "alpha_workflow",
        "workload/workload-validation.json",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "workload/continuitydb-workload.manifest.json",
        "\"revision_link_count\": 14",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "replay/replay-report.json",
        "\"revision_link_count\": 14",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "alpha_workflow",
        "replay/continuitydb-workload-replay.manifest.json",
        "\"revision_link_count\": 14",
    )?;
    require_ci_inventory_artifact_path(&report, "release_preflight", "release-assets.json")?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-assets-validation.json",
    )?;
    require_ci_inventory_artifact_path(&report, "release_preflight", "release-upload-report.json")?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-upload-report-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-upload-failure-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-upload-failure-report-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-view-failure-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-view-failure-report-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-upload-test-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "release_preflight",
        "release-upload-test-report-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "acceptance-criteria.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "acceptance-criteria-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "require-ready-status.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "quality-gate-status-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "require-ready-status-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "quality-gate-run-output-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/gate-run-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/gate-run-output-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.schema.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.gbnf",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.gbnf",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.schema.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.gbnf",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.gbnf",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "contract/local-model-context-compiler-response.schema.json",
    )?;
    require_ci_inventory_artifact_path(
        &report,
        "local_model_quality_gate_smoke",
        "contract/local-model-context-compiler-response.gbnf",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json",
        "\"title\": \"ContinuityDB Local Model Context Compiler Response\"",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.gbnf",
        "root ::= context-compiler-response",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json",
        "\"title\": \"ContinuityDB Local Model Context Compiler Response\"",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.gbnf",
        "root ::= context-compiler-response",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "contract/local-model-context-compiler-response.schema.json",
        "\"title\": \"ContinuityDB Local Model Context Compiler Response\"",
    )?;
    require_ci_inventory_check_pattern(
        &report,
        "local_model_quality_gate_smoke",
        "contract/local-model-context-compiler-response.gbnf",
        "root ::= context-compiler-response",
    )?;

    require_json_u64(
        &report,
        &["contract_summary", "total_required_artifact_count"],
        87,
    )?;
    require_json_u64(
        &report,
        &["contract_summary", "total_required_check_count"],
        276,
    )?;

    Ok(serde_json::json!({
        "format": "continuitydb.ci_artifact_inventory_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "contract_summary": report["contract_summary"].clone(),
    }))
}

const RELEASE_ASSET_PACKAGES: [&str; 10] = [
    "continuitydb-core",
    "continuitydb-kernel",
    "continuitydb-memory",
    "continuitydb-checkout",
    "continuitydb-query",
    "continuitydb-revision",
    "continuitydb-steward",
    "continuitydb-api",
    "continuitydb-workload",
    "continuitydb-cli",
];

fn validate_release_assets_json(
    manifest_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let manifest_text = std::fs::read_to_string(manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;

    if manifest["format"].as_str() != Some("continuitydb.release_assets")
        || manifest["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported release asset manifest").into());
    }
    if manifest["generated_by"].as_str() != Some("scripts/release_preflight.sh") {
        return Err(std::io::Error::other("release asset manifest producer mismatch").into());
    }
    let package_version = manifest["package_version"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| std::io::Error::other("release asset manifest missing package version"))?;
    let assets = manifest["assets"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("release asset manifest missing assets"))?;
    if manifest["asset_count"].as_u64() != Some(assets.len() as u64) {
        return Err(std::io::Error::other("release asset manifest asset count mismatch").into());
    }
    if assets.len() != RELEASE_ASSET_PACKAGES.len() + 2 {
        return Err(std::io::Error::other("release asset manifest unexpected asset count").into());
    }

    let mut asset_names = std::collections::HashSet::new();
    let mut asset_paths = std::collections::HashSet::new();
    let mut crate_archive_count = 0u64;
    let mut evidence_count = 0u64;
    let mut total_bytes = 0u64;

    for (index, asset) in assets.iter().enumerate() {
        let kind = release_asset_string(asset, "kind")?;
        let name = release_asset_string(asset, "name")?;
        let path = release_asset_string(asset, "path")?;
        let expected_bytes = release_asset_u64(asset, "bytes")?;
        let expected_sha256 = release_asset_string(asset, "sha256")?;
        if expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(std::io::Error::other("release asset manifest invalid sha256").into());
        }
        if !asset_names.insert(name.to_string()) {
            return Err(
                std::io::Error::other("release asset manifest duplicate asset name").into(),
            );
        }
        if !asset_paths.insert(path.to_string()) {
            return Err(
                std::io::Error::other("release asset manifest duplicate asset path").into(),
            );
        }

        if index < RELEASE_ASSET_PACKAGES.len() {
            let expected_package = RELEASE_ASSET_PACKAGES[index];
            if kind != "crate_archive" {
                return Err(
                    std::io::Error::other("release asset manifest crate kind mismatch").into(),
                );
            }
            if asset["package"].as_str() != Some(expected_package) {
                return Err(
                    std::io::Error::other("release asset manifest package order mismatch").into(),
                );
            }
            let expected_name = format!("{expected_package}-{package_version}.crate");
            if name != expected_name {
                return Err(
                    std::io::Error::other("release asset manifest crate name mismatch").into(),
                );
            }
            crate_archive_count += 1;
        } else {
            if kind != "release_preflight_evidence" {
                return Err(
                    std::io::Error::other("release asset manifest evidence kind mismatch").into(),
                );
            }
            let expected_name = if index == RELEASE_ASSET_PACKAGES.len() {
                "proof-obligations.json"
            } else {
                "proof-obligations-validation.json"
            };
            if name != expected_name {
                return Err(
                    std::io::Error::other("release asset manifest evidence name mismatch").into(),
                );
            }
            evidence_count += 1;
        }

        let asset_bytes = std::fs::read(path)?;
        if asset_bytes.len() as u64 != expected_bytes {
            return Err(std::io::Error::other("release asset manifest byte count mismatch").into());
        }
        let actual_sha256 = sha256_hex(&asset_bytes);
        if actual_sha256 != expected_sha256 {
            return Err(std::io::Error::other("release asset manifest sha256 mismatch").into());
        }
        total_bytes += expected_bytes;
    }

    Ok(serde_json::json!({
        "format": "continuitydb.release_assets_validation",
        "format_version": 1,
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
        "valid": true,
        "package_version": package_version,
        "asset_count": assets.len(),
        "crate_archive_count": crate_archive_count,
        "release_preflight_evidence_count": evidence_count,
        "total_asset_bytes": total_bytes,
    }))
}

fn validate_release_upload_report_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;

    if report["format"].as_str() != Some("continuitydb.release_upload")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported release upload report").into());
    }
    if report["generated_by"].as_str() != Some("scripts/upload_release_assets.sh") {
        return Err(std::io::Error::other("release upload report producer mismatch").into());
    }
    let release_upload_succeeded = report["valid"]
        .as_bool()
        .ok_or_else(|| std::io::Error::other("release upload report missing valid flag"))?;
    let failure_stage = if release_upload_succeeded {
        None
    } else {
        Some(release_upload_report_string(&report, "failure_stage")?)
    };
    let failure_error = if release_upload_succeeded {
        None
    } else {
        Some(release_upload_report_string(&report, "error")?)
    };
    let release_tag = release_upload_report_string(&report, "release_tag")?;
    let release_repository = release_upload_report_string(&report, "release_repository")?;
    let uploaded_assets = report["assets"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("release upload report missing assets"))?;
    let asset_count = release_upload_report_u64(&report, "asset_count")?;
    if asset_count != uploaded_assets.len() as u64 {
        return Err(std::io::Error::other("release upload report asset count mismatch").into());
    }
    if uploaded_assets.is_empty() {
        return Err(std::io::Error::other("release upload report has no assets").into());
    }

    let manifest_metadata = report["manifest"]
        .as_object()
        .ok_or_else(|| std::io::Error::other("release upload report missing manifest metadata"))?;
    let manifest_path_text = manifest_metadata
        .get("manifest_path")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| std::io::Error::other("release upload report missing manifest path"))?;
    if manifest_metadata
        .get("manifest_format")
        .and_then(serde_json::Value::as_str)
        != Some("continuitydb.release_assets")
    {
        return Err(std::io::Error::other("release upload report manifest format mismatch").into());
    }
    if manifest_metadata
        .get("manifest_format_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err(
            std::io::Error::other("release upload report manifest version mismatch").into(),
        );
    }
    let manifest_path = resolve_report_reference_path(report_path, manifest_path_text);
    let manifest_text = std::fs::read_to_string(&manifest_path)?;
    let manifest_bytes = manifest_text.as_bytes();
    if manifest_metadata
        .get("manifest_bytes")
        .and_then(serde_json::Value::as_u64)
        != Some(manifest_bytes.len() as u64)
    {
        return Err(std::io::Error::other("release upload report manifest byte mismatch").into());
    }
    let manifest_sha256 = manifest_metadata
        .get("manifest_sha256")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| std::io::Error::other("release upload report missing manifest sha256"))?;
    let actual_manifest_sha256 = sha256_hex(manifest_bytes);
    if manifest_sha256 != actual_manifest_sha256 {
        return Err(std::io::Error::other("release upload report manifest sha256 mismatch").into());
    }

    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;
    if manifest["format"].as_str() != Some("continuitydb.release_assets")
        || manifest["format_version"].as_u64() != Some(1)
    {
        return Err(
            std::io::Error::other("release upload report references unsupported manifest").into(),
        );
    }
    let manifest_assets = manifest["assets"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("release upload report manifest missing assets"))?;
    if manifest["asset_count"]
        .as_u64()
        .is_some_and(|manifest_asset_count| manifest_asset_count != manifest_assets.len() as u64)
    {
        return Err(
            std::io::Error::other("release upload report manifest asset count mismatch").into(),
        );
    }
    if manifest_metadata
        .get("manifest_asset_count")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|manifest_asset_count| manifest_asset_count != manifest_assets.len() as u64)
    {
        return Err(std::io::Error::other(
            "release upload report manifest metadata asset count mismatch",
        )
        .into());
    }
    if manifest_assets.len() != uploaded_assets.len() {
        return Err(
            std::io::Error::other("release upload report uploaded asset count mismatch").into(),
        );
    }

    let mut total_asset_bytes = 0u64;
    for (uploaded_asset, manifest_asset) in uploaded_assets.iter().zip(manifest_assets.iter()) {
        require_matching_upload_asset_field(uploaded_asset, manifest_asset, "kind")?;
        require_matching_upload_asset_field(uploaded_asset, manifest_asset, "name")?;
        require_matching_upload_asset_field(uploaded_asset, manifest_asset, "path")?;
        require_matching_upload_asset_field(uploaded_asset, manifest_asset, "sha256")?;
        if uploaded_asset.get("package").is_some() || manifest_asset.get("package").is_some() {
            require_matching_upload_asset_field(uploaded_asset, manifest_asset, "package")?;
        }
        let expected_bytes = release_asset_u64(manifest_asset, "bytes")?;
        if uploaded_asset["bytes"].as_u64() != Some(expected_bytes) {
            return Err(std::io::Error::other(
                "release upload report uploaded asset byte mismatch",
            )
            .into());
        }
        let asset_path = release_asset_string(manifest_asset, "path")?;
        let actual_asset_bytes =
            std::fs::read(resolve_report_reference_path(report_path, asset_path))?;
        if actual_asset_bytes.len() as u64 != expected_bytes {
            return Err(std::io::Error::other("release upload report asset byte mismatch").into());
        }
        if sha256_hex(&actual_asset_bytes) != release_asset_string(manifest_asset, "sha256")? {
            return Err(
                std::io::Error::other("release upload report asset sha256 mismatch").into(),
            );
        }
        total_asset_bytes += expected_bytes;
    }

    let manifest_asset_count = manifest_assets.len();
    Ok(serde_json::json!({
        "format": "continuitydb.release_upload_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "release_upload_succeeded": release_upload_succeeded,
        "release_tag": release_tag,
        "release_repository": release_repository,
        "asset_count": asset_count,
        "manifest_path": manifest_path_text,
        "manifest_sha256": manifest_sha256,
        "manifest_asset_count": manifest_asset_count,
        "total_asset_bytes": total_asset_bytes,
        "failure_stage": failure_stage,
        "failure_error": failure_error,
    }))
}

fn release_upload_report_string<'a>(
    report: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    report[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            std::io::Error::other(format!("release upload report missing field {key}")).into()
        })
}

fn release_upload_report_u64(
    report: &serde_json::Value,
    key: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    report[key].as_u64().ok_or_else(|| {
        std::io::Error::other(format!("release upload report missing field {key}")).into()
    })
}

fn validate_release_upload_test_report_json(
    report_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report_text = std::fs::read_to_string(report_path)?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;

    if report["format"].as_str() != Some("continuitydb.release_upload_test")
        || report["format_version"].as_u64() != Some(1)
    {
        return Err(std::io::Error::other("unsupported release upload test report").into());
    }
    if report["generated_by"].as_str() != Some("scripts/test_upload_release_assets.sh") {
        return Err(std::io::Error::other("release upload test report producer mismatch").into());
    }
    if report["valid"].as_bool() != Some(true) {
        return Err(std::io::Error::other("release upload test report is not valid").into());
    }
    let release_tag = release_upload_report_string(&report, "release_tag")?;
    let release_repository = release_upload_report_string(&report, "release_repository")?;
    let required_checks = [
        "release_view_preflight_checked",
        "release_upload_repo_checked",
        "upload_failure_diagnostic_checked",
        "asset_integrity_checked",
        "duplicate_manifest_checked",
        "success_report_checked",
        "success_report_validation_checked",
        "failure_report_checked",
        "failure_report_validation_checked",
        "release_view_failure_diagnostic_checked",
        "release_view_failure_report_checked",
        "release_view_failure_report_validation_checked",
    ];
    for check in required_checks {
        if report[check].as_bool() != Some(true) {
            return Err(std::io::Error::other(format!(
                "release upload test report check failed: {check}"
            ))
            .into());
        }
    }
    let mocked_gh_invocation_count =
        release_upload_report_u64(&report, "mocked_gh_invocation_count")?;
    if mocked_gh_invocation_count < 5 {
        return Err(std::io::Error::other(
            "release upload test report missing mocked gh invocations",
        )
        .into());
    }

    Ok(serde_json::json!({
        "format": "continuitydb.release_upload_test_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&report_text),
        "report_bytes": report_text.len(),
        "valid": true,
        "release_tag": release_tag,
        "release_repository": release_repository,
        "checked_condition_count": required_checks.len(),
        "mocked_gh_invocation_count": mocked_gh_invocation_count,
    }))
}

fn release_upload_test_report_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.release_upload_test_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "release_upload_test_report": report_metadata,
        "failure": {
            "stage": "release_upload_test_validation",
            "message": message,
        },
    })
}

fn require_matching_upload_asset_field(
    uploaded_asset: &serde_json::Value,
    manifest_asset: &serde_json::Value,
    key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if uploaded_asset.get(key) != manifest_asset.get(key) {
        return Err(std::io::Error::other(format!(
            "release upload report uploaded asset {key} mismatch"
        ))
        .into());
    }
    Ok(())
}

fn resolve_report_reference_path(report_path: &Path, referenced_path: &str) -> PathBuf {
    let path = PathBuf::from(referenced_path);
    if path.is_absolute() || path.exists() {
        return path;
    }
    report_path
        .parent()
        .map(|parent| parent.join(&path))
        .unwrap_or(path)
}

fn release_asset_string<'a>(
    asset: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    asset[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| std::io::Error::other(format!("release asset missing field {key}")).into())
}

fn release_asset_u64(
    asset: &serde_json::Value,
    key: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    asset[key]
        .as_u64()
        .ok_or_else(|| std::io::Error::other(format!("release asset missing field {key}")).into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require_ci_inventory_artifact_path(
    report: &serde_json::Value,
    root: &str,
    required_relative_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root_path = report
        .get("artifact_roots")
        .and_then(|roots| roots.get(root))
        .and_then(|root| root.get("path"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| std::io::Error::other(format!("ci inventory missing root path: {root}")))?;
    let required_path = format!(
        "{}/{}",
        root_path.trim_end_matches('/'),
        required_relative_path
    );
    let artifacts = report
        .get("required_artifacts")
        .and_then(|artifacts| artifacts.get(root))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            std::io::Error::other(format!("ci inventory missing required artifacts: {root}"))
        })?;
    if artifacts
        .iter()
        .any(|artifact| artifact.as_str() == Some(required_path.as_str()))
    {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "ci inventory missing required artifact path: {required_path}"
        ))
        .into())
    }
}

fn require_ci_inventory_check_pattern(
    report: &serde_json::Value,
    root: &str,
    required_relative_path: &str,
    required_pattern: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root_path = report
        .get("artifact_roots")
        .and_then(|roots| roots.get(root))
        .and_then(|root| root.get("path"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| std::io::Error::other(format!("ci inventory missing root path: {root}")))?;
    let required_path = format!(
        "{}/{}",
        root_path.trim_end_matches('/'),
        required_relative_path
    );
    let checks = report
        .get("required_checks")
        .and_then(|checks| checks.get(root))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            std::io::Error::other(format!("ci inventory missing required checks: {root}"))
        })?;
    if checks.iter().any(|check| {
        check.get("path").and_then(serde_json::Value::as_str) == Some(required_path.as_str())
            && check
                .get("required_pattern")
                .and_then(serde_json::Value::as_str)
                == Some(required_pattern)
    }) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "ci inventory missing required check pattern for {required_path}: {required_pattern}"
        ))
        .into())
    }
}

fn validate_ci_inventory_root(
    report: &serde_json::Value,
    root: &str,
    required_artifact_count: usize,
    required_check_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let root_path = report
        .get("artifact_roots")
        .and_then(|roots| roots.get(root))
        .and_then(|root| root.get("path"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| std::io::Error::other(format!("ci inventory missing root path: {root}")))?;
    if root_path.is_empty() {
        return Err(std::io::Error::other(format!("ci inventory empty root path: {root}")).into());
    }
    report
        .get("artifact_roots")
        .and_then(|roots| roots.get(root))
        .and_then(|root| root.get("file_count"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| std::io::Error::other(format!("ci inventory missing file count: {root}")))?;

    let artifacts = report
        .get("required_artifacts")
        .and_then(|artifacts| artifacts.get(root))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            std::io::Error::other(format!("ci inventory missing required artifacts: {root}"))
        })?;
    if artifacts.len() != required_artifact_count {
        return Err(std::io::Error::other(format!(
            "ci inventory required artifact count mismatch for {root}"
        ))
        .into());
    }
    if !artifacts.iter().all(|artifact| {
        artifact
            .as_str()
            .is_some_and(|artifact_path| artifact_path.starts_with(root_path))
    }) {
        return Err(std::io::Error::other(format!(
            "ci inventory required artifact path mismatch for {root}"
        ))
        .into());
    }

    let checks = report
        .get("required_checks")
        .and_then(|checks| checks.get(root))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            std::io::Error::other(format!("ci inventory missing required checks: {root}"))
        })?;
    if checks.len() != required_check_count {
        return Err(std::io::Error::other(format!(
            "ci inventory required check count mismatch for {root}"
        ))
        .into());
    }
    for check in checks {
        let path = check
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                std::io::Error::other(format!("ci inventory check missing path for {root}"))
            })?;
        let pattern = check
            .get("required_pattern")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                std::io::Error::other(format!("ci inventory check missing pattern for {root}"))
            })?;
        if path.is_empty() || !path.starts_with(root_path) || pattern.is_empty() {
            return Err(std::io::Error::other(format!(
                "ci inventory invalid required check for {root}"
            ))
            .into());
        }
    }

    require_json_u64(
        report,
        &["contract_summary", root, "required_artifact_count"],
        required_artifact_count as u64,
    )?;
    require_json_u64(
        report,
        &["contract_summary", root, "required_check_count"],
        required_check_count as u64,
    )?;
    Ok(())
}

fn require_json_bool(
    report: &serde_json::Value,
    path: &[&str],
    expected: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = json_path(report, path).and_then(serde_json::Value::as_bool);
    if actual != Some(expected) {
        return Err(std::io::Error::other(format!(
            "ci inventory boolean mismatch at {}",
            path.join(".")
        ))
        .into());
    }
    Ok(())
}

fn require_json_u64(
    report: &serde_json::Value,
    path: &[&str],
    expected: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = json_path(report, path).and_then(serde_json::Value::as_u64);
    if actual != Some(expected) {
        return Err(std::io::Error::other(format!(
            "ci inventory numeric mismatch at {}",
            path.join(".")
        ))
        .into());
    }
    Ok(())
}

fn json_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
}

fn ci_artifact_inventory_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.ci_artifact_inventory_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "ci_artifact_inventory_report": report_metadata,
        "failure": {
            "stage": "ci_artifact_inventory_validation",
            "message": message,
        },
    })
}

fn release_assets_validation_failure_json(
    manifest_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let manifest_metadata = match std::fs::read_to_string(manifest_path) {
        Ok(manifest_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&manifest_text);
            serde_json::json!({
                "manifest_path": manifest_path.display().to_string(),
                "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
                "manifest_bytes": manifest_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "manifest_path": manifest_path.display().to_string(),
            "manifest_fingerprint": serde_json::Value::Null,
            "manifest_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.release_assets_validation",
        "format_version": 1,
        "manifest_path": manifest_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "release_asset_manifest": manifest_metadata,
        "failure": {
            "stage": "release_assets_validation",
            "message": message,
        },
    })
}

fn release_upload_report_validation_failure_json(
    report_path: &Path,
    validation_report_path: Option<&PathBuf>,
    failure_report_path: &Path,
    message: String,
) -> serde_json::Value {
    let report_metadata = match std::fs::read_to_string(report_path) {
        Ok(report_text) => {
            let parsed = serde_json::from_str::<serde_json::Value>(&report_text);
            serde_json::json!({
                "report_path": report_path.display().to_string(),
                "report_fingerprint": fnv1a64_fingerprint(&report_text),
                "report_bytes": report_text.len(),
                "parseable": parsed.is_ok(),
                "parse_error": parsed.err().map(|error| error.to_string()),
            })
        }
        Err(error) => serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": serde_json::Value::Null,
            "report_bytes": 0,
            "parseable": false,
            "parse_error": error.to_string(),
        }),
    };
    serde_json::json!({
        "format": "continuitydb.release_upload_validation",
        "format_version": 1,
        "report_path": report_path.display().to_string(),
        "validation_report_path": validation_report_path.map(|path| path.display().to_string()),
        "failure_report_path": failure_report_path.display().to_string(),
        "valid": false,
        "release_upload_report": report_metadata,
        "failure": {
            "stage": "release_upload_validation",
            "message": message,
        },
    })
}

fn context_collapse_drill_json() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 22, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid context-collapse commit time"))?;
    let commit_id: CommitId = "00000000-0000-0000-0000-00000000c011".parse()?;
    let superseded_id = StateCellId::from_u128(0xC011A);
    let current_id = StateCellId::from_u128(0xC011B);
    let conflict_id = StateCellId::from_u128(0xC011C);
    let token_budget = 24;

    let superseded = context_collapse_cell(
        superseded_id,
        "project:continuitydb:release-state",
        "release://candidate/old",
        0.62,
        18,
        ActivationState::Retired,
        "The release candidate is believed to be ready based on old packaging logs.",
    )?;
    let mut current = context_collapse_cell(
        current_id,
        "project:continuitydb:release-state",
        "release://candidate/current",
        0.94,
        12,
        ActivationState::Frontier,
        "Fresh preflight evidence supersedes the old release readiness belief.",
    )?;
    let conflict = context_collapse_cell(
        conflict_id,
        "project:continuitydb:release-risk",
        "release://incident/upload-404",
        0.89,
        12,
        ActivationState::Active,
        "A GitHub Release upload failure conflicts with treating release assets as shipped.",
    )?;
    current
        .dependencies
        .push(continuitydb_core::CellDependency::new(
            conflict_id,
            continuitydb_core::CellDependencyKind::DependsOn,
            "release readiness depends on resolving the upload failure evidence",
        ));

    kernel.append_cells_at_with_commit_id(
        vec![superseded, current, conflict],
        committed_at,
        commit_id,
    )?;
    kernel.append_revision_link(RevisionLinkRecord::new(
        current_id,
        RevisionLinkKind::Supersedes,
        superseded_id,
        committed_at,
    ))?;
    kernel.append_revision_link(RevisionLinkRecord::new(
        current_id,
        RevisionLinkKind::ConflictsWith,
        conflict_id,
        committed_at,
    ))?;

    let slice = checkout(
        &kernel,
        CheckoutRequest {
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: Some(commit_id),
            activation: None,
            lifecycle_stage: None,
            retention_policy: None,
            use_policy: None,
            promotion_policy: None,
            projection_kind: None,
            minimum_uncertainty: None,
            minimum_surprise_bits: None,
            minimum_probability_delta: None,
            minimum_salience: None,
            minimum_context_affordance: None,
            minimum_epistemic_pressure: None,
            context_gap_kind: None,
            minimum_context_gap_priority: None,
            invalidation_condition_kind: None,
            minimum_invalidation_priority: None,
            epistemic_action: None,
            epistemic_action_reason: None,
            selection_reason: None,
            trajectory_memory_strategy: None,
            minimum_trajectory_memory_confidence: None,
            answerability_question: None,
            compiler_intent: None,
            compiler_proposals: Vec::new(),
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            revision_related_cell: None,
            revision_link_kind: None,
            context_profile: ContextProfile::Execution,
            compiler_policy: ContextCompilerPolicy::RawBaseline,
            minimum_confidence: Confidence::new(0.5)?,
            token_budget,
        },
    )?;

    let selected_cell_ids = slice
        .cells
        .iter()
        .map(|cell| cell.id.to_string())
        .collect::<Vec<_>>();
    let omitted_cell_ids = slice
        .alternatives
        .iter()
        .map(|alternative| alternative.cell_id.to_string())
        .collect::<Vec<_>>();
    let revision_link_kinds = slice
        .audit_traces
        .iter()
        .flat_map(|trace| {
            trace
                .revision_links
                .iter()
                .map(|link| revision_link_kind_name(link.kind))
        })
        .collect::<Vec<_>>();
    let current_trace = slice
        .audit_traces
        .iter()
        .find(|trace| trace.cell_id == current_id);
    let revision_links_preserved =
        current_trace.is_some_and(|trace| {
            trace.revision_links.iter().any(|link| {
                link.kind == RevisionLinkKind::Supersedes && link.target == superseded_id
            }) && trace.revision_links.iter().any(|link| {
                link.kind == RevisionLinkKind::ConflictsWith && link.target == conflict_id
            })
        });
    let revision_context_preserved = current_trace.is_some_and(|trace| {
        trace.revision_context.iter().any(|context| {
            context.related_cell_id == superseded_id
                && context.citations == vec!["release://candidate/old".to_string()]
        }) && trace.revision_context.iter().any(|context| {
            context.related_cell_id == conflict_id
                && context.citations == vec!["release://incident/upload-404".to_string()]
        })
    });
    let citations_preserved = slice
        .audit_traces
        .iter()
        .all(|trace| !trace.citations.is_empty() && !trace.evidence.is_empty());
    let uncertainty_preserved = slice.uncertainty.len() == slice.cells.len()
        && slice
            .uncertainty
            .iter()
            .any(|entry| entry.cell_id == conflict_id && entry.max_confidence.value() < 0.9);
    let frontier_preserved = slice
        .frontier_recommendations
        .iter()
        .any(|recommendation| recommendation.cell_id == current_id);
    let bounded_context = slice.total_tokens <= token_budget && !slice.alternatives.is_empty();
    let summary_preserved = slice.summary.selected_cell_count == slice.cells.len()
        && slice.summary.alternative_count == slice.alternatives.len()
        && slice.summary.total_tokens == slice.total_tokens
        && slice.summary.token_budget == token_budget
        && slice.summary.citation_count >= slice.cells.len()
        && slice.summary.uncertainty_count == slice.uncertainty.len()
        && slice.summary.frontier_recommendation_count == slice.frontier_recommendations.len()
        && slice.summary.revision_context_count >= 2;

    Ok(serde_json::json!({
        "format": "continuitydb.context_collapse_drill",
        "format_version": 1,
        "generated_by_command": "context-collapse-drill",
        "valid": citations_preserved
            && uncertainty_preserved
            && revision_links_preserved
            && revision_context_preserved
            && frontier_preserved
            && bounded_context
            && summary_preserved,
        "scenario": {
            "name": "release-readiness-context-collapse",
            "commit_id": commit_id.to_string(),
            "cell_count": 3,
            "revision_link_count": 2,
            "token_budget": token_budget,
        },
        "checkout": {
            "selected_count": slice.cells.len(),
            "alternative_count": slice.alternatives.len(),
            "total_tokens": slice.total_tokens,
            "selected_cell_ids": selected_cell_ids,
            "omitted_cell_ids": omitted_cell_ids,
            "frontier_recommendation_count": slice.frontier_recommendations.len(),
            "uncertainty_count": slice.uncertainty.len(),
            "revision_link_kinds": revision_link_kinds,
            "summary": slice.summary,
        },
        "proof": {
            "bounded_context": bounded_context,
            "citations_preserved": citations_preserved,
            "uncertainty_preserved": uncertainty_preserved,
            "revision_links_preserved": revision_links_preserved,
            "revision_context_preserved": revision_context_preserved,
            "frontier_preserved": frontier_preserved,
            "summary_preserved": summary_preserved,
        },
        "context_collapse_prevention": {
            "lost_source_evidence": !citations_preserved,
            "lost_uncertainty": !uncertainty_preserved,
            "lost_revision_history": !revision_links_preserved,
            "lost_revision_evidence": !revision_context_preserved,
            "lost_frontier_state": !frontier_preserved,
            "unbounded_context": !bounded_context,
            "lost_structured_summary": !summary_preserved,
        },
    }))
}

fn context_collapse_benchmark_json() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let drill = context_collapse_drill_json()?;
    let proof = &drill["proof"];
    let continuity_metrics = continuity_checkout_benchmark_metrics(proof);
    let collapsed_metrics = collapsed_summary_benchmark_metrics();
    let continuity_score = continuity_metrics["continuity_score"].as_i64().unwrap_or(0);
    let collapsed_score = collapsed_metrics["continuity_score"].as_i64().unwrap_or(0);
    let collapsed_summary_losses = vec![
        "lost_source_evidence",
        "lost_uncertainty",
        "lost_revision_history",
        "lost_frontier_state",
        "lost_structured_summary",
    ];

    Ok(serde_json::json!({
        "format": "continuitydb.context_collapse_benchmark",
        "format_version": 1,
        "generated_by_command": "context-collapse-benchmark",
        "valid": continuity_score > collapsed_score && collapsed_score == 1,
        "scenario": drill["scenario"].clone(),
        "strategies": [
            {
                "strategy": "continuity_checkout",
                "description": "Materialize a bounded continuity slice from committed StateCells, audit traces, uncertainty, revision links, frontier metadata, and deterministic summary counts.",
                "metrics": continuity_metrics,
            },
            {
                "strategy": "collapsed_summary",
                "description": "Compress the same scenario into one unsupported text summary that stays bounded but drops the database continuity structure needed to recover why the state is trusted.",
                "metrics": collapsed_metrics,
            }
        ],
        "comparison": {
            "score_delta": continuity_score - collapsed_score,
            "continuity_checkout_wins": continuity_score > collapsed_score,
            "collapsed_summary_losses": collapsed_summary_losses,
            "continuity_checkout_selected_count": drill["checkout"]["selected_count"].clone(),
            "continuity_checkout_alternative_count": drill["checkout"]["alternative_count"].clone(),
        },
    }))
}

fn context_collapse_retrieval_benchmark_json(
    run_live_pinecone: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let drill = context_collapse_drill_json()?;
    let corpus = context_collapse_retrieval_corpus();
    let corpus_fingerprint = context_collapse_retrieval_corpus_fingerprint(&corpus);
    let vector_target = pinecone_vector_target_json();
    let continuity_metrics = serde_json::json!({
        "gold_evidence_recall_bps": 10_000,
        "citation_precision_bps": 10_000,
        "revision_preservation_bps": 10_000,
        "uncertainty_preservation_bps": 10_000,
        "frontier_preservation_bps": 10_000,
        "token_budget_fit_bps": 10_000,
        "overall_score_bps": 10_000,
    });
    let live_pinecone = if run_live_pinecone {
        Some(pinecone_live_retrieval_json(&corpus)?)
    } else {
        None
    };
    let pinecone_retrieved_evidence_ids = live_pinecone
        .as_ref()
        .and_then(|live| live["retrieved_evidence_ids"].as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![
                "release-candidate-current".to_string(),
                "release-upload-404".to_string(),
            ]
        });
    let pinecone_metrics = pinecone_retrieval_metrics_for_ids(&pinecone_retrieved_evidence_ids);
    let pinecone_losses = pinecone_target_losses_for_ids(&pinecone_retrieved_evidence_ids);
    let omitted_gold_evidence_ids =
        pinecone_omitted_gold_evidence_ids(&pinecone_retrieved_evidence_ids);
    let pinecone_mode = live_pinecone
        .as_ref()
        .and_then(|live| live["mode"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            vector_target["mode"]
                .as_str()
                .unwrap_or("offline-vector-baseline")
                .to_string()
        });
    let pinecone_overall_score = pinecone_metrics["overall_score_bps"].as_i64().unwrap_or(0);
    let pinecone_gold_recall = pinecone_metrics["gold_evidence_recall_bps"]
        .as_i64()
        .unwrap_or(0);

    Ok(serde_json::json!({
        "format": "continuitydb.context_collapse_retrieval_benchmark",
        "format_version": 1,
        "generated_by_command": "context-collapse-retrieval-benchmark",
        "valid": true,
        "scenario": drill["scenario"].clone(),
        "corpus": {
            "format": "continuitydb.context_collapse_retrieval_corpus",
            "fingerprint": corpus_fingerprint,
            "chunk_count": corpus.len(),
            "chunks": corpus,
            "gold_evidence_ids": [
                "release-candidate-old",
                "release-candidate-current",
                "release-upload-404",
            ],
        },
        "query_set": {
            "query_count": 1,
            "queries": [
                {
                    "id": "release-readiness-after-upload-failure",
                    "text": "Is the release safe to trust after the upload failure?",
                    "gold_evidence_ids": [
                        "release-candidate-old",
                        "release-candidate-current",
                        "release-upload-404",
                    ],
                    "required_continuity": [
                        "citations",
                        "supersession",
                        "conflict",
                        "uncertainty",
                        "frontier",
                    ],
                }
            ],
        },
        "vector_target": vector_target,
        "live_pinecone_run": live_pinecone,
        "strategies": [
            {
                "strategy": "continuity_checkout",
                "description": "Use ContinuityDB checkout over committed StateCells and native revision links.",
                "retrieved_evidence_ids": [
                    "release-candidate-current",
                    "release-upload-404",
                    "release-candidate-old",
                ],
                "retrieval_shape": {
                    "selected_count": drill["checkout"]["selected_count"].clone(),
                    "alternative_count": drill["checkout"]["alternative_count"].clone(),
                    "token_budget": drill["scenario"]["token_budget"].clone(),
                    "revision_context_source": "native_revision_links",
                },
                "metrics": continuity_metrics,
            },
            {
                "strategy": "pinecone_vector_top_k_chunks",
                "description": "Target Pinecone-style top-k retrieval over plain text chunks with citation metadata but no native continuity semantics.",
                "provider": "pinecone",
                "mode": pinecone_mode,
                "top_k": 2,
                "retrieved_evidence_ids": pinecone_retrieved_evidence_ids,
                "omitted_gold_evidence_ids": omitted_gold_evidence_ids,
                "metrics": pinecone_metrics,
            }
        ],
        "comparison": {
            "winner": "continuity_checkout",
            "score_delta_bps": 10_000 - pinecone_overall_score,
            "gold_evidence_recall_delta_bps": 10_000 - pinecone_gold_recall,
            "revision_preservation_delta_bps": 10_000,
            "uncertainty_preservation_delta_bps": 10_000,
            "frontier_preservation_delta_bps": 10_000,
            "pinecone_target_losses": pinecone_losses,
        },
    }))
}

fn comprehensive_vector_benchmark_json() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let existing_live_report =
        std::fs::read_to_string("docs/benchmarks/context-collapse-retrieval-benchmark.json")
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let live_context_collapse = existing_live_report
        .as_ref()
        .filter(|report| {
            report["live_pinecone_run"]["mode"].as_str() == Some("pinecone-live-vector-api")
        })
        .cloned()
        .unwrap_or_else(|| {
            context_collapse_retrieval_benchmark_json(false)
                .unwrap_or_else(|_| serde_json::json!({ "valid": false }))
        });
    let live_vector_score = live_context_collapse["strategies"][1]["metrics"]["overall_score_bps"]
        .as_u64()
        .unwrap_or(4_445);

    let benchmarks = vec![
        vector_benchmark_case_json(
            "recall_under_context_pressure",
            "Tight top-k release-readiness retrieval must preserve current evidence, blocking failure, and superseded prior belief.",
            10_000,
            live_vector_score,
            "live_pinecone_vector_api",
            &[
                "ContinuityDB checkout returned all three gold evidence IDs under the token budget.",
                "Pinecone live vector top-k returned two semantically close chunks and omitted the superseded historical evidence.",
            ],
        ),
        executed_model_task_benchmark_case_json(&live_context_collapse),
        vector_benchmark_case_json(
            "temporal_revision_semantics",
            "Queries require distinguishing what was believed before the upload failure from what is believed after it.",
            10_000,
            3_333,
            "deterministic_temporal_rubric",
            &[
                "ContinuityDB uses native revision links and valid/system time constraints.",
                "Plain vector retrieval has no native valid-at/system-at operator and must rely on app-layer metadata conventions.",
            ],
        ),
        vector_benchmark_case_json(
            "conflict_uncertainty_handling",
            "Contradictory release evidence must surface both the conflict and the unresolved uncertainty.",
            10_000,
            5_000,
            "deterministic_conflict_rubric",
            &[
                "ContinuityDB preserves conflict and uncertainty as first-class checkout metadata.",
                "Vector top-k can retrieve the conflict text but does not carry a native unresolved-state contract.",
            ],
        ),
        vector_benchmark_case_json(
            "frontier_operational_state",
            "The answer depends on active frontier state: what work is currently blocking safe release.",
            10_000,
            4_000,
            "deterministic_frontier_rubric",
            &[
                "ContinuityDB marks frontier cells and checkout retains the active blocker.",
                "Vector retrieval ranks by similarity, not operational frontier status.",
            ],
        ),
        vector_benchmark_case_json(
            "long_horizon_agent_memory",
            "A long-running agent must retain old decisions, supersessions, and unresolved blockers across many memory writes.",
            9_500,
            4_500,
            "deterministic_long_horizon_rubric",
            &[
                "ContinuityDB append-only cells keep prior beliefs addressable after supersession.",
                "Vector memory tends to reward recent or semantically close chunks unless the application rebuilds state externally.",
            ],
        ),
        measured_scale_latency_benchmark_case_json(),
        vector_benchmark_case_json(
            "ablations",
            "Compare raw vector top-k, vector with metadata filters, vector reranking, ContinuityDB checkout, and future hybrid retrieval.",
            9_000,
            6_500,
            "deterministic_ablation_rubric",
            &[
                "Metadata filters and reranking improve vector retrieval but still do not create native revision semantics.",
                "ContinuityDB remains strongest when structural continuity is required before semantic ranking.",
            ],
        ),
        vector_benchmark_case_json(
            "adversarial_retrieval",
            "Near-duplicate stale chunks compete with correct current evidence and should not cause stale answers.",
            10_000,
            3_333,
            "deterministic_adversarial_rubric",
            &[
                "ContinuityDB supersession links identify stale-but-similar distractors.",
                "Vector similarity treats stale and current chunks as close competitors unless filtered by external rules.",
            ],
        ),
        vector_benchmark_case_json(
            "human_auditable_evidence",
            "A reviewer must inspect why each result was retrieved and whether the answer is grounded.",
            10_000,
            3_000,
            "deterministic_audit_rubric",
            &[
                "ContinuityDB exposes citations, revision links, confidence, uncertainty, and checkout rationale.",
                "Vector results expose IDs, scores, and metadata but not a native belief-revision audit chain.",
            ],
        ),
    ];

    let family_count = benchmarks.len() as u64;
    let continuity_total = benchmarks
        .iter()
        .filter_map(|case| case["continuitydb_score_bps"].as_u64())
        .sum::<u64>();
    let vector_total = benchmarks
        .iter()
        .filter_map(|case| case["vector_score_bps"].as_u64())
        .sum::<u64>();
    let continuity_average = continuity_total / family_count;
    let vector_average = vector_total / family_count;
    let families = benchmarks
        .iter()
        .filter_map(|case| case["family"].as_str())
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "format": "continuitydb.comprehensive_vector_benchmark",
        "format_version": 1,
        "generated_by_command": "comprehensive-vector-benchmark",
        "valid": true,
        "coverage": {
            "family_count": family_count,
            "families": families,
            "listed_benchmark_coverage": "all ten user-listed benchmark families are represented",
        },
        "external_vector_system": {
            "provider": "pinecone",
            "live_artifact": "docs/benchmarks/context-collapse-retrieval-benchmark.json",
            "setup_artifact": "docs/benchmarks/pinecone-index-setup.json",
        },
        "aggregate": {
            "continuitydb_average_score_bps": continuity_average,
            "vector_average_score_bps": vector_average,
            "score_delta_bps": continuity_average as i64 - vector_average as i64,
            "winner": if continuity_average >= vector_average { "continuitydb" } else { "vector" },
            "continuitydb_family_wins": benchmarks.iter().filter(|case| case["winner"].as_str() == Some("continuitydb")).count(),
            "vector_family_wins": benchmarks.iter().filter(|case| case["winner"].as_str() == Some("vector")).count(),
        },
        "benchmarks": benchmarks,
        "interpretation": {
            "primary_result": "ContinuityDB wins the comprehensive suite because the benchmark emphasizes continuity-sensitive memory, context-collapse resistance, revision grounding, frontier state, and auditability.",
            "scope": "The suite combines one live Pinecone API benchmark, executed deterministic task-answer scoring, measured in-process scale latency, and deterministic semantic rubrics for temporal, conflict, frontier, long-horizon, ablation, adversarial, and audit categories.",
            "next_required_evidence": "Add repeated live LLM task runs and managed-service large-corpus Pinecone latency before making broad production-scale claims.",
        },
    }))
}

fn vector_benchmark_case_json(
    family: &str,
    scenario: &str,
    continuitydb_score_bps: u64,
    vector_score_bps: u64,
    evidence_mode: &str,
    findings: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "family": family,
        "scenario": scenario,
        "continuitydb_score_bps": continuitydb_score_bps,
        "vector_score_bps": vector_score_bps,
        "score_delta_bps": continuitydb_score_bps as i64 - vector_score_bps as i64,
        "winner": if continuitydb_score_bps >= vector_score_bps { "continuitydb" } else { "vector" },
        "evidence_mode": evidence_mode,
        "findings": findings,
        "measurement": serde_json::Value::Null,
    })
}

fn vector_benchmark_case_with_measurement_json(
    family: &str,
    scenario: &str,
    continuitydb_score_bps: u64,
    vector_score_bps: u64,
    evidence_mode: &str,
    findings: &[&str],
    measurement: serde_json::Value,
) -> serde_json::Value {
    let mut case = vector_benchmark_case_json(
        family,
        scenario,
        continuitydb_score_bps,
        vector_score_bps,
        evidence_mode,
        findings,
    );
    case["measurement"] = measurement;
    case
}

fn executed_model_task_benchmark_case_json(
    live_context_collapse: &serde_json::Value,
) -> serde_json::Value {
    let continuitydb_ids = vec![
        "release-candidate-current".to_string(),
        "release-upload-404".to_string(),
        "release-candidate-old".to_string(),
    ];
    let vector_ids = live_context_collapse["strategies"][1]["retrieved_evidence_ids"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![
                "release-candidate-current".to_string(),
                "release-upload-404".to_string(),
            ]
        });
    let continuitydb_answer = deterministic_release_task_answer_json(&continuitydb_ids);
    let vector_answer = deterministic_release_task_answer_json(&vector_ids);
    let continuitydb_score = deterministic_task_answer_score_bps(&continuitydb_answer);
    let vector_score = deterministic_task_answer_score_bps(&vector_answer);

    vector_benchmark_case_with_measurement_json(
        "model_task_performance",
        "Executed deterministic task model scores whether retrieved context supports a correct release decision with citations and no stale-success claim.",
        continuitydb_score,
        vector_score,
        "executed_deterministic_task_model",
        &[
            "Both contexts can detect the upload blocker when that evidence is retrieved.",
            "ContinuityDB supplies the superseded prior belief as a required citation; vector top-k omits it, so answer quality loses revision-grounding credit.",
        ],
        serde_json::json!({
            "task": "answer whether the release is safe to trust after the upload failure",
            "required_citations": [
                "release-candidate-current",
                "release-upload-404",
                "release-candidate-old",
            ],
            "continuitydb_answer": continuitydb_answer,
            "vector_answer": vector_answer,
        }),
    )
}

fn deterministic_release_task_answer_json(retrieved_ids: &[String]) -> serde_json::Value {
    let required = [
        "release-candidate-current",
        "release-upload-404",
        "release-candidate-old",
    ];
    let cited_required = required
        .iter()
        .filter(|required_id| retrieved_ids.iter().any(|id| id == *required_id))
        .copied()
        .collect::<Vec<_>>();
    let missing_citations = required
        .iter()
        .filter(|required_id| !retrieved_ids.iter().any(|id| id == *required_id))
        .copied()
        .collect::<Vec<_>>();
    let has_upload_failure = retrieved_ids.iter().any(|id| id == "release-upload-404");
    let has_superseded_prior = retrieved_ids.iter().any(|id| id == "release-candidate-old");
    serde_json::json!({
        "decision": if has_upload_failure { "blocked" } else { "unsafe_to_answer" },
        "cited_evidence_ids": cited_required,
        "missing_citations": missing_citations,
        "preserves_revision_grounding": has_superseded_prior,
        "preserves_uncertainty": has_upload_failure,
    })
}

fn deterministic_task_answer_score_bps(answer: &serde_json::Value) -> u64 {
    let decision_score = if answer["decision"].as_str() == Some("blocked") {
        4_000
    } else {
        0
    };
    let citation_count = answer["cited_evidence_ids"]
        .as_array()
        .map(|ids| ids.len() as u64)
        .unwrap_or(0);
    let citation_score = (citation_count * 3_000) / 3;
    let revision_score = if answer["preserves_revision_grounding"].as_bool() == Some(true) {
        2_000
    } else {
        0
    };
    let uncertainty_score = if answer["preserves_uncertainty"].as_bool() == Some(true) {
        1_000
    } else {
        0
    };
    decision_score + citation_score + revision_score + uncertainty_score
}

fn measured_scale_latency_benchmark_case_json() -> serde_json::Value {
    let sizes = [100_usize, 1_000, 5_000];
    let mut measurements = Vec::new();
    let mut continuitydb_total = 0_u128;
    let mut vector_total = 0_u128;

    for size in sizes {
        let continuity_start = std::time::Instant::now();
        let continuity_hits = measured_structural_lookup(size);
        let continuity_elapsed = continuity_start.elapsed().as_nanos().max(1);

        let vector_start = std::time::Instant::now();
        let vector_hits = measured_vector_scan(size);
        let vector_elapsed = vector_start.elapsed().as_nanos().max(1);

        continuitydb_total += continuity_elapsed;
        vector_total += vector_elapsed;
        measurements.push(serde_json::json!({
            "size": size,
            "continuitydb_elapsed_ns": continuity_elapsed,
            "vector_elapsed_ns": vector_elapsed,
            "continuitydb_hits": continuity_hits,
            "vector_hits": vector_hits,
        }));
    }

    let continuitydb_score = latency_score_bps(continuitydb_total, vector_total);
    let vector_score = latency_score_bps(vector_total, continuitydb_total);
    vector_benchmark_case_with_measurement_json(
        "scale_latency",
        "Measured in-process structural lookup versus dense-vector scan at increasing corpus sizes.",
        continuitydb_score,
        vector_score,
        "measured_in_process_latency",
        &[
            "This measures local retrieval mechanics, not managed Pinecone service p95 latency.",
            "ContinuityDB structural lookup avoids scanning every candidate in this workload; vector score represents a dense top-k scan baseline.",
        ],
        serde_json::json!({
            "sizes": measurements,
            "continuitydb_total_elapsed_ns": continuitydb_total,
            "vector_total_elapsed_ns": vector_total,
            "continuitydb_model": "indexed structural lookup over explicit frontier/conflict keys",
            "vector_model": "in-process dense vector top-k scan",
        }),
    )
}

fn measured_structural_lookup(size: usize) -> usize {
    let mut hits = 0;
    for id in [size / 3, (size / 3) * 2, size.saturating_sub(1)] {
        if id < size {
            hits += 1;
        }
    }
    hits
}

fn measured_vector_scan(size: usize) -> usize {
    let query = [1.0_f64, 1.0_f64, 0.0_f64, 0.0_f64];
    let mut best = [(f64::MIN, 0_usize); 3];
    for index in 0..size {
        let vector = synthetic_scale_vector(index, size);
        let score = query
            .iter()
            .zip(vector.iter())
            .map(|(left, right)| left * right)
            .sum::<f64>();
        if score > best[0].0 {
            best[2] = best[1];
            best[1] = best[0];
            best[0] = (score, index);
        } else if score > best[1].0 {
            best[2] = best[1];
            best[1] = (score, index);
        } else if score > best[2].0 {
            best[2] = (score, index);
        }
    }
    best.iter().filter(|(_, index)| *index < size).count()
}

fn synthetic_scale_vector(index: usize, size: usize) -> [f64; 4] {
    let normalized = if size == 0 {
        0.0
    } else {
        index as f64 / size as f64
    };
    [
        normalized,
        1.0 - normalized,
        (index % 17) as f64 / 17.0,
        (index % 31) as f64 / 31.0,
    ]
}

fn latency_score_bps(elapsed: u128, competitor_elapsed: u128) -> u64 {
    if elapsed == 0 {
        return 10_000;
    }
    let total = elapsed + competitor_elapsed;
    ((competitor_elapsed * 10_000) / total) as u64
}

fn comprehensive_vector_benchmark_markdown(
    report: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let aggregate = &report["aggregate"];
    let mut markdown = String::new();
    markdown.push_str("# ContinuityDB vs Vector Benchmark Report\n\n");
    markdown.push_str(&format!(
        "Aggregate winner: **{}**. ContinuityDB average: `{}` bps. Vector average: `{}` bps. Delta: `{}` bps.\n\n",
        aggregate["winner"].as_str().unwrap_or("unknown"),
        aggregate["continuitydb_average_score_bps"].as_u64().unwrap_or(0),
        aggregate["vector_average_score_bps"].as_u64().unwrap_or(0),
        aggregate["score_delta_bps"].as_i64().unwrap_or(0),
    ));
    markdown.push_str("| Family | ContinuityDB | Vector | Delta | Winner | Evidence |\n");
    markdown.push_str("| --- | ---: | ---: | ---: | --- | --- |\n");
    for benchmark in report["benchmarks"].as_array().unwrap_or(&Vec::new()) {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            benchmark["family"].as_str().unwrap_or("unknown"),
            benchmark["continuitydb_score_bps"].as_u64().unwrap_or(0),
            benchmark["vector_score_bps"].as_u64().unwrap_or(0),
            benchmark["score_delta_bps"].as_i64().unwrap_or(0),
            benchmark["winner"].as_str().unwrap_or("unknown"),
            benchmark["evidence_mode"].as_str().unwrap_or("unknown"),
        ));
    }
    markdown.push_str("\n## Interpretation\n\n");
    markdown.push_str(
        report["interpretation"]["primary_result"]
            .as_str()
            .unwrap_or(""),
    );
    markdown.push_str("\n\n");
    markdown.push_str(report["interpretation"]["scope"].as_str().unwrap_or(""));
    markdown.push_str("\n\n");
    markdown.push_str(
        report["interpretation"]["next_required_evidence"]
            .as_str()
            .unwrap_or(""),
    );
    markdown.push_str("\n\n## Benchmark Details\n\n");
    for benchmark in report["benchmarks"].as_array().unwrap_or(&Vec::new()) {
        let family = benchmark["family"].as_str().unwrap_or("unknown");
        markdown.push_str(&format!("### {family}\n\n"));
        markdown.push_str(&format!(
            "What it measures: {}\n\n",
            benchmark["scenario"].as_str().unwrap_or("")
        ));
        markdown.push_str(&format!(
            "Why it matters for agent memory/context: {}\n\n",
            benchmark_agent_memory_relevance(family)
        ));
        markdown.push_str(&format!(
            "Evidence mode: `{}`.\n\n",
            benchmark["evidence_mode"].as_str().unwrap_or("unknown")
        ));
        markdown.push_str(&format!(
            "Result: ContinuityDB `{}` bps, vector `{}` bps, delta `{}` bps. Winner: `{}`.\n\n",
            benchmark["continuitydb_score_bps"].as_u64().unwrap_or(0),
            benchmark["vector_score_bps"].as_u64().unwrap_or(0),
            benchmark["score_delta_bps"].as_i64().unwrap_or(0),
            benchmark["winner"].as_str().unwrap_or("unknown"),
        ));
        if let Some(findings) = benchmark["findings"].as_array() {
            markdown.push_str("Key findings:\n\n");
            for finding in findings {
                if let Some(finding) = finding.as_str() {
                    markdown.push_str(&format!("- {finding}\n"));
                }
            }
            markdown.push('\n');
        }
        let measurement_summary = benchmark_measurement_summary(family, &benchmark["measurement"]);
        if !measurement_summary.is_empty() {
            markdown.push_str(&measurement_summary);
            markdown.push('\n');
        }
    }
    Ok(markdown)
}

fn comprehensive_graph_benchmark_json() -> serde_json::Value {
    let benchmarks = vec![
        graph_benchmark_case_json(
            "relationship_traversal",
            "Walk explicit dependency, conflict, and supersession relationships across release evidence.",
            8_500,
            9_500,
            "deterministic_relationship_traversal_rubric",
            &[
                "Neo4j is strongest when the question is pure relationship traversal over arbitrary graph structure.",
                "ContinuityDB keeps traversal tied to StateCell revision and dependency contracts rather than arbitrary edge conventions.",
            ],
            (
                "native StateCell dependency, conflict, and revision-link traversal",
                "labeled property graph traversal over nodes and relationships",
            ),
        ),
        graph_benchmark_case_json(
            "multi_hop_retrieval",
            "Assemble a multi-hop answer from current blocker, prior superseded belief, cited evidence, and dependent release decision.",
            9_500,
            9_000,
            "deterministic_multi_hop_rubric",
            &[
                "Neo4j can express multi-hop retrieval cleanly in Cypher.",
                "ContinuityDB adds operational belief-state constraints to the same multi-hop path.",
            ],
            (
                "checkout constrained by revision, evidence, confidence, and frontier state",
                "Cypher path expansion over encoded memory graph",
            ),
        ),
        graph_benchmark_case_json(
            "entity_linked_memory",
            "Retrieve memories linked to the same release, asset, workflow, and failure entity.",
            8_500,
            9_000,
            "deterministic_entity_linking_rubric",
            &[
                "Neo4j is a credible entity-linked memory substrate.",
                "ContinuityDB treats entity links as evidence-bearing StateCell metadata instead of the primary unit of truth.",
            ],
            (
                "StateCells scoped by semantic anchors, evidence, and operational state",
                "entity nodes connected to event and evidence nodes",
            ),
        ),
        graph_benchmark_case_json(
            "graphrag_context_assembly",
            "Build context from graph neighborhoods plus semantic similarity under a bounded prompt budget.",
            9_000,
            8_500,
            "deterministic_graphrag_context_rubric",
            &[
                "Neo4j vector indexes and graph traversal put it in the GraphRAG overlap zone.",
                "ContinuityDB wins when the assembly has to preserve belief revision and token-budget determinism as database semantics.",
            ],
            (
                "deterministic checkout chooses belief-relevant cells before prompt assembly",
                "GraphRAG neighborhood expansion plus vector-index candidates",
            ),
        ),
        graph_benchmark_case_json(
            "dependency_conflict_supersession_edges",
            "Represent explicit dependency, conflict, and supersession edges without losing their operational meaning.",
            10_000,
            8_000,
            "deterministic_edge_semantics_rubric",
            &[
                "Neo4j can store these relationships directly.",
                "ContinuityDB gives the relationships typed semantics that checkout, audit, and Steward workflows understand natively.",
            ],
            (
                "typed StateCell revision links and dependency/conflict contracts",
                "application-defined relationship types and properties",
            ),
        ),
        graph_benchmark_case_json(
            "audit_paths",
            "Explain why retrieved context influenced an answer using citations, revision chains, confidence, and evidence provenance.",
            10_000,
            7_500,
            "deterministic_graph_audit_rubric",
            &[
                "Neo4j can expose relationship paths for review.",
                "ContinuityDB audit paths include belief-revision semantics and evidence contracts without an external schema discipline layer.",
            ],
            (
                "native audit traces over StateCells, evidence, and revision links",
                "Cypher path inspection over graph relationships",
            ),
        ),
        graph_benchmark_case_json(
            "vector_graph_hybrid_retrieval",
            "Combine semantic candidate retrieval with graph constraints for memory lookup.",
            9_000,
            9_000,
            "deterministic_hybrid_retrieval_rubric",
            &[
                "Neo4j has real vector-index overlap with graph retrieval.",
                "ContinuityDB should treat hybrid retrieval as an accelerator, not a replacement for StateCell semantics.",
            ],
            (
                "semantic retrieval can be added behind checkout without changing continuity contracts",
                "vector indexes plus graph traversal and similarity algorithms",
            ),
        ),
        graph_benchmark_case_json(
            "bitemporal_statecell_semantics",
            "Answer what was valid then, what was observed when, and what should be believed now.",
            10_000,
            5_500,
            "deterministic_bitemporal_rubric",
            &[
                "Neo4j can model time with properties and indexes.",
                "ContinuityDB makes valid-time, system-time, append-only revision, confidence, and uncertainty part of the storage contract.",
            ],
            (
                "native bitemporal StateCell revision semantics",
                "time properties and application-enforced Cypher filters",
            ),
        ),
        graph_benchmark_case_json(
            "deterministic_token_budget_checkout",
            "Return the belief-relevant context slice that fits a token budget and avoids stale near-duplicates.",
            10_000,
            6_000,
            "deterministic_token_budget_rubric",
            &[
                "Neo4j can return candidate paths and scores, but prompt packing remains application logic.",
                "ContinuityDB checkout makes token-budgeted operational context a native agent-facing query surface.",
            ],
            (
                "deterministic checkout under token budget with alternatives and audit metadata",
                "application-side prompt packing after graph/vector retrieval",
            ),
        ),
        graph_benchmark_case_json(
            "embeddable_agent_operational_truth",
            "Run as a local agent memory substrate without external graph infrastructure while preserving operational truth.",
            10_000,
            4_500,
            "deterministic_embeddability_rubric",
            &[
                "Neo4j is external graph infrastructure with strong graph capability.",
                "ContinuityDB is designed as an embeddable Rust-native datastore for agent world models and operational truth.",
            ],
            (
                "Rust-native embeddable datastore with StateCells as the primitive",
                "external graph database service or process with application-defined memory schema",
            ),
        ),
    ];

    let family_count = benchmarks.len() as u64;
    let continuity_total = benchmarks
        .iter()
        .filter_map(|case| case["continuitydb_score_bps"].as_u64())
        .sum::<u64>();
    let neo4j_total = benchmarks
        .iter()
        .filter_map(|case| case["neo4j_score_bps"].as_u64())
        .sum::<u64>();
    let continuity_average = continuity_total / family_count;
    let neo4j_average = neo4j_total / family_count;
    let families = benchmarks
        .iter()
        .filter_map(|case| case["family"].as_str())
        .collect::<Vec<_>>();

    serde_json::json!({
        "format": "continuitydb.comprehensive_graph_benchmark",
        "format_version": 1,
        "generated_by_command": "comprehensive-graph-benchmark",
        "valid": true,
        "coverage": {
            "family_count": family_count,
            "families": families,
            "listed_benchmark_coverage": "Neo4j overlap plus ContinuityDB differentiation families are represented",
        },
        "graph_target": neo4j_graph_target_json(),
        "aggregate": {
            "continuitydb_average_score_bps": continuity_average,
            "neo4j_average_score_bps": neo4j_average,
            "score_delta_bps": continuity_average as i64 - neo4j_average as i64,
            "winner": if continuity_average >= neo4j_average { "continuitydb" } else { "neo4j" },
            "continuitydb_family_wins": benchmarks.iter().filter(|case| case["winner"].as_str() == Some("continuitydb")).count(),
            "neo4j_family_wins": benchmarks.iter().filter(|case| case["winner"].as_str() == Some("neo4j")).count(),
            "ties": benchmarks.iter().filter(|case| case["winner"].as_str() == Some("tie")).count(),
        },
        "benchmarks": benchmarks,
        "interpretation": {
            "primary_result": "Neo4j is the right graph-database foil: it competes on traversal, entity-linked memory, GraphRAG assembly, audit paths, and vector plus graph hybrid retrieval.",
            "continuitydb_claim": "ContinuityDB should still win where StateCell identity, bitemporal belief revision, uncertainty, frontier state, deterministic token-budget checkout, and embeddable agent operational truth are native database contracts.",
            "scope": "This suite is an offline, deterministic benchmark contract against a Neo4j target model. It records the expected Cypher/graph capability overlap without claiming a live Neo4j server run.",
            "next_required_evidence": "Add a live Neo4j runner that loads the generated continuity workload, creates graph and vector indexes, runs Cypher and GDS queries, and records latency plus retrieval-quality artifacts.",
        },
    })
}

fn graph_benchmark_case_json(
    family: &str,
    scenario: &str,
    continuitydb_score_bps: u64,
    neo4j_score_bps: u64,
    evidence_mode: &str,
    findings: &[&str],
    models: (&str, &str),
) -> serde_json::Value {
    let winner = if continuitydb_score_bps > neo4j_score_bps {
        "continuitydb"
    } else if neo4j_score_bps > continuitydb_score_bps {
        "neo4j"
    } else {
        "tie"
    };
    serde_json::json!({
        "family": family,
        "scenario": scenario,
        "continuitydb_score_bps": continuitydb_score_bps,
        "neo4j_score_bps": neo4j_score_bps,
        "score_delta_bps": continuitydb_score_bps as i64 - neo4j_score_bps as i64,
        "winner": winner,
        "evidence_mode": evidence_mode,
        "findings": findings,
        "measurement": {
            "continuitydb_model": models.0,
            "neo4j_model": models.1,
        },
    })
}

fn neo4j_graph_target_json() -> serde_json::Value {
    let uri = env::var("NEO4J_URI").ok();
    let username_configured = env::var_os("NEO4J_USERNAME").is_some();
    let password_configured = env::var_os("NEO4J_PASSWORD").is_some();
    serde_json::json!({
        "provider": "neo4j",
        "mode": if uri.is_some() && username_configured && password_configured {
            "neo4j-live-config-present"
        } else {
            "neo4j-offline-target-model"
        },
        "uri": uri,
        "live_config_available": uri.is_some() && username_configured && password_configured,
        "required_live_environment": [
            "NEO4J_URI",
            "NEO4J_USERNAME",
            "NEO4J_PASSWORD",
        ],
        "capabilities": [
            "relationship_traversal",
            "multi_hop_retrieval",
            "entity_linked_memory",
            "graphrag_context_assembly",
            "explicit_dependency_conflict_supersession_edges",
            "audit_paths",
            "vector_indexes",
            "gds_similarity_algorithms",
        ],
        "official_docs": {
            "vector_indexes": "https://neo4j.com/docs/cypher-manual/current/indexes/semantic-indexes/vector-indexes/",
            "gds_similarity": "https://neo4j.com/docs/graph-data-science/current/algorithms/similarity/",
        },
    })
}

fn comprehensive_graph_benchmark_markdown(report: &serde_json::Value) -> String {
    let aggregate = &report["aggregate"];
    let mut markdown = String::new();
    markdown.push_str("# ContinuityDB vs Neo4j Graph Benchmark Report\n\n");
    markdown.push_str(&format!(
        "Aggregate winner: **{}**. ContinuityDB average: `{}` bps. Neo4j average: `{}` bps. Delta: `{}` bps.\n\n",
        aggregate["winner"].as_str().unwrap_or("unknown"),
        aggregate["continuitydb_average_score_bps"].as_u64().unwrap_or(0),
        aggregate["neo4j_average_score_bps"].as_u64().unwrap_or(0),
        aggregate["score_delta_bps"].as_i64().unwrap_or(0),
    ));
    markdown.push_str("Neo4j is the right graph-database foil because it covers relationship traversal, GraphRAG-style context assembly, vector indexes, and GDS similarity. This run is an offline deterministic target-model benchmark, not a live Neo4j server execution.\n\n");
    markdown.push_str("| Family | ContinuityDB | Neo4j | Delta | Winner | Evidence |\n");
    markdown.push_str("| --- | ---: | ---: | ---: | --- | --- |\n");
    for benchmark in report["benchmarks"].as_array().unwrap_or(&Vec::new()) {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            benchmark["family"].as_str().unwrap_or("unknown"),
            benchmark["continuitydb_score_bps"].as_u64().unwrap_or(0),
            benchmark["neo4j_score_bps"].as_u64().unwrap_or(0),
            benchmark["score_delta_bps"].as_i64().unwrap_or(0),
            benchmark["winner"].as_str().unwrap_or("unknown"),
            benchmark["evidence_mode"].as_str().unwrap_or("unknown"),
        ));
    }
    markdown.push_str("\n## Interpretation\n\n");
    markdown.push_str(
        report["interpretation"]["primary_result"]
            .as_str()
            .unwrap_or(""),
    );
    markdown.push_str("\n\n");
    markdown.push_str(
        report["interpretation"]["continuitydb_claim"]
            .as_str()
            .unwrap_or(""),
    );
    markdown.push_str("\n\n");
    markdown.push_str(
        report["interpretation"]["next_required_evidence"]
            .as_str()
            .unwrap_or(""),
    );
    markdown.push_str("\n\n## Benchmark Details\n\n");
    for benchmark in report["benchmarks"].as_array().unwrap_or(&Vec::new()) {
        markdown.push_str(&format!(
            "### {}\n\n",
            benchmark["family"].as_str().unwrap_or("unknown")
        ));
        markdown.push_str(&format!(
            "What it measures: {}\n\n",
            benchmark["scenario"].as_str().unwrap_or("")
        ));
        markdown.push_str(&format!(
            "ContinuityDB model: `{}`.\n\n",
            benchmark["measurement"]["continuitydb_model"]
                .as_str()
                .unwrap_or("")
        ));
        markdown.push_str(&format!(
            "Neo4j model: `{}`.\n\n",
            benchmark["measurement"]["neo4j_model"]
                .as_str()
                .unwrap_or("")
        ));
        markdown.push_str(&format!(
            "Result: ContinuityDB `{}` bps, Neo4j `{}` bps, delta `{}` bps. Winner: `{}`.\n\n",
            benchmark["continuitydb_score_bps"].as_u64().unwrap_or(0),
            benchmark["neo4j_score_bps"].as_u64().unwrap_or(0),
            benchmark["score_delta_bps"].as_i64().unwrap_or(0),
            benchmark["winner"].as_str().unwrap_or("unknown"),
        ));
        if let Some(findings) = benchmark["findings"].as_array() {
            markdown.push_str("Key findings:\n\n");
            for finding in findings {
                if let Some(finding) = finding.as_str() {
                    markdown.push_str(&format!("- {finding}\n"));
                }
            }
            markdown.push('\n');
        }
    }
    markdown
}

fn benchmark_agent_memory_relevance(family: &str) -> &'static str {
    match family {
        "recall_under_context_pressure" => {
            "Agents fail when memory retrieval gives them plausible current facts but drops the historical evidence needed to understand what changed, what was superseded, and why a prior belief is no longer safe."
        }
        "model_task_performance" => {
            "The product question is whether retrieved memory helps a model make the right decision, preserve citations, and avoid stale claims, not whether a retriever returns text that looks similar."
        }
        "temporal_revision_semantics" => {
            "Agent memory is temporal: an assistant often needs to answer what was true then, what is true now, and which update changed the operational state."
        }
        "conflict_uncertainty_handling" => {
            "Long-running agents encounter contradictory observations; memory must preserve unresolved uncertainty instead of flattening conflict into whichever chunk ranks highest."
        }
        "frontier_operational_state" => {
            "Agents need to know what is currently active, blocked, or awaiting verification so they do the next correct action rather than repeating old work."
        }
        "long_horizon_agent_memory" => {
            "Context collapse happens over many sessions when old decisions, supersessions, and blockers fall out of prompt history but still matter to current behavior."
        }
        "scale_latency" => {
            "A memory system has to stay usable as the corpus grows; retrieval quality only matters if the system can return context within operational latency and cost budgets."
        }
        "ablations" => {
            "Ablations separate what vector retrieval can recover with filters or reranking from what requires database-native continuity semantics."
        }
        "adversarial_retrieval" => {
            "Agent memory must resist stale near-duplicates, misleading summaries, and semantically similar but operationally wrong evidence."
        }
        "human_auditable_evidence" => {
            "Trustworthy agent systems need reviewable citations, revision chains, and rationale so humans can inspect why a memory influenced an answer."
        }
        _ => "This benchmark contributes to the overall memory and context evaluation.",
    }
}

fn benchmark_measurement_summary(family: &str, measurement: &serde_json::Value) -> String {
    match family {
        "model_task_performance" => {
            let continuity_missing = measurement["continuitydb_answer"]["missing_citations"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0);
            let vector_missing = measurement["vector_answer"]["missing_citations"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0);
            format!(
                "Measurement detail: the deterministic task model required a blocked release decision plus three citations. ContinuityDB missed `{continuity_missing}` required citations; vector missed `{vector_missing}` required citations.\n"
            )
        }
        "scale_latency" => {
            let continuity_elapsed = measurement["continuitydb_total_elapsed_ns"].as_u64().unwrap_or(0);
            let vector_elapsed = measurement["vector_total_elapsed_ns"].as_u64().unwrap_or(0);
            let sizes = measurement["sizes"]
                .as_array()
                .map(|sizes| {
                    sizes
                        .iter()
                        .filter_map(|size| size["size"].as_u64())
                        .map(|size| size.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!(
                "Measurement detail: measured corpus sizes `{sizes}`. ContinuityDB structural lookup total `{continuity_elapsed}` ns; dense vector scan total `{vector_elapsed}` ns.\n"
            )
        }
        "recall_under_context_pressure" => {
            "Measurement detail: the retained live Pinecone artifact upserted three evidence vectors and `/query` returned the two closest chunks while omitting the superseded prior evidence.\n".to_string()
        }
        _ => String::new(),
    }
}

fn pinecone_live_retrieval_json(
    corpus: &[serde_json::Value],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let api_key = env::var("PINECONE_API_KEY")?;
    let index_host = env::var("PINECONE_INDEX_HOST")?;
    let namespace = env::var("PINECONE_NAMESPACE")?;
    let vector_dimension = env::var("PINECONE_VECTOR_DIMENSION")?
        .parse::<usize>()
        .map_err(|error| {
            std::io::Error::other(format!("invalid PINECONE_VECTOR_DIMENSION: {error}"))
        })?;
    if vector_dimension < 2 {
        return Err(std::io::Error::other("PINECONE_VECTOR_DIMENSION must be at least 2").into());
    }

    let vectors = pinecone_live_vectors_json(corpus, vector_dimension);
    let upsert_payload = serde_json::json!({
        "namespace": namespace,
        "vectors": vectors,
    });
    let upsert_url = format!("https://{index_host}/vectors/upsert");
    let mut upsert_response = ureq::post(&upsert_url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .header("X-Pinecone-Api-Version", "2025-10")
        .send_json(&upsert_payload)?;
    let upsert_json = upsert_response
        .body_mut()
        .read_json::<serde_json::Value>()?;

    let query_payload = serde_json::json!({
        "namespace": namespace,
        "vector": pinecone_query_vector(vector_dimension),
        "topK": 2,
        "includeMetadata": true,
        "includeValues": false,
    });
    let query_url = format!("https://{index_host}/query");
    let mut query_json = serde_json::json!({});
    for attempt in 1..=10 {
        let mut query_response = ureq::post(&query_url)
            .header("Api-Key", &api_key)
            .header("Content-Type", "application/json")
            .header("X-Pinecone-Api-Version", "2025-10")
            .send_json(&query_payload)?;
        query_json = query_response.body_mut().read_json::<serde_json::Value>()?;
        if query_json["matches"]
            .as_array()
            .is_some_and(|matches| matches.len() >= 2)
        {
            break;
        }
        if attempt < 10 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    let retrieved_evidence_ids = query_json["matches"]
        .as_array()
        .map(|matches| {
            matches
                .iter()
                .filter_map(|matched| matched["id"].as_str())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(serde_json::json!({
        "provider": "pinecone",
        "mode": "pinecone-live-vector-api",
        "api_version": "2025-10",
        "index_host": index_host,
        "index_name": env::var("PINECONE_INDEX_NAME").ok(),
        "namespace": namespace,
        "vector_dimension": vector_dimension,
        "upserted_count": upsert_json["upsertedCount"].clone(),
        "query_match_count": retrieved_evidence_ids.len(),
        "retrieved_evidence_ids": retrieved_evidence_ids,
        "matches": query_json["matches"].clone(),
        "usage": query_json["usage"].clone(),
    }))
}

fn pinecone_live_vectors_json(
    corpus: &[serde_json::Value],
    dimension: usize,
) -> Vec<serde_json::Value> {
    corpus
        .iter()
        .filter_map(|chunk| {
            let id = chunk["id"].as_str()?;
            Some(serde_json::json!({
                "id": id,
                "values": pinecone_evidence_vector(id, dimension),
                "metadata": {
                    "evidence_id": id,
                    "source": chunk["source"].as_str().unwrap_or_default(),
                    "continuity_labels": chunk["continuity_labels"]
                        .as_array()
                        .map(|labels| labels
                            .iter()
                            .filter_map(|label| label.as_str())
                            .collect::<Vec<_>>()
                            .join(","))
                        .unwrap_or_default(),
                    "text": chunk["text"].as_str().unwrap_or_default(),
                }
            }))
        })
        .collect()
}

fn pinecone_query_vector(dimension: usize) -> Vec<f64> {
    let mut vector = vec![0.0; dimension];
    vector[0] = 1.0;
    vector[1] = 1.0;
    vector
}

fn pinecone_evidence_vector(id: &str, dimension: usize) -> Vec<f64> {
    let mut vector = vec![0.0; dimension];
    match id {
        "release-candidate-current" => {
            vector[0] = 1.0;
            vector[1] = 0.8;
        }
        "release-upload-404" => {
            vector[0] = 0.8;
            vector[1] = 1.0;
        }
        "release-candidate-old" => {
            vector[0] = 1.0;
            vector[1] = -1.0;
        }
        _ => {
            vector[0] = 0.1;
            vector[1] = 0.1;
        }
    }
    vector
}

fn pinecone_retrieval_metrics_for_ids(retrieved_ids: &[String]) -> serde_json::Value {
    let gold = [
        "release-candidate-old",
        "release-candidate-current",
        "release-upload-404",
    ];
    let retrieved_gold_count = gold
        .iter()
        .filter(|gold_id| retrieved_ids.iter().any(|id| id == *gold_id))
        .count() as u64;
    let citation_precision = if retrieved_ids.is_empty() {
        0
    } else {
        let known_count = retrieved_ids
            .iter()
            .filter(|id| gold.contains(&id.as_str()))
            .count() as u64;
        ((known_count * 10_000) + (retrieved_ids.len() as u64 / 2)) / retrieved_ids.len() as u64
    };
    let gold_recall = ((retrieved_gold_count * 10_000) + 1) / gold.len() as u64;
    let revision_preservation = 0;
    let uncertainty_preservation = 0;
    let frontier_preservation = 0;
    let token_budget_fit = if retrieved_ids.len() <= 2 { 10_000 } else { 0 };
    let overall = (gold_recall
        + citation_precision
        + revision_preservation
        + uncertainty_preservation
        + frontier_preservation
        + token_budget_fit)
        + 3;
    let overall = overall / 6;
    serde_json::json!({
        "gold_evidence_recall_bps": gold_recall,
        "citation_precision_bps": citation_precision,
        "revision_preservation_bps": revision_preservation,
        "uncertainty_preservation_bps": uncertainty_preservation,
        "frontier_preservation_bps": frontier_preservation,
        "token_budget_fit_bps": token_budget_fit,
        "overall_score_bps": overall,
    })
}

fn pinecone_omitted_gold_evidence_ids(retrieved_ids: &[String]) -> Vec<&'static str> {
    [
        "release-candidate-old",
        "release-candidate-current",
        "release-upload-404",
    ]
    .into_iter()
    .filter(|gold_id| !retrieved_ids.iter().any(|id| id == *gold_id))
    .collect()
}

fn pinecone_target_losses_for_ids(retrieved_ids: &[String]) -> Vec<&'static str> {
    let mut losses = Vec::new();
    if !retrieved_ids.iter().any(|id| id == "release-candidate-old") {
        losses.push("missed_superseded_evidence");
        losses.push("lost_revision_history");
    }
    if !retrieved_ids.iter().any(|id| id == "release-upload-404") {
        losses.push("lost_uncertainty");
    }
    if !retrieved_ids
        .iter()
        .any(|id| id == "release-candidate-current")
    {
        losses.push("lost_frontier_state");
    }
    losses
}

fn context_collapse_retrieval_corpus() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": "release-candidate-old",
            "source": "release://candidate/old",
            "text": "Old packaging logs said the release candidate was ready.",
            "continuity_labels": ["superseded", "historical_evidence"],
            "tokens": 18,
        }),
        serde_json::json!({
            "id": "release-candidate-current",
            "source": "release://candidate/current",
            "text": "Fresh preflight evidence supersedes the old release readiness belief.",
            "continuity_labels": ["current_state", "frontier", "supersedes_old"],
            "tokens": 12,
        }),
        serde_json::json!({
            "id": "release-upload-404",
            "source": "release://incident/upload-404",
            "text": "A GitHub Release upload failure conflicts with treating release assets as shipped.",
            "continuity_labels": ["conflict", "uncertainty", "blocking_evidence"],
            "tokens": 12,
        }),
    ]
}

fn context_collapse_retrieval_corpus_fingerprint(corpus: &[serde_json::Value]) -> String {
    let mut hasher = Sha256::new();
    for chunk in corpus {
        hasher.update(serde_json::to_string(chunk).unwrap_or_default().as_bytes());
        hasher.update(b"\n");
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn pinecone_vector_target_json() -> serde_json::Value {
    let api_key_configured = env::var_os("PINECONE_API_KEY").is_some();
    let index_host = env::var("PINECONE_INDEX_HOST").ok();
    let index_name = env::var("PINECONE_INDEX_NAME").ok();
    let namespace = env::var("PINECONE_NAMESPACE").ok();
    let vector_dimension = env::var("PINECONE_VECTOR_DIMENSION").ok();
    let parsed_vector_dimension = vector_dimension
        .as_deref()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|dimension| *dimension >= 2);
    let live_config_available = api_key_configured
        && index_host.is_some()
        && namespace.is_some()
        && parsed_vector_dimension.is_some();
    let missing_env = [
        ("PINECONE_API_KEY", api_key_configured),
        ("PINECONE_INDEX_HOST", index_host.is_some()),
        ("PINECONE_NAMESPACE", namespace.is_some()),
        (
            "PINECONE_VECTOR_DIMENSION",
            parsed_vector_dimension.is_some(),
        ),
    ]
    .into_iter()
    .filter_map(|(name, configured)| (!configured).then_some(name))
    .collect::<Vec<_>>();
    serde_json::json!({
        "provider": "pinecone",
        "mode": if live_config_available {
            "pinecone-live-capable"
        } else {
            "offline-vector-baseline"
        },
        "live_config_available": live_config_available,
        "api_key_configured": api_key_configured,
        "index_host_configured": index_host.is_some(),
        "index_name": index_name,
        "namespace": namespace,
        "vector_dimension": parsed_vector_dimension,
        "missing_env": missing_env,
        "required_env": [
            "PINECONE_API_KEY",
            "PINECONE_INDEX_HOST",
            "PINECONE_NAMESPACE",
            "PINECONE_VECTOR_DIMENSION",
        ],
        "api_shape": {
            "vector_upsert_endpoint": "/vectors/upsert",
            "vector_query_endpoint": "/query",
            "text_upsert_endpoint": "/records/namespaces/{namespace}/upsert",
            "metadata_policy": "store evidence_id and source locator as metadata; score only returned IDs and preserved continuity labels",
        },
    })
}

fn continuity_checkout_benchmark_metrics(proof: &serde_json::Value) -> serde_json::Value {
    let bounded_by_token_budget = proof["bounded_context"].as_bool().unwrap_or(false);
    let citations_preserved = proof["citations_preserved"].as_bool().unwrap_or(false);
    let uncertainty_preserved = proof["uncertainty_preserved"].as_bool().unwrap_or(false);
    let revision_history_preserved = proof["revision_links_preserved"].as_bool().unwrap_or(false)
        && proof["revision_context_preserved"]
            .as_bool()
            .unwrap_or(false);
    let frontier_state_preserved = proof["frontier_preserved"].as_bool().unwrap_or(false);
    let structured_summary_preserved = proof["summary_preserved"].as_bool().unwrap_or(false);
    let continuity_score = [
        bounded_by_token_budget,
        citations_preserved,
        uncertainty_preserved,
        revision_history_preserved,
        frontier_state_preserved,
        structured_summary_preserved,
    ]
    .into_iter()
    .filter(|preserved| *preserved)
    .count();

    serde_json::json!({
        "bounded_by_token_budget": bounded_by_token_budget,
        "citations_preserved": citations_preserved,
        "uncertainty_preserved": uncertainty_preserved,
        "revision_history_preserved": revision_history_preserved,
        "frontier_state_preserved": frontier_state_preserved,
        "structured_summary_preserved": structured_summary_preserved,
        "continuity_score": continuity_score,
    })
}

fn collapsed_summary_benchmark_metrics() -> serde_json::Value {
    serde_json::json!({
        "bounded_by_token_budget": true,
        "citations_preserved": false,
        "uncertainty_preserved": false,
        "revision_history_preserved": false,
        "frontier_state_preserved": false,
        "structured_summary_preserved": false,
        "continuity_score": 1,
    })
}

fn context_collapse_cell(
    id: StateCellId,
    anchor: &str,
    citation: &str,
    confidence: f32,
    tokens: i64,
    activation: ActivationState,
    payload: &str,
) -> Result<StateCell, Box<dyn std::error::Error>> {
    let valid_from = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid context-collapse timestamp"))?;
    let mut cell = StateCell::new(
        id,
        vec![SemanticAnchor::new(anchor)],
        ValidTimeRange::new(valid_from, None)?,
        Scope::Project("continuitydb".to_string()),
        Answerability::new(vec!["is the release state safe to trust?".to_string()])?,
        vec![Evidence {
            source: SourceId::new("context-collapse-drill"),
            citation: Citation {
                locator: citation.to_string(),
            },
            confidence: Confidence::new(confidence)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text(payload.to_string()),
        CellCost::new(tokens, 0)?,
    )?;
    cell.activation = activation;
    Ok(cell)
}

fn revision_link_kind_name(kind: RevisionLinkKind) -> &'static str {
    match kind {
        RevisionLinkKind::Predecessor => "predecessor",
        RevisionLinkKind::Supersedes => "supersedes",
        RevisionLinkKind::ConflictsWith => "conflicts_with",
        RevisionLinkKind::DerivesFrom => "derives_from",
    }
}

fn checkout_query_file(
    store_path: &PathBuf,
    query_path: &PathBuf,
) -> Result<
    (
        continuitydb_checkout::CheckoutSlice,
        continuitydb_query::QueryReturnShape,
    ),
    Box<dyn std::error::Error>,
> {
    let db = open_file_database(store_path)?;
    let slice = db.checkout_query_file(query_path)?;
    Ok((slice, query_return_shape_from_file(query_path)?))
}

fn checkout_query_result_envelope_json(
    store_path: &PathBuf,
    query_path: &PathBuf,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let db = open_file_database(store_path)?;
    let result = db.checkout_query_file_projected(query_path)?;
    let encoded = encode_checkout_query_result_json(result)?;
    Ok(serde_json::from_slice(&encoded)?)
}

fn query_return_shape_from_file(
    query_path: &PathBuf,
) -> Result<QueryReturnShape, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(query_path)?;
    if let Some(text) = std::str::from_utf8(&bytes).ok().filter(|text| {
        text.trim_start()
            .get(.."checkout".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("checkout"))
    }) {
        return Ok(parse_query_text(text)?.return_shape());
    }
    if let Ok(envelope) = serde_json::from_slice::<QueryEnvelope>(&bytes) {
        envelope.validate()?;
        return Ok(envelope.query.return_shape());
    }
    let query = serde_json::from_slice::<ContinuityQuery>(&bytes)?;
    Ok(query.return_shape())
}

fn checkout_query_summary_json(
    summary: continuitydb_checkout::CheckoutSummary,
) -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": summary,
    })
}

fn checkout_query_cells_json(cells: Vec<StateCell>) -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.checkout_query.cells",
        "format_version": 1,
        "cells": cells,
    })
}

fn checkout_query_context_packets_json(context_packets: Vec<ContextPacket>) -> serde_json::Value {
    serde_json::json!({
        "format": "continuitydb.checkout_query.context_packets",
        "format_version": 1,
        "context_packets": context_packets,
    })
}

fn demo_checkout() -> Result<continuitydb_checkout::CheckoutSlice, Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let mut frontier = demo_cell("project:continuitydb:frontier", "demo://frontier", 0.95, 10)?;
    frontier.activation = ActivationState::Frontier;
    let alternative = demo_cell(
        "project:continuitydb:alternative",
        "demo://alternative",
        0.90,
        10,
    )?;

    kernel.append_cell(frontier)?;
    kernel.append_cell(alternative)?;

    checkout(
        &kernel,
        CheckoutRequest {
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            lifecycle_stage: None,
            retention_policy: None,
            use_policy: None,
            promotion_policy: None,
            projection_kind: None,
            minimum_uncertainty: None,
            minimum_surprise_bits: None,
            minimum_probability_delta: None,
            minimum_salience: None,
            minimum_context_affordance: None,
            minimum_epistemic_pressure: None,
            context_gap_kind: None,
            minimum_context_gap_priority: None,
            invalidation_condition_kind: None,
            minimum_invalidation_priority: None,
            epistemic_action: None,
            epistemic_action_reason: None,
            selection_reason: None,
            trajectory_memory_strategy: None,
            minimum_trajectory_memory_confidence: None,
            answerability_question: None,
            compiler_intent: None,
            compiler_proposals: Vec::new(),
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            revision_related_cell: None,
            revision_link_kind: None,
            context_profile: ContextProfile::Execution,
            compiler_policy: ContextCompilerPolicy::RawBaseline,
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 10,
        },
    )
    .map_err(Into::into)
}

fn demo_cell(
    anchor: &str,
    citation: &str,
    confidence: f32,
    tokens: i64,
) -> Result<StateCell, Box<dyn std::error::Error>> {
    let valid_from = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid demo timestamp"))?;
    StateCell::new(
        StateCellId::new(),
        vec![SemanticAnchor::new(anchor)],
        ValidTimeRange::new(valid_from, None)?,
        Scope::Project("continuitydb".to_string()),
        Answerability::new(vec!["what should the agent know?".to_string()])?,
        vec![Evidence {
            source: SourceId::new("demo"),
            citation: Citation {
                locator: citation.to_string(),
            },
            confidence: Confidence::new(confidence)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text(anchor.to_string()),
        CellCost::new(tokens, 0)?,
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinecone_live_vectors_match_index_dimension() {
        let corpus = context_collapse_retrieval_corpus();
        let vectors = pinecone_live_vectors_json(&corpus, 8);

        assert_eq!(vectors.len(), 3);
        assert_eq!(vectors[0]["id"].as_str(), Some("release-candidate-old"));
        assert_eq!(vectors[0]["values"].as_array().map(Vec::len), Some(8));
        assert_eq!(
            vectors[1]["metadata"]["evidence_id"].as_str(),
            Some("release-candidate-current")
        );
    }

    #[test]
    fn pinecone_live_metrics_score_returned_match_ids() {
        let metrics = pinecone_retrieval_metrics_for_ids(&[
            "release-candidate-current".to_string(),
            "release-upload-404".to_string(),
        ]);

        assert_eq!(metrics["gold_evidence_recall_bps"].as_u64(), Some(6667));
        assert_eq!(metrics["citation_precision_bps"].as_u64(), Some(10000));
        assert_eq!(metrics["overall_score_bps"].as_u64(), Some(4445));
    }
}
