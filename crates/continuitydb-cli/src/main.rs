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
    measure_ingest_and_checkout, FileWorkloadBaselineStore, WorkloadBaselineComparison,
    WorkloadBaselineRecord, WorkloadConfig, WorkloadMeasurement, WorkloadMeasurementSnapshot,
    WorkloadRegressionThresholds,
};
#[cfg(feature = "local-model")]
use std::path::Path;
use std::path::PathBuf;

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
    dry_run: bool,
    compare_baseline: bool,
    fail_on_regression: bool,
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
    let effective_contract_dir = options.contract_dir.or(artifact_contract_dir.as_deref());
    let effective_prompt_dir = options.prompt_dir.or(artifact_prompt_dir.as_deref());
    let effective_response_dir = options.response_dir.or(artifact_response_dir.as_deref());
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
        if options.artifact_dir.is_some() || options.failure_report_path.is_some() {
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
        if let Some(artifact_dir) = options.artifact_dir {
            report = write_local_model_artifact_bundle_report(artifact_dir, report)?;
        }
        if let Some(report_path) = options.failure_report_path {
            write_pretty_json_file(report_path, &report)?;
        }
        return Err(std::io::Error::other("local model benchmark regression detected").into());
    }
    store.append_baseline(current_baseline.clone())?;

    Ok(local_model_benchmark_json(
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
    ))
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
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
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
    let manifest = serde_json::json!({
        "format": "continuitydb.local_model.benchmark_bundle",
        "format_version": 1,
        "benchmark_report_path": report_path.display().to_string(),
        "contract_artifacts": report["contract_artifacts"].clone(),
        "prompt_artifacts": report["prompt_artifacts"].clone(),
        "response_artifacts": report["response_artifacts"].clone(),
        "response_artifact_manifest": report["response_artifact_manifest"].clone(),
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
                measure_ingest_and_checkout(&mut memory, &workload, committed_at, request)?,
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

    let regression_detected = match comparison.as_ref() {
        Some(comparison) => !comparison.passed(),
        None => false,
    };
    if options.fail_on_regression && regression_detected {
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

    Ok(workload_measurement_json(
        options.kernel,
        options.store_path,
        options.baseline_path,
        options.label,
        comparison.as_ref(),
        lookup_plan,
        measurement,
    ))
}

fn workload_measurement_json(
    kernel: WorkloadKernelProfile,
    store_path: Option<&PathBuf>,
    baseline_path: Option<&PathBuf>,
    label: &str,
    comparison: Option<&WorkloadBaselineComparison>,
    lookup_plan: Option<continuitydb_kernel::FileKernelLookupPlan>,
    measurement: WorkloadMeasurement,
) -> serde_json::Value {
    serde_json::json!({
        "kernel": workload_kernel_name(kernel),
        "store_path": store_path.map(|path| path.display().to_string()),
        "baseline_path": baseline_path.map(|path| path.display().to_string()),
        "baseline_label": baseline_path.map(|_| label),
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
        "candidate_count": plan.candidate_count,
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
