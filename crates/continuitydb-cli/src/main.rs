//! ContinuityDB command-line interface.

use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use continuitydb_api::{ContinuityDb, ContinuityError};
use continuitydb_checkout::{cell_lookup_from_checkout_request, checkout, CheckoutRequest};
#[cfg(feature = "local-model")]
use continuitydb_core::RevisionLinkKind;
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellPayload, Citation, CommitId, Confidence,
    Evidence, Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
};
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, KernelCapabilities, KernelDurability, KernelRequirements,
    StorageKernel,
};
use continuitydb_memory::MemoryKernel;
#[cfg(feature = "local-model")]
use continuitydb_steward::{
    default_steward_evaluation_suite, local_model_prompt_fingerprint_for_suite,
    local_model_prompt_for_input, local_model_response_gbnf_grammar,
    local_model_response_json_schema, small_model_candidates, FileLocalModelBenchmarkBaselineStore,
    LocalExecutableRunner, LocalExecutableRunnerConfig, LocalModelBenchmark,
    LocalModelBenchmarkBaseline, LocalModelBenchmarkBaselineStore, LocalModelBenchmarkRegression,
    LocalModelStabilityReport, SmallModelCandidate, StewardAction, StewardEvaluationCaseResponse,
    StewardEvaluationSuite, StewardIdentity, LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
};
use continuitydb_workload::{
    compare_workload_snapshot_to_baseline, generate_world_model_workload,
    measure_ingest_and_checkout, ContinuityWorkload, FileWorkloadBaselineStore,
    WorkloadBaselineComparison, WorkloadBaselineRecord, WorkloadConfig, WorkloadMeasurement,
    WorkloadMeasurementSnapshot, WorkloadRegressionThresholds, WorkloadSummary,
};
use std::path::{Path, PathBuf};

/// ContinuityDB command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "continuitydb",
    version,
    about = "Embeddable datastore for agent world models"
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
    candidate_id: &'a str,
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
    /// Print deterministic demo checkout JSON.
    DemoCheckout,
    /// Execute a serialized typed Continuity query JSON file against a file-backed store.
    CheckoutQuery {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to a serialized ContinuityQuery JSON file.
        query_path: PathBuf,
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
        #[arg(long = "candidate", default_value = "Qwen/Qwen2.5-0.5B-Instruct")]
        candidate: String,
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
    },
    /// Write local Steward model JSON Schema and GBNF grammar artifacts.
    #[cfg(feature = "local-model")]
    LocalModelContract {
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
    /// Print the fixed local Steward model candidate registry as JSON.
    #[cfg(feature = "local-model")]
    LocalModelCandidates,
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
        Some(Command::DemoCheckout) => {
            let slice = demo_checkout()?;
            println!("{}", serde_json::to_string_pretty(&slice)?);
        }
        Some(Command::CheckoutQuery {
            store_path,
            query_path,
        }) => {
            let slice = checkout_query_file(&store_path, &query_path)?;
            println!("{}", serde_json::to_string_pretty(&slice)?);
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
            require,
            require_canonical,
            lookup_plan,
            lookup_query,
        }) => {
            let db = if let Some(profile) = require {
                open_file_database_with_profile(&store_path, profile)?
            } else {
                open_file_database(&store_path)?
            };
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
            let required = require.map(profile_name);
            let satisfies = require
                .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
                .unwrap_or(true);
            let output = serde_json::json!({
                "path": store_path.display().to_string(),
                "capabilities": capabilities_json(capabilities),
                "status": status,
                "health": health,
                "lookup_plan": lookup_plan,
                "required": required,
                "satisfies": satisfies,
            });
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
                candidate_id: &candidate,
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
        }) => {
            let validation = validate_local_model_bundle_manifest(&artifact_dir)?;
            let output = serde_json::json!({
                "artifact_dir": artifact_dir.display().to_string(),
                "report_path": report_path.as_ref().map(|path| path.display().to_string()),
                "manifest": local_model_bundle_manifest_json(Some(&validation.manifest)),
                "benchmark_report": validation.benchmark_report,
                "changed_case_report": validation.changed_case_report,
                "response_artifact_manifest": validation.response_artifact_manifest,
            });
            if let Some(path) = report_path.as_ref() {
                write_pretty_json_file(path, &output)?;
            }
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelContract {
            schema_path,
            grammar_path,
        }) => {
            let output = write_local_model_contract_json(&schema_path, &grammar_path)?;
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelEvaluationSuite) => {
            let output = local_model_evaluation_suite_json();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        #[cfg(feature = "local-model")]
        Some(Command::LocalModelCandidates) => {
            let output = local_model_candidates_json();
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
        "default_candidate": candidates.first().map(SmallModelCandidate::model_id),
        "total_candidates": candidates.len(),
        "candidates": candidates
            .iter()
            .map(|candidate| {
                let recommended_config =
                    candidate.recommended_runner_config("llama-cli", "<model.gguf>");
                serde_json::json!({
                    "model_id": candidate.model_id(),
                    "role": candidate.role(),
                    "recommended_runtime": candidate.recommended_runtime(),
                    "artifact_format": candidate.artifact_format(),
                    "recommended_temperature": candidate.recommended_temperature(),
                    "requires_grammar": candidate.requires_grammar(),
                    "recommended_runner_arguments": recommended_config.command_arguments(),
                    "notes": candidate.notes(),
                })
            })
            .collect::<Vec<_>>(),
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
        "evaluation_suite_fingerprint": suite.fingerprint(),
        "total_cases": suite.len(),
        "cases": cases,
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
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let schema = local_model_response_json_schema();
    let grammar = local_model_response_gbnf_grammar();
    std::fs::write(schema_path, schema)?;
    std::fs::write(grammar_path, grammar)?;

    Ok(serde_json::json!({
        "schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "schema_path": schema_path.display().to_string(),
        "grammar_path": grammar_path.display().to_string(),
        "schema_fingerprint": local_model_contract_fingerprint(schema),
        "grammar_fingerprint": local_model_contract_fingerprint(grammar),
    }))
}

#[cfg(feature = "local-model")]
fn benchmark_local_model_json(
    options: LocalModelBenchmarkOptions<'_>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let candidate = local_model_candidate(options.candidate_id)?;
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
            let current_baseline = LocalModelBenchmarkBaseline::from_report(report, Utc::now());
            let mut report = local_model_benchmark_json(
                options.baseline_path,
                options.compare_baseline,
                &current_baseline,
                None,
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
    let current_baseline = LocalModelBenchmarkBaseline::from_report(report, Utc::now());
    if options.fail_on_failed_cases && !current_baseline.evaluation_summary().passed() {
        let mut report = local_model_benchmark_json(
            options.baseline_path,
            options.compare_baseline,
            &current_baseline,
            None,
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
    config: &LocalExecutableRunnerConfig,
    baseline_path: &Path,
    artifacts: LocalModelBenchmarkArtifacts<'_>,
    gates: LocalModelBenchmarkDryRunGates,
) -> serde_json::Value {
    let mut value = serde_json::json!({
        "dry_run": true,
        "will_record_baseline": false,
        "candidate_model_id": candidate.model_id(),
        "candidate_role": candidate.role(),
        "baseline_path": baseline_path.display().to_string(),
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "evaluation_suite_fingerprint": default_steward_evaluation_suite().fingerprint(),
        "schema_fingerprint": local_model_contract_fingerprint(local_model_response_json_schema()),
        "grammar_fingerprint": local_model_contract_fingerprint(local_model_response_gbnf_grammar()),
        "prompt_fingerprint": local_model_prompt_fingerprint_for_suite(&default_steward_evaluation_suite()),
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

    Ok(serde_json::json!({
        "compared": true,
        "compatible_baseline_found": latest.is_some(),
        "previous_recorded_at": latest.as_ref().map(LocalModelBenchmarkBaseline::recorded_at),
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
    let schema_path = contract_dir.join("local-model-response.schema.json");
    let grammar_path = contract_dir.join("local-model-response.gbnf");
    std::fs::write(&schema_path, schema)?;
    std::fs::write(&grammar_path, grammar)?;

    Ok(LocalModelContractArtifacts {
        schema_path,
        grammar_path,
        schema_fingerprint: local_model_contract_fingerprint(schema),
        grammar_fingerprint: local_model_contract_fingerprint(grammar),
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
                serde_json::json!({
                    "case_name": artifact.case_name,
                    "captured": artifact.captured,
                    "response_path": artifact.response_path.as_ref().map(|path| path.display().to_string()),
                    "response_fingerprint": artifact.response_fingerprint,
                    "response_bytes": artifact.response_bytes,
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
            serde_json::json!({
                "manifest_path": manifest.manifest_path.display().to_string(),
                "manifest_fingerprint": manifest.manifest_fingerprint,
                "manifest_bytes": manifest.manifest_bytes,
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
    let manifest = serde_json::json!({
        "format": "continuitydb.local_model.benchmark_bundle",
        "format_version": 1,
        "benchmark_report_path": report_path.display().to_string(),
        "benchmark_report_fingerprint": local_model_contract_fingerprint(&report_text),
        "benchmark_report_bytes": report_text.len(),
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
    let response_artifact_manifest =
        validate_local_model_response_artifact_manifest(artifact_dir, &manifest, &report)?;

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
        }),
        changed_case_report,
        response_artifact_manifest,
    })
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

    Ok(serde_json::json!({
        "report_path": report_path.display().to_string(),
        "report_fingerprint": current_report_fingerprint,
        "report_bytes": report_text.len(),
    }))
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
            serde_json::json!({
                "manifest_path": manifest.manifest_path.display().to_string(),
                "manifest_fingerprint": manifest.manifest_fingerprint,
                "manifest_bytes": manifest.manifest_bytes,
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
    artifacts: LocalModelBenchmarkArtifacts<'_>,
    stability: Option<&LocalModelStabilityReport>,
) -> serde_json::Value {
    let summary = baseline.evaluation_summary();
    let mut value = serde_json::json!({
        "candidate_model_id": baseline.candidate_model_id(),
        "candidate_role": baseline.candidate_role(),
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
        "schema_fingerprint": baseline.schema_fingerprint(),
        "grammar_fingerprint": baseline.grammar_fingerprint(),
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
        RequirementProfile::IndexedEmbedded => KernelRequirements::indexed_embedded(),
    }
}

fn profile_name(profile: RequirementProfile) -> &'static str {
    match profile {
        RequirementProfile::Ephemeral => "ephemeral",
        RequirementProfile::DurableAppendLog => "durable-append-log",
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
        answerability_question: None,
        evidence_source: None,
        dependency_target: None,
        dependency_kind: None,
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
    serde_json::json!({
        "manifest_path": manifest.manifest_path.display().to_string(),
        "manifest_fingerprint": manifest.manifest_fingerprint,
        "manifest_bytes": manifest.manifest_bytes,
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
            "checkout_request_path": checkout_request_path.display().to_string(),
            "checkout_request_fingerprint": fnv1a64_fingerprint(&request_text),
            "checkout_request_bytes": request_text.len(),
        },
        "lookup_plan": lookup_plan.map(file_lookup_plan_json),
        "workload": {
            "cell_count": measurement.workload_summary.cell_count,
            "frontier_count": measurement.workload_summary.frontier_count,
            "dependency_count": measurement.workload_summary.dependency_count,
            "total_token_cost": measurement.workload_summary.total_token_cost,
        },
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
    let output = serde_json::json!({
        "kernel": workload_kernel_name(options.kernel),
        "artifact_dir": options.artifact_dir.display().to_string(),
        "store_path": options.store_path.map(|path| path.display().to_string()),
        "report_path": options.report_path.map(|path| path.display().to_string()),
        "failure_report_path": options.failure_report_path.map(|path| path.display().to_string()),
        "replay_artifact_dir": options.replay_artifact_dir.map(|path| path.display().to_string()),
        "replay_bundle_manifest": serde_json::Value::Null,
        "input_bundle_manifest": serde_json::Value::Null,
        "workload_artifacts": {
            "cells_path": cells_path.display().to_string(),
            "cells_fingerprint": fnv1a64_fingerprint(cells_text),
            "cells_bytes": cells_text.len(),
            "checkout_request_path": checkout_request_path.display().to_string(),
            "checkout_request_fingerprint": fnv1a64_fingerprint(request_text),
            "checkout_request_bytes": request_text.len(),
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

    let report_text = std::fs::read_to_string(artifact_dir.join("workload-report.json"))?;
    let report: serde_json::Value = serde_json::from_str(&report_text)?;
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
    let workload_artifacts = report["workload_artifacts"].clone();

    Ok(WorkloadBundleValidation {
        manifest,
        workload_report: serde_json::json!({
            "report_path": report_path.display().to_string(),
            "report_fingerprint": report_fingerprint,
            "report_bytes": manifest_report_payload_text.len(),
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

    serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": fnv1a64_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
    })
}

fn workload_validation_failure_report_json(artifact_dir: &Path) -> serde_json::Value {
    let report_path = artifact_dir.join("workload-report.json");
    let Ok(report_text) = std::fs::read_to_string(&report_path) else {
        return serde_json::Value::Null;
    };
    let canonical_report_text = serde_json::from_str::<serde_json::Value>(&report_text)
        .ok()
        .and_then(|report| workload_report_manifest_payload_text(&report).ok())
        .unwrap_or_else(|| report_text.clone());

    serde_json::json!({
        "report_path": report_path.display().to_string(),
        "report_fingerprint": fnv1a64_fingerprint(&canonical_report_text),
        "report_bytes": canonical_report_text.len(),
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

    serde_json::json!({
        "cells_path": cells_path.display().to_string(),
        "cells_fingerprint": fnv1a64_fingerprint(&cells_text),
        "cells_bytes": cells_text.len(),
        "checkout_request_path": checkout_request_path.display().to_string(),
        "checkout_request_fingerprint": fnv1a64_fingerprint(&request_text),
        "checkout_request_bytes": request_text.len(),
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
    let manifest = serde_json::json!({
        "format": "continuitydb.workload.replay_bundle",
        "format_version": 1,
        "replay_report_path": report_path.display().to_string(),
        "replay_report_fingerprint": fnv1a64_fingerprint(&report_text),
        "replay_report_bytes": report_text.len(),
        "input_artifact_dir": input_artifact_dir.display().to_string(),
        "kernel": report["kernel"].clone(),
        "store_path": report["store_path"].clone(),
        "input_bundle_manifest": report["input_bundle_manifest"].clone(),
        "workload_artifacts": report["workload_artifacts"].clone(),
        "lookup_plan": report["lookup_plan"].clone(),
        "workload": report["workload"].clone(),
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
        answerability_question: serde_json::from_value(request["answerability_question"].clone())?,
        evidence_source: serde_json::from_value(request["evidence_source"].clone())?,
        dependency_target: serde_json::from_value(request["dependency_target"].clone())?,
        dependency_kind: serde_json::from_value(request["dependency_kind"].clone())?,
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

fn open_file_database_with_profile(
    path: &PathBuf,
    profile: RequirementProfile,
) -> Result<ContinuityDb<continuitydb_kernel::FileKernel>, Box<dyn std::error::Error>> {
    ContinuityDb::open_file_with_requirements(path, requirements_for_profile(profile))
        .map_err(Into::into)
}

fn checkout_query_file(
    store_path: &PathBuf,
    query_path: &PathBuf,
) -> Result<continuitydb_checkout::CheckoutSlice, Box<dyn std::error::Error>> {
    let db = open_file_database(store_path)?;
    db.checkout_query_file(query_path).map_err(Into::into)
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
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
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
