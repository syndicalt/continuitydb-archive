//! ContinuityDB command-line interface.

use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use continuitydb_api::{ContinuityDb, ContinuityError};
use continuitydb_checkout::{checkout, CheckoutRequest};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellPayload, Citation, CommitId, Confidence,
    Evidence, Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
};
use continuitydb_kernel::{
    CommitManifestLookup, KernelCapabilities, KernelDurability, KernelRequirements, StorageKernel,
};
use continuitydb_memory::MemoryKernel;
#[cfg(feature = "local-model")]
use continuitydb_steward::{
    default_steward_evaluation_suite, local_model_response_gbnf_grammar,
    local_model_response_json_schema, record_local_model_benchmark_baseline_with_regression,
    small_model_candidates, FileLocalModelBenchmarkBaselineStore, LocalExecutableRunner,
    LocalExecutableRunnerConfig, LocalModelBenchmark, LocalModelBenchmarkBaseline,
    LocalModelBenchmarkRegression, SmallModelCandidate, StewardIdentity,
    LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
};
use continuitydb_workload::{
    compare_workload_snapshot_to_baseline, generate_world_model_workload,
    measure_ingest_and_checkout, FileWorkloadBaselineStore, WorkloadBaselineComparison,
    WorkloadBaselineRecord, WorkloadConfig, WorkloadMeasurement, WorkloadMeasurementSnapshot,
    WorkloadRegressionThresholds,
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
        /// JSONL path to append a benchmark baseline record.
        #[arg(long = "baseline-path")]
        baseline_path: PathBuf,
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
            let required = require.map(profile_name);
            let satisfies = require
                .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
                .unwrap_or(true);
            let output = serde_json::json!({
                "path": store_path.display().to_string(),
                "capabilities": capabilities_json(capabilities),
                "status": status,
                "health": health,
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
            baseline_path,
            compare_baseline,
            fail_on_regression,
        }) => {
            let output = benchmark_local_model_json(
                &candidate,
                &executable,
                &model_path,
                &arguments,
                &baseline_path,
                compare_baseline || fail_on_regression,
                fail_on_regression,
            )?;
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
        None => {}
    }
    Ok(())
}

#[cfg(feature = "local-model")]
fn write_local_model_contract_json(
    schema_path: &Path,
    grammar_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    std::fs::write(schema_path, local_model_response_json_schema())?;
    std::fs::write(grammar_path, local_model_response_gbnf_grammar())?;

    Ok(serde_json::json!({
        "schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "schema_path": schema_path.display().to_string(),
        "grammar_path": grammar_path.display().to_string(),
    }))
}

#[cfg(feature = "local-model")]
fn benchmark_local_model_json(
    candidate_id: &str,
    executable: &Path,
    model_path: &Path,
    arguments: &[String],
    baseline_path: &Path,
    compare_baseline: bool,
    fail_on_regression: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let candidate = local_model_candidate(candidate_id)?;
    let mut config = LocalExecutableRunnerConfig::new(executable.to_path_buf())
        .with_model_path(model_path.to_path_buf());
    for argument in arguments {
        config = config.with_argument(argument);
    }
    let benchmark = LocalModelBenchmark::new(
        candidate,
        LocalExecutableRunner::new(config),
        default_steward_evaluation_suite(),
    );
    let mut store = FileLocalModelBenchmarkBaselineStore::open(baseline_path)?;
    let identity = StewardIdentity::new("continuitydb-cli-local-model", "0.1.0", "strict")?;
    let report = record_local_model_benchmark_baseline_with_regression(
        &benchmark,
        identity,
        Utc::now(),
        &mut store,
    )?;

    if fail_on_regression && report.regressed() {
        return Err(std::io::Error::other("local model benchmark regression detected").into());
    }

    Ok(local_model_benchmark_json(
        baseline_path,
        compare_baseline,
        report.current_baseline(),
        report.regression(),
    ))
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
) -> serde_json::Value {
    let total_cases = baseline.evaluation().case_reports().len();
    let passed_cases = baseline
        .evaluation()
        .case_reports()
        .iter()
        .filter(|case| case.passed())
        .count();
    serde_json::json!({
        "candidate_model_id": baseline.candidate_model_id(),
        "candidate_role": baseline.candidate_role(),
        "baseline_path": baseline_path.display().to_string(),
        "recorded_at": baseline.recorded_at(),
        "passed": baseline.passed(),
        "passed_cases": passed_cases,
        "total_cases": total_cases,
        "runtime": {
            "executable": baseline.runtime().executable(),
            "arguments": baseline.runtime().arguments(),
        },
        "baseline_comparison": regression.map(|regression| {
            serde_json::json!({
                "compared": true,
                "regressed": regression.regressed(),
                "previous_recorded_at": regression.previous_recorded_at(),
                "current_recorded_at": regression.current_recorded_at(),
                "previous_passed_cases": regression.previous_passed_cases(),
                "current_passed_cases": regression.current_passed_cases(),
                "pass_count_delta": regression.pass_count_delta(),
            })
        }).or_else(|| compared.then(|| serde_json::json!({
            "compared": true,
            "regressed": false,
            "previous_recorded_at": null,
        }))),
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

    let measurement = match options.kernel {
        WorkloadKernelProfile::Memory => {
            let mut memory = MemoryKernel::default();
            measure_ingest_and_checkout(&mut memory, &workload, committed_at, request)?
        }
        WorkloadKernelProfile::File => {
            let path = options
                .store_path
                .ok_or_else(|| std::io::Error::other("store path is required"))?;
            let mut file = continuitydb_kernel::FileKernel::open(path)?;
            measure_ingest_and_checkout(&mut file, &workload, committed_at, request)?
        }
    };

    let snapshot = WorkloadMeasurementSnapshot::from_measurement(&measurement);
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
            &measurement,
        )?;
    }

    Ok(workload_measurement_json(
        options.kernel,
        options.store_path,
        options.baseline_path,
        options.label,
        comparison.as_ref(),
        measurement,
    ))
}

fn workload_measurement_json(
    kernel: WorkloadKernelProfile,
    store_path: Option<&PathBuf>,
    baseline_path: Option<&PathBuf>,
    label: &str,
    comparison: Option<&WorkloadBaselineComparison>,
    measurement: WorkloadMeasurement,
) -> serde_json::Value {
    serde_json::json!({
        "kernel": workload_kernel_name(kernel),
        "store_path": store_path.map(|path| path.display().to_string()),
        "baseline_path": baseline_path.map(|path| path.display().to_string()),
        "baseline_label": baseline_path.map(|_| label),
        "baseline_comparison": comparison.map(workload_baseline_comparison_json),
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
    measurement: &WorkloadMeasurement,
) -> Result<(), Box<dyn std::error::Error>> {
    let recorded_at = Utc::now();
    let record = WorkloadBaselineRecord::new(
        recorded_at,
        label,
        kernel,
        WorkloadMeasurementSnapshot::from_measurement(measurement),
    );
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
