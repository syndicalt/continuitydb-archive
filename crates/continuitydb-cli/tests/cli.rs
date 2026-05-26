//! CLI smoke tests.
#![recursion_limit = "512"]

use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use continuitydb_api::ContinuityDb;
use continuitydb_core::{
    Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload, Citation, CommitId,
    Confidence, ContextAbstractionLevel, ContextCompilerPolicy, ContextCompilerProposal,
    ContextPacketStrategy, Evidence, MemoryProjection, MemoryProjectionKind, RevisionLinkKind,
    Scope, SemanticAnchor, SourceId, StateCell, StateCellId, SystemTimeRange, TrajectoryMemory,
    TrustSignal, ValidTimeRange,
};
use continuitydb_kernel::{CommitManifestLookup, FileKernel, RevisionLinkLookup};
use continuitydb_query::{
    encode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelope, QueryOptimization,
    QueryRequirements, QueryReturnShape, QueryTask, QUERY_ENVELOPE_FORMAT,
    QUERY_ENVELOPE_FORMAT_VERSION,
};
use predicates::str::contains;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[test]
fn cli_help_does_not_expose_old_benchmark_commands() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output)?;

    assert!(
        help.contains("agent-behavior-benchmark"),
        "current downstream behavior benchmark is missing from help"
    );
    assert!(
        help.contains("agent-behavior-task-matrix"),
        "agent behavior task matrix is missing from help"
    );
    assert!(
        help.contains("agent-behavior-benchmark-bundle"),
        "agent behavior benchmark bundle is missing from help"
    );
    assert!(
        help.contains("agent-behavior-benchmark-report"),
        "agent behavior benchmark report is missing from help"
    );
    assert!(
        help.contains("score-agent-behavior-outputs"),
        "retained model-output scorer is missing from help"
    );
    assert!(
        help.contains("run-agent-behavior-outputs"),
        "executable retained-output runner is missing from help"
    );
    assert!(
        help.contains("thesis-falsification-benchmark"),
        "thesis falsification benchmark is missing from help"
    );

    for hidden_command in [
        "context-collapse-benchmark",
        "context-collapse-retrieval-benchmark",
        "comprehensive-vector-benchmark",
        "comprehensive-graph-benchmark",
        "live-benchmark-corpus",
        "live-benchmark-run",
        "live-benchmark-smoke-report",
        "live-benchmark-quality-report",
        "representative-benchmark-report",
        "adversarial-validation-report",
        "adversarial-task-harness-report",
    ] {
        assert!(
            !help.contains(hidden_command),
            "old benchmark command leaked into help: {hidden_command}"
        );
    }

    Ok(())
}

#[test]
fn cli_thesis_falsification_benchmark_writes_decisive_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-thesis-falsification-report");
    let summary_path = temp_store_path("continuitydb-cli-thesis-falsification-summary");

    let output = Command::cargo_bin("continuitydb")?
        .arg("thesis-falsification-benchmark")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let summary = fs::read_to_string(summary_path)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.thesis_falsification_benchmark")
    );
    assert_eq!(
        report["judgement"]["verdict"].as_str(),
        Some("original_db_primitive_collapses_control_layer_survives")
    );
    assert_eq!(
        report["judgement"]["original_database_primitive_thesis_survives"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["judgement"]["durable_control_layer_thesis_survives"].as_bool(),
        Some(true)
    );
    assert!(summary.contains("Final Judgement"));
    assert!(summary.contains("Original database-primitive thesis survives: `false`"));
    Ok(())
}

#[test]
fn cli_agent_behavior_benchmark_reports_downstream_metrics(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-benchmark-report");

    let output = Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.agent_behavior_benchmark")
    );
    assert_eq!(report["task_count"].as_u64(), Some(10));
    assert_eq!(report["turns_per_task"].as_u64(), Some(5));
    let checkout = report["strategies"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing strategy array"))?
        .iter()
        .find(|strategy| strategy["strategy"].as_str() == Some("continuitydb_checkout"))
        .ok_or_else(|| std::io::Error::other("missing checkout strategy"))?;
    assert_eq!(
        checkout["metrics"]["stale_belief_rate_bps"].as_u64(),
        Some(0)
    );
    assert_eq!(
        checkout["metrics"]["action_regression_rate_bps"].as_u64(),
        Some(0)
    );
    assert!(report["next_expansion_gates"]
        .as_array()
        .is_some_and(|gates| gates.len() >= 4));
    Ok(())
}

#[test]
fn cli_agent_behavior_task_matrix_writes_runner_tasks() -> Result<(), Box<dyn std::error::Error>> {
    let tasks_path = temp_store_path("continuitydb-cli-agent-behavior-task-matrix-tasks");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-task-matrix-report");

    let output = Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-task-matrix")
        .arg("--tasks-path")
        .arg(&tasks_path)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let tasks: Value = serde_json::from_str(&fs::read_to_string(tasks_path)?)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.agent_behavior_task_matrix")
    );
    assert_eq!(report["scenario_count"].as_u64(), Some(10));
    assert_eq!(report["strategy_count"].as_u64(), Some(4));
    assert_eq!(tasks.as_array().map(Vec::len), Some(40));
    assert!(tasks
        .as_array()
        .is_some_and(|tasks| tasks.iter().any(|task| {
            task["task_id"].as_str() == Some("release-upload-0")
                && task["strategy"].as_str() == Some("continuitydb_checkout")
                && task["context_packet"]
                    .as_str()
                    .is_some_and(|context| context.contains("Superseding correction"))
        })));
    Ok(())
}

#[test]
fn cli_agent_behavior_task_matrix_can_emit_representative_repo_tasks(
) -> Result<(), Box<dyn std::error::Error>> {
    let tasks_path = temp_store_path("continuitydb-cli-representative-agent-behavior-tasks");
    let report_path = temp_store_path("continuitydb-cli-representative-agent-behavior-report");

    let output = Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-task-matrix")
        .arg("--representative-corpus")
        .arg("--tasks-path")
        .arg(&tasks_path)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let tasks: Value = serde_json::from_str(&fs::read_to_string(tasks_path)?)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.representative_agent_behavior_task_matrix")
    );
    assert_eq!(report["scenario_count"].as_u64(), Some(8));
    assert_eq!(report["strategy_count"].as_u64(), Some(4));
    assert_eq!(tasks.as_array().map(Vec::len), Some(32));
    assert!(tasks
        .as_array()
        .is_some_and(|tasks| tasks.iter().any(|task| {
            task["task_id"].as_str() == Some("repo-ci-flake-triage")
                && task["strategy"].as_str() == Some("continuitydb_checkout")
                && task["context_packet"]
                    .as_str()
                    .is_some_and(|context| context.contains("Evidence locator"))
        })));
    Ok(())
}

#[test]
fn cli_agent_behavior_benchmark_bundle_writes_reproducible_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-agent-behavior-benchmark-bundle").with_extension("dir");
    let adapter_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("scripts/agent_behavior_model_runner.mjs")
        .canonicalize()?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--runner")
        .arg("node")
        .arg(format!("--runner-arg={}", adapter_path.display()))
        .arg("--runner-arg=--dry-run")
        .arg("--trials")
        .arg("2")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;

    let manifest_path = artifact_dir.join("agent-behavior-benchmark.manifest.json");
    let matrix_path = artifact_dir.join("agent-behavior-task-matrix.json");
    let tasks_path = artifact_dir.join("agent-behavior-tasks.json");
    let records_path = artifact_dir.join("agent-behavior-output-records.json");
    let report_path = artifact_dir.join("agent-behavior-execution-benchmark.json");

    assert_eq!(
        stdout["format"].as_str(),
        Some("continuitydb.agent_behavior_benchmark_bundle")
    );
    assert_eq!(
        stdout["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    for path in [
        &manifest_path,
        &matrix_path,
        &tasks_path,
        &records_path,
        &report_path,
    ] {
        assert!(path.exists(), "missing artifact: {}", path.display());
    }

    let manifest: Value = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let records: Value = serde_json::from_str(&fs::read_to_string(records_path)?)?;

    assert_eq!(
        manifest["format"].as_str(),
        Some("continuitydb.agent_behavior_benchmark_bundle")
    );
    assert_eq!(manifest["trial_count"].as_u64(), Some(2));
    assert_eq!(manifest["task_count"].as_u64(), Some(40));
    assert_eq!(manifest["execution_record_count"].as_u64(), Some(80));
    assert_eq!(records.as_array().map(Vec::len), Some(80));
    assert_eq!(report["execution_record_count"].as_u64(), Some(80));
    assert!(records.as_array().is_some_and(|records| records
        .iter()
        .all(|record| record["model_latency_ms"].as_u64().is_some())));
    assert!(report["strategy_latency"]
        .as_array()
        .is_some_and(|latencies| latencies.len() == 4
            && latencies.iter().all(|latency| latency["sample_count"]
                .as_u64()
                .is_some_and(|count| count > 0)
                && latency["p50_ms"].as_u64().is_some()
                && latency["p95_ms"].as_u64().is_some()
                && latency["p99_ms"].as_u64().is_some())));
    assert!(report["strategies"]
        .as_array()
        .is_some_and(|strategies| strategies.iter().all(|strategy| strategy
            ["task_success_confidence_interval_bps"]["lower_bps"]
            .as_u64()
            .is_some()
            && strategy["task_success_confidence_interval_bps"]["upper_bps"]
                .as_u64()
                .is_some())));
    assert!(manifest["artifacts"]["task_matrix"]["fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(manifest["artifacts"]["execution_report"]["bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    Ok(())
}

#[test]
fn cli_agent_behavior_benchmark_report_classifies_retained_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-agent-behavior-report-bundle").with_extension("dir");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-report");
    let summary_path = temp_store_path("continuitydb-cli-agent-behavior-summary");
    let adapter_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("scripts/agent_behavior_model_runner.mjs")
        .canonicalize()?;

    Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark-bundle")
        .arg("--representative-corpus")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--runner")
        .arg("node")
        .arg(format!("--runner-arg={}", adapter_path.display()))
        .arg("--runner-arg=--dry-run")
        .arg("--trials")
        .arg("2")
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark-report")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let summary = fs::read_to_string(summary_path)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.agent_behavior_benchmark_report")
    );
    assert_eq!(
        report["bundle"]["corpus"].as_str(),
        Some("representative_repo_lifecycle")
    );
    assert_eq!(
        report["evidence_tier"].as_str(),
        Some("representative_dry_run_artifact_validation")
    );
    assert_eq!(report["marketable"].as_bool(), Some(false));
    assert_eq!(
        report["marketing_blockers"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        report["metrics"]["best_strategy"].as_str(),
        Some("continuitydb_checkout")
    );
    assert!(report["metrics"]["checkout_task_success_rate_bps"]
        .as_u64()
        .is_some());
    assert!(report["metrics"]["checkout_latency_p95_ms"]
        .as_u64()
        .is_some());
    assert!(summary.contains("representative_dry_run_artifact_validation"));
    assert!(summary.contains("Not marketable"));
    Ok(())
}

#[test]
fn cli_agent_behavior_benchmark_report_require_marketable_rejects_dry_run(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-agent-behavior-require-marketable").with_extension("dir");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-require-marketable-report");
    let adapter_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("scripts/agent_behavior_model_runner.mjs")
        .canonicalize()?;

    Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark-bundle")
        .arg("--representative-corpus")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--runner")
        .arg("node")
        .arg(format!("--runner-arg={}", adapter_path.display()))
        .arg("--runner-arg=--dry-run")
        .arg("--trials")
        .arg("2")
        .assert()
        .success();

    Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark-report")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--require-marketable")
        .assert()
        .failure()
        .stderr(contains(
            "agent behavior benchmark evidence is not marketable",
        ));

    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    assert_eq!(report["marketable"].as_bool(), Some(false));
    assert!(report["marketing_blockers"]
        .as_array()
        .is_some_and(|blockers| blockers.iter().any(|blocker| blocker.as_str()
            == Some("replace dry-run adapter with retained live model outputs"))));
    Ok(())
}

#[test]
fn cli_agent_behavior_benchmark_report_require_marketable_rejects_weak_checkout_signal(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-agent-behavior-weak-signal").with_extension("dir");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-weak-signal-report");
    fs::create_dir_all(&artifact_dir)?;
    fs::write(
        artifact_dir.join("agent-behavior-benchmark.manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "format": "continuitydb.agent_behavior_benchmark_bundle",
            "format_version": 1,
            "artifact_dir": artifact_dir.display().to_string(),
            "runner": "node",
            "runner_args": ["scripts/agent_behavior_model_runner.mjs", "--model", "deepseek-v4-flash:cloud"],
            "corpus": "representative_repo_lifecycle",
            "trial_count": 3,
            "scenario_count": 8,
            "strategy_count": 4,
            "task_count": 32,
            "execution_record_count": 96
        }))?,
    )?;
    fs::write(
        artifact_dir.join("agent-behavior-execution-benchmark.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "format": "continuitydb.agent_behavior_execution_benchmark",
            "format_version": 1,
            "evidence_mode": "retained_model_output_downstream_behavior",
            "task_count": 8,
            "execution_record_count": 96,
            "trial_count": 3,
            "strategies": [
                weak_signal_strategy("transcript_summary", 9000, 0, 0),
                weak_signal_strategy("vector_retrieval", 8000, 1000, 0),
                weak_signal_strategy("raw_statecell", 7000, 2000, 1000),
                weak_signal_strategy("continuitydb_checkout", 5000, 3000, 2000)
            ],
            "strategy_stability": [],
            "strategy_latency": [
                weak_signal_latency("transcript_summary"),
                weak_signal_latency("vector_retrieval"),
                weak_signal_latency("raw_statecell"),
                weak_signal_latency("continuitydb_checkout")
            ],
            "execution_records": [],
            "next_expansion_gates": []
        }))?,
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("agent-behavior-benchmark-report")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--require-marketable")
        .assert()
        .failure()
        .stderr(contains(
            "agent behavior benchmark evidence is not marketable",
        ));

    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    assert_eq!(report["marketable"].as_bool(), Some(false));
    assert_eq!(
        report["gates"]["checkout_best_or_tied_task_success"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["gates"]["checkout_lowest_or_tied_stale_belief"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["gates"]["checkout_lowest_or_tied_action_regression"].as_bool(),
        Some(false)
    );
    assert!(report["marketing_blockers"]
        .as_array()
        .is_some_and(|blockers| blockers.iter().any(
            |blocker| blocker.as_str() == Some("checkout must be best or tied on task success")
        )));
    Ok(())
}

#[test]
fn cli_score_agent_behavior_outputs_scores_retained_model_records(
) -> Result<(), Box<dyn std::error::Error>> {
    let records_path = temp_store_path("continuitydb-cli-agent-behavior-output-records");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-output-report");
    fs::write(
        &records_path,
        serde_json::to_string_pretty(&serde_json::json!([
            {
                "task_id": "release-upload",
                "strategy": "transcript_summary",
                "prompt": "The GitHub release upload failed with HTTP 404. What next?",
                "context_packet": "Older summary says the package upload succeeded.",
                "model_output": "The upload succeeded. Retry the same shopt -s globstar upload command; it is definitely fine.",
                "requirements": {
                    "requires_revision": true,
                    "requires_uncertainty": false,
                    "requires_verification": true,
                    "has_stale_trap": true,
                    "has_known_failed_action": true
                }
            },
            {
                "task_id": "release-upload",
                "strategy": "continuitydb_checkout",
                "prompt": "The GitHub release upload failed with HTTP 404. What next?",
                "context_packet": "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.",
                "model_output": "Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale.",
                "requirements": {
                    "requires_revision": true,
                    "requires_uncertainty": false,
                    "requires_verification": true,
                    "has_stale_trap": true,
                    "has_known_failed_action": true
                }
            }
        ]))?,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("score-agent-behavior-outputs")
        .arg("--records-path")
        .arg(&records_path)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.agent_behavior_execution_benchmark")
    );
    assert_eq!(
        report["evidence_mode"].as_str(),
        Some("retained_model_output_downstream_behavior")
    );
    assert_eq!(report["task_count"].as_u64(), Some(1));
    assert_eq!(report["execution_record_count"].as_u64(), Some(2));
    assert_eq!(
        report["execution_records"][0]["model_output"].as_str(),
        Some("The upload succeeded. Retry the same shopt -s globstar upload command; it is definitely fine.")
    );
    let checkout = report["strategies"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing strategy array"))?
        .iter()
        .find(|strategy| strategy["strategy"].as_str() == Some("continuitydb_checkout"))
        .ok_or_else(|| std::io::Error::other("missing checkout strategy"))?;
    assert_eq!(
        checkout["metrics"]["task_success_rate_bps"].as_u64(),
        Some(10_000)
    );
    assert_eq!(
        checkout["metrics"]["action_regression_rate_bps"].as_u64(),
        Some(0)
    );
    Ok(())
}

fn weak_signal_strategy(
    strategy: &str,
    task_success_rate_bps: u64,
    stale_belief_rate_bps: u64,
    action_regression_rate_bps: u64,
) -> Value {
    serde_json::json!({
        "strategy": strategy,
        "role": format!("retained model output strategy: {strategy}"),
        "passed_task_count": task_success_rate_bps / 1000,
        "total_task_count": 10,
        "metrics": {
            "task_success_rate_bps": task_success_rate_bps,
            "stale_belief_rate_bps": stale_belief_rate_bps,
            "revision_accuracy_bps": 8000,
            "unsupported_certainty_rate_bps": 0,
            "verification_rate_bps": 8000,
            "context_budget_fit_bps": 10000,
            "action_regression_rate_bps": action_regression_rate_bps
        },
        "task_success_confidence_interval_bps": {
            "lower_bps": task_success_rate_bps.saturating_sub(1000),
            "upper_bps": (task_success_rate_bps + 1000).min(10000)
        }
    })
}

fn weak_signal_latency(strategy: &str) -> Value {
    serde_json::json!({
        "strategy": strategy,
        "sample_count": 24,
        "p50_ms": 100,
        "p95_ms": 150,
        "p99_ms": 175
    })
}

#[cfg(unix)]
#[test]
fn cli_run_agent_behavior_outputs_executes_runner_and_scores_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let tasks_path = temp_store_path("continuitydb-cli-agent-behavior-runner-tasks");
    let records_path = temp_store_path("continuitydb-cli-agent-behavior-runner-records");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-runner-report");
    let executable_path = temp_store_path("continuitydb-cli-agent-behavior-runner");
    fs::write(
        &tasks_path,
        serde_json::to_string_pretty(&serde_json::json!([
            {
                "task_id": "release-upload",
                "strategy": "continuitydb_checkout",
                "prompt": "The GitHub release upload failed with HTTP 404. What next?",
                "context_packet": "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.",
                "requirements": {
                    "requires_revision": true,
                    "requires_uncertainty": false,
                    "requires_verification": true,
                    "has_stale_trap": true,
                    "has_known_failed_action": true
                }
            }
        ]))?,
    )?;
    fs::write(
        &executable_path,
        r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"model_output":"Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale."}'
"#,
    )?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("run-agent-behavior-outputs")
        .arg("--tasks-path")
        .arg(&tasks_path)
        .arg("--runner")
        .arg(&executable_path)
        .arg("--records-path")
        .arg(&records_path)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout: Value = serde_json::from_slice(&output)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report_path)?)?;
    let records: Value = serde_json::from_str(&fs::read_to_string(records_path)?)?;

    assert_eq!(stdout, report);
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.agent_behavior_execution_benchmark")
    );
    assert_eq!(report["execution_record_count"].as_u64(), Some(1));
    assert_eq!(
        records[0]["model_output"].as_str(),
        Some("Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale.")
    );
    assert_eq!(
        report["execution_records"][0]["outcome"]["task_success"].as_bool(),
        Some(true)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn cli_run_agent_behavior_outputs_names_task_when_runner_returns_empty_answer(
) -> Result<(), Box<dyn std::error::Error>> {
    let tasks_path = temp_store_path("continuitydb-cli-agent-behavior-empty-output-tasks");
    let executable_path = temp_store_path("continuitydb-cli-agent-behavior-empty-output-runner");
    fs::write(
        &tasks_path,
        serde_json::to_string_pretty(&serde_json::json!([
            {
                "task_id": "release-upload",
                "strategy": "continuitydb_checkout",
                "prompt": "The GitHub release upload failed with HTTP 404. What next?",
                "context_packet": "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.",
                "requirements": {
                    "requires_revision": true,
                    "requires_uncertainty": false,
                    "requires_verification": true,
                    "has_stale_trap": true,
                    "has_known_failed_action": true
                }
            }
        ]))?,
    )?;
    fs::write(
        &executable_path,
        r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"model_output":""}'
"#,
    )?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("run-agent-behavior-outputs")
        .arg("--tasks-path")
        .arg(&tasks_path)
        .arg("--runner")
        .arg(&executable_path)
        .assert()
        .failure()
        .stderr(contains(
            "agent behavior runner output missing model_output for task release-upload strategy continuitydb_checkout trial 0",
        ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn cli_run_agent_behavior_outputs_repeats_trials_and_reports_stability(
) -> Result<(), Box<dyn std::error::Error>> {
    let tasks_path = temp_store_path("continuitydb-cli-agent-behavior-trials-tasks");
    let records_path = temp_store_path("continuitydb-cli-agent-behavior-trials-records");
    let report_path = temp_store_path("continuitydb-cli-agent-behavior-trials-report");
    let executable_path = temp_store_path("continuitydb-cli-agent-behavior-trials-runner");
    fs::write(
        &tasks_path,
        serde_json::to_string_pretty(&serde_json::json!([
            {
                "task_id": "release-upload",
                "strategy": "continuitydb_checkout",
                "prompt": "The GitHub release upload failed with HTTP 404. What next?",
                "context_packet": "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.",
                "requirements": {
                    "requires_revision": true,
                    "requires_uncertainty": false,
                    "requires_verification": true,
                    "has_stale_trap": true,
                    "has_known_failed_action": true
                }
            }
        ]))?,
    )?;
    fs::write(
        &executable_path,
        r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"model_output":"Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale."}'
"#,
    )?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("run-agent-behavior-outputs")
        .arg("--tasks-path")
        .arg(&tasks_path)
        .arg("--runner")
        .arg(&executable_path)
        .arg("--trials")
        .arg("3")
        .arg("--records-path")
        .arg(&records_path)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output)?;
    let records: Value = serde_json::from_str(&fs::read_to_string(records_path)?)?;

    assert_eq!(records.as_array().map(Vec::len), Some(3));
    assert_eq!(records[0]["trial_index"].as_u64(), Some(0));
    assert_eq!(records[1]["trial_index"].as_u64(), Some(1));
    assert_eq!(records[2]["trial_index"].as_u64(), Some(2));
    assert_eq!(report["trial_count"].as_u64(), Some(3));
    assert_eq!(
        report["strategy_stability"][0]["strategy"].as_str(),
        Some("continuitydb_checkout")
    );
    assert_eq!(
        report["strategy_stability"][0]["trial_count"].as_u64(),
        Some(3)
    );
    assert_eq!(
        report["strategy_stability"][0]["task_success_mean_bps"].as_u64(),
        Some(10_000)
    );
    assert_eq!(
        report["strategy_stability"][0]["task_success_min_bps"].as_u64(),
        Some(10_000)
    );
    assert_eq!(
        report["strategy_stability"][0]["task_success_max_bps"].as_u64(),
        Some(10_000)
    );
    Ok(())
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn test_fnv1a64_fingerprint(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn refresh_workload_manifest_report_metadata(
    artifact_dir: &std::path::Path,
    report: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut canonical_report = report.clone();
    canonical_report["bundle_manifest"] = Value::Null;
    let canonical_report_text = serde_json::to_string_pretty(&canonical_report)?;

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_report_bytes"] = Value::from(canonical_report_text.len());
    manifest["workload_report_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&canonical_report_text));
    manifest["revision_link_count"] = report["revision_link_count"].clone();
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

#[cfg(feature = "local-model")]
fn write_dry_run_local_model_bundle(
    artifact_dir: &std::path::Path,
    baseline_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--artifact-dir")
        .arg(artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(baseline_path)
        .assert()
        .success();
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
fn write_changed_case_local_model_bundle(
    artifact_dir: &std::path::Path,
    baseline_path: &std::path::Path,
    executable_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_script = passing_local_model_runner_script().replace(
        "The evidence is thin, so uncertainty remains.",
        "Uncertainty remains because the evidence is thin.",
    );
    fs::write(executable_path, passing_local_model_runner_script())?;
    let mut permissions = fs::metadata(executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(baseline_path)
        .assert()
        .success();
    fs::write(executable_path, current_script)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--compare-baseline")
        .arg("--artifact-dir")
        .arg(artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(baseline_path)
        .assert()
        .success();
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
fn write_real_local_model_bundle(
    artifact_dir: &std::path::Path,
    baseline_path: &std::path::Path,
    executable_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(executable_path, passing_local_model_runner_script())?;
    let mut permissions = fs::metadata(executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--artifact-dir")
        .arg(artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(baseline_path)
        .assert()
        .success();
    Ok(())
}

#[cfg(feature = "local-model")]
fn refresh_local_model_changed_case_report_metadata(
    artifact_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    let changed_case_report_text = fs::read_to_string(&changed_case_report_path)?;
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut bundle_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    bundle_manifest["changed_case_report"]["report_bytes"] =
        Value::from(changed_case_report_text.len());
    bundle_manifest["changed_case_report"]["report_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&changed_case_report_text));
    fs::write(
        &bundle_manifest_path,
        serde_json::to_string_pretty(&bundle_manifest)?,
    )?;
    Ok(())
}

#[cfg(feature = "local-model")]
fn refresh_local_model_response_manifest_metadata(
    artifact_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let response_manifest_text = fs::read_to_string(&response_manifest_path)?;
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut bundle_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    bundle_manifest["response_artifact_manifest"]["manifest_bytes"] =
        Value::from(response_manifest_text.len());
    bundle_manifest["response_artifact_manifest"]["manifest_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&response_manifest_text));
    fs::write(
        &bundle_manifest_path,
        serde_json::to_string_pretty(&bundle_manifest)?,
    )?;
    Ok(())
}

#[cfg(feature = "local-model")]
fn refresh_local_model_benchmark_report_metadata(
    artifact_dir: &std::path::Path,
    report: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut canonical_report = report.clone();
    canonical_report["bundle_manifest"] = Value::Null;
    let canonical_report_text = serde_json::to_string_pretty(&canonical_report)?;

    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut bundle_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    bundle_manifest["benchmark_report_bytes"] = Value::from(canonical_report_text.len());
    bundle_manifest["benchmark_report_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&canonical_report_text));
    fs::write(
        &bundle_manifest_path,
        serde_json::to_string_pretty(&bundle_manifest)?,
    )?;
    Ok(())
}

#[test]
fn cli_reports_version() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::cargo_bin("continuitydb")?;
    command.arg("--version");
    command.assert().success().stdout(contains("continuitydb"));
    Ok(())
}

#[test]
fn cli_demo_checkout_outputs_metadata_json() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::cargo_bin("continuitydb")?;
    let output = command
        .arg("demo-checkout")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(json["summary"]["selected_cell_count"].as_u64(), Some(1));
    assert_eq!(json["summary"]["alternative_count"].as_u64(), Some(1));
    assert_eq!(json["summary"]["total_tokens"].as_u64(), Some(10));
    assert_eq!(json["summary"]["token_budget"].as_u64(), Some(10));
    assert_eq!(json["summary"]["citation_count"].as_u64(), Some(1));
    assert_eq!(json["summary"]["uncertainty_count"].as_u64(), Some(1));
    assert_eq!(
        json["summary"]["frontier_recommendation_count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        json["summary"]["bounded_by_token_budget"].as_bool(),
        Some(true)
    );
    assert_eq!(json["audit_traces"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["audit_traces"][0]["evidence"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        json["audit_traces"][0]["dependencies"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(json["uncertainty"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["frontier_recommendations"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(json["alternatives"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["alternatives"][0]["reason"].as_str(),
        Some("TokenBudgetExceeded")
    );
    Ok(())
}

#[test]
fn cli_context_collapse_drill_outputs_prevention_report() -> Result<(), Box<dyn std::error::Error>>
{
    let report_path = temp_store_path("continuitydb-cli-context-collapse-drill-report");
    let output = Command::cargo_bin("continuitydb")?
        .arg("context-collapse-drill")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.context_collapse_drill")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["scenario"]["cell_count"].as_u64(), Some(3));
    assert_eq!(json["scenario"]["revision_link_count"].as_u64(), Some(2));
    assert_eq!(json["checkout"]["selected_count"].as_u64(), Some(2));
    assert_eq!(json["checkout"]["alternative_count"].as_u64(), Some(1));
    assert_eq!(json["proof"]["bounded_context"].as_bool(), Some(true));
    assert_eq!(json["proof"]["citations_preserved"].as_bool(), Some(true));
    assert_eq!(json["proof"]["uncertainty_preserved"].as_bool(), Some(true));
    assert_eq!(
        json["proof"]["revision_links_preserved"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["proof"]["revision_context_preserved"].as_bool(),
        Some(true)
    );
    assert_eq!(json["proof"]["frontier_preserved"].as_bool(), Some(true));
    assert_eq!(json["proof"]["summary_preserved"].as_bool(), Some(true));
    assert_eq!(
        json["checkout"]["summary"]["selected_cell_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        json["checkout"]["summary"]["alternative_count"].as_u64(),
        Some(1)
    );
    assert!(json["checkout"]["summary"]["revision_context_count"]
        .as_u64()
        .is_some_and(|count| count >= 2));
    assert_eq!(
        json["context_collapse_prevention"]["lost_revision_history"].as_bool(),
        Some(false)
    );
    assert_eq!(
        json["context_collapse_prevention"]["lost_revision_evidence"].as_bool(),
        Some(false)
    );
    assert_eq!(
        json["context_collapse_prevention"]["lost_structured_summary"].as_bool(),
        Some(false)
    );
    assert!(json["checkout"]["revision_link_kinds"]
        .as_array()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("supersedes"))));

    let report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    assert_eq!(report, json);

    fs::remove_file(report_path)?;
    Ok(())
}

#[test]
fn cli_context_collapse_benchmark_compares_checkout_to_collapsed_summary(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-context-collapse-benchmark-report");
    let output = Command::cargo_bin("continuitydb")?
        .arg("context-collapse-benchmark")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.context_collapse_benchmark")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("context-collapse-benchmark")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["scenario"]["cell_count"].as_u64(), Some(3));
    assert_eq!(json["scenario"]["revision_link_count"].as_u64(), Some(2));
    assert_eq!(json["strategies"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        json["strategies"][0]["strategy"].as_str(),
        Some("continuity_checkout")
    );
    assert_eq!(
        json["strategies"][1]["strategy"].as_str(),
        Some("collapsed_summary")
    );
    assert_eq!(
        json["strategies"][0]["metrics"]["continuity_score"].as_u64(),
        Some(6)
    );
    assert_eq!(
        json["strategies"][1]["metrics"]["continuity_score"].as_u64(),
        Some(1)
    );
    assert_eq!(json["comparison"]["score_delta"].as_i64(), Some(5));
    assert_eq!(
        json["comparison"]["continuity_checkout_wins"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["comparison"]["collapsed_summary_losses"]
            .as_array()
            .map(Vec::len),
        Some(5)
    );
    assert!(json["comparison"]["collapsed_summary_losses"]
        .as_array()
        .is_some_and(|losses| losses
            .iter()
            .any(|loss| loss.as_str() == Some("lost_revision_history"))));
    assert_eq!(
        json["strategies"][0]["metrics"]["bounded_by_token_budget"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["strategies"][1]["metrics"]["bounded_by_token_budget"].as_bool(),
        Some(true)
    );

    let report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    assert_eq!(report, json);

    fs::remove_file(report_path)?;
    Ok(())
}

#[test]
fn cli_context_collapse_retrieval_benchmark_targets_pinecone_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-context-collapse-retrieval-benchmark");
    let output = Command::cargo_bin("continuitydb")?
        .arg("context-collapse-retrieval-benchmark")
        .arg("--report-path")
        .arg(&report_path)
        .env_remove("PINECONE_API_KEY")
        .env_remove("PINECONE_INDEX_HOST")
        .env_remove("PINECONE_INDEX_NAME")
        .env_remove("PINECONE_NAMESPACE")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.context_collapse_retrieval_benchmark")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("context-collapse-retrieval-benchmark")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["scenario"]["cell_count"].as_u64(), Some(3));
    assert_eq!(json["query_set"]["query_count"].as_u64(), Some(1));
    assert!(json["corpus"]["fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("sha256:")));
    assert_eq!(json["vector_target"]["provider"].as_str(), Some("pinecone"));
    assert_eq!(
        json["vector_target"]["mode"].as_str(),
        Some("offline-vector-baseline")
    );
    assert_eq!(
        json["vector_target"]["live_config_available"].as_bool(),
        Some(false)
    );
    assert_eq!(
        json["vector_target"]["missing_env"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
    assert!(json["vector_target"]["missing_env"]
        .as_array()
        .is_some_and(|missing| missing
            .iter()
            .any(|name| name.as_str() == Some("PINECONE_INDEX_HOST"))));
    assert_eq!(json["strategies"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        json["strategies"][0]["strategy"].as_str(),
        Some("continuity_checkout")
    );
    assert_eq!(
        json["strategies"][1]["strategy"].as_str(),
        Some("pinecone_vector_top_k_chunks")
    );
    assert_eq!(
        json["strategies"][0]["metrics"]["gold_evidence_recall_bps"].as_u64(),
        Some(10000)
    );
    assert_eq!(
        json["strategies"][1]["metrics"]["gold_evidence_recall_bps"].as_u64(),
        Some(6667)
    );
    assert_eq!(
        json["strategies"][0]["metrics"]["revision_preservation_bps"].as_u64(),
        Some(10000)
    );
    assert_eq!(
        json["strategies"][1]["metrics"]["revision_preservation_bps"].as_u64(),
        Some(0)
    );
    assert_eq!(
        json["comparison"]["winner"].as_str(),
        Some("continuity_checkout")
    );
    assert_eq!(
        json["comparison"]["gold_evidence_recall_delta_bps"].as_i64(),
        Some(3333)
    );
    assert!(json["comparison"]["pinecone_target_losses"]
        .as_array()
        .is_some_and(|losses| losses
            .iter()
            .any(|loss| loss.as_str() == Some("lost_revision_history"))));

    let report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    assert_eq!(report, json);

    fs::remove_file(report_path)?;
    Ok(())
}

#[test]
fn cli_context_collapse_retrieval_benchmark_can_require_live_pinecone_config(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-context-collapse-retrieval-live-required");
    Command::cargo_bin("continuitydb")?
        .arg("context-collapse-retrieval-benchmark")
        .arg("--require-live-pinecone")
        .arg("--report-path")
        .arg(&report_path)
        .env_remove("PINECONE_API_KEY")
        .env_remove("PINECONE_INDEX_HOST")
        .env_remove("PINECONE_INDEX_NAME")
        .env_remove("PINECONE_NAMESPACE")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .failure()
        .stderr(contains("live Pinecone retrieval benchmark requires"));

    assert!(!report_path.exists());
    Ok(())
}

#[test]
fn cli_context_collapse_retrieval_benchmark_requires_live_vector_dimension(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-context-collapse-retrieval-live-dimension");
    Command::cargo_bin("continuitydb")?
        .arg("context-collapse-retrieval-benchmark")
        .arg("--require-live-pinecone")
        .arg("--report-path")
        .arg(&report_path)
        .env("PINECONE_API_KEY", "test-key")
        .env("PINECONE_INDEX_HOST", "test-index.svc.test.pinecone.io")
        .env("PINECONE_NAMESPACE", "continuitydb-test")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .failure()
        .stderr(contains("PINECONE_VECTOR_DIMENSION"));

    assert!(!report_path.exists());
    Ok(())
}

#[test]
fn cli_comprehensive_vector_benchmark_outputs_all_listed_families(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-comprehensive-vector-benchmark");
    let summary_path = temp_store_path("continuitydb-cli-comprehensive-vector-benchmark-summary");
    let output = Command::cargo_bin("continuitydb")?
        .arg("comprehensive-vector-benchmark")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .env_remove("NEO4J_URI")
        .env_remove("NEO4J_USERNAME")
        .env_remove("NEO4J_PASSWORD")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.comprehensive_vector_benchmark")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["coverage"]["family_count"].as_u64(), Some(10));
    assert_eq!(
        json["coverage"]["families"].as_array().map(Vec::len),
        Some(10)
    );
    assert!(json["coverage"]["families"]
        .as_array()
        .is_some_and(|families| families
            .iter()
            .any(|family| family.as_str() == Some("model_task_performance"))));
    assert_eq!(json["aggregate"]["winner"].as_str(), Some("continuitydb"));
    assert!(
        json["aggregate"]["score_delta_bps"]
            .as_i64()
            .unwrap_or_default()
            > 0
    );
    assert!(json["benchmarks"]
        .as_array()
        .is_some_and(
            |benchmarks| benchmarks
                .iter()
                .any(|benchmark| benchmark["evidence_mode"].as_str()
                    == Some("live_pinecone_vector_api"))
        ));
    let benchmarks = json["benchmarks"].as_array().ok_or("missing benchmarks")?;
    let model_task = benchmarks
        .iter()
        .find(|benchmark| benchmark["family"].as_str() == Some("model_task_performance"))
        .ok_or("missing model task benchmark")?;
    assert_eq!(
        model_task["evidence_mode"].as_str(),
        Some("executed_deterministic_task_model")
    );
    assert_eq!(
        model_task["measurement"]["continuitydb_answer"]["decision"].as_str(),
        Some("blocked")
    );
    assert_eq!(
        model_task["measurement"]["vector_answer"]["decision"].as_str(),
        Some("blocked")
    );
    assert_eq!(
        model_task["measurement"]["vector_answer"]["missing_citations"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    let scale_latency = benchmarks
        .iter()
        .find(|benchmark| benchmark["family"].as_str() == Some("scale_latency"))
        .ok_or("missing scale latency benchmark")?;
    assert_eq!(
        scale_latency["evidence_mode"].as_str(),
        Some("measured_in_process_latency")
    );
    assert_eq!(
        scale_latency["measurement"]["sizes"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    assert!(
        scale_latency["measurement"]["continuitydb_total_elapsed_ns"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        scale_latency["measurement"]["vector_total_elapsed_ns"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );

    let report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    assert_eq!(report, json);
    let summary = fs::read_to_string(&summary_path)?;
    assert!(summary.contains("# ContinuityDB vs Vector Benchmark Report"));
    assert!(summary.contains("| model_task_performance |"));
    assert!(summary.contains("executed_deterministic_task_model"));
    assert!(summary.contains("measured_in_process_latency"));
    assert!(summary.contains("## Benchmark Details"));
    assert!(summary.contains("### recall_under_context_pressure"));
    assert!(summary.contains("What it measures:"));
    assert!(summary.contains("Why it matters for agent memory/context:"));
    assert!(summary.contains("Agents fail when memory retrieval gives them plausible current facts but drops the historical evidence"));
    assert!(summary.contains("### human_auditable_evidence"));

    fs::remove_file(report_path)?;
    fs::remove_file(summary_path)?;
    Ok(())
}

#[test]
fn cli_comprehensive_graph_benchmark_outputs_neo4j_overlap_families(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-comprehensive-graph-benchmark");
    let summary_path = temp_store_path("continuitydb-cli-comprehensive-graph-benchmark-summary");
    let output = Command::cargo_bin("continuitydb")?
        .arg("comprehensive-graph-benchmark")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.comprehensive_graph_benchmark")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("comprehensive-graph-benchmark")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["coverage"]["family_count"].as_u64(), Some(10));
    assert_eq!(json["graph_target"]["provider"].as_str(), Some("neo4j"));
    assert_eq!(
        json["graph_target"]["live_config_available"].as_bool(),
        Some(false)
    );
    assert!(json["graph_target"]["capabilities"]
        .as_array()
        .is_some_and(|capabilities| capabilities
            .iter()
            .any(|capability| capability.as_str() == Some("vector_indexes"))));
    assert!(json["graph_target"]["capabilities"]
        .as_array()
        .is_some_and(|capabilities| capabilities
            .iter()
            .any(|capability| capability.as_str() == Some("gds_similarity_algorithms"))));
    assert_eq!(json["aggregate"]["winner"].as_str(), Some("continuitydb"));
    assert!(
        json["aggregate"]["score_delta_bps"]
            .as_i64()
            .unwrap_or_default()
            > 0
    );

    let benchmarks = json["benchmarks"].as_array().ok_or("missing benchmarks")?;
    assert_eq!(benchmarks.len(), 10);
    assert!(benchmarks.iter().any(|benchmark| {
        benchmark["family"].as_str() == Some("relationship_traversal")
            && benchmark["winner"].as_str() == Some("neo4j")
    }));
    assert!(benchmarks.iter().any(|benchmark| {
        benchmark["family"].as_str() == Some("bitemporal_statecell_semantics")
            && benchmark["continuitydb_score_bps"].as_u64() == Some(10_000)
            && benchmark["neo4j_score_bps"].as_u64().unwrap_or_default() < 7_000
    }));
    assert!(benchmarks.iter().any(|benchmark| {
        benchmark["family"].as_str() == Some("deterministic_token_budget_checkout")
            && benchmark["evidence_mode"].as_str() == Some("deterministic_token_budget_rubric")
    }));
    assert!(benchmarks.iter().all(|benchmark| {
        benchmark["measurement"]["neo4j_model"].as_str().is_some()
            && benchmark["measurement"]["continuitydb_model"]
                .as_str()
                .is_some()
    }));

    let report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    assert_eq!(report, json);
    let summary = fs::read_to_string(&summary_path)?;
    assert!(summary.contains("# ContinuityDB vs Neo4j Graph Benchmark Report"));
    assert!(summary.contains("| relationship_traversal |"));
    assert!(summary.contains("| bitemporal_statecell_semantics |"));
    assert!(summary.contains("Neo4j is the right graph-database foil"));
    assert!(summary.contains("vector indexes"));
    assert!(summary.contains("GDS similarity"));
    assert!(summary.contains("## Benchmark Details"));

    fs::remove_file(report_path)?;
    fs::remove_file(summary_path)?;
    Ok(())
}

#[test]
fn cli_live_benchmark_corpus_writes_scaled_artifacts() -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-live-benchmark-corpus-{}",
        StateCellId::new()
    ));
    let output = Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-corpus")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--size")
        .arg("12")
        .arg("--size")
        .arg("24")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.live_benchmark_corpus")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["scale_profiles"].as_array().map(Vec::len), Some(2));
    assert_eq!(json["aggregate"]["total_cell_count"].as_u64(), Some(36));
    assert!(
        json["aggregate"]["total_conflict_edge_count"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        json["aggregate"]["total_supersession_edge_count"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert_eq!(
        json["metrics_contract"]["latency"]["unit"].as_str(),
        Some("nanoseconds")
    );
    assert_eq!(
        json["metrics_contract"]["retrieval_quality"]["unit"].as_str(),
        Some("basis_points")
    );

    for size in [12, 24] {
        let manifest_path = artifact_dir.join(format!("corpus-{size}.manifest.json"));
        let cells_path = artifact_dir.join(format!("corpus-{size}.cells.jsonl"));
        let relationships_path = artifact_dir.join(format!("corpus-{size}.relationships.jsonl"));
        let queries_path = artifact_dir.join(format!("corpus-{size}.queries.json"));
        assert!(manifest_path.exists());
        assert!(cells_path.exists());
        assert!(relationships_path.exists());
        assert!(queries_path.exists());
        let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        assert_eq!(
            manifest["format"].as_str(),
            Some("continuitydb.live_benchmark_corpus_manifest")
        );
        assert_eq!(manifest["cell_count"].as_u64(), Some(size));
        assert!(
            manifest["relationship_counts"]["dependency"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        assert!(
            manifest["relationship_counts"]["conflict"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        assert!(
            manifest["relationship_counts"]["supersession"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
    }

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_live_benchmark_run_reports_target_contracts() -> Result<(), Box<dyn std::error::Error>> {
    for target in ["neo4j", "pinecone"] {
        let artifact_dir = std::env::temp_dir().join(format!(
            "continuitydb-live-benchmark-run-{target}-{}",
            StateCellId::new()
        ));
        let output = Command::cargo_bin("continuitydb")?
            .arg("live-benchmark-run")
            .arg("--target")
            .arg(target)
            .arg("--cells")
            .arg("16")
            .arg("--artifact-dir")
            .arg(&artifact_dir)
            .env_remove("NEO4J_URI")
            .env_remove("NEO4J_USERNAME")
            .env_remove("NEO4J_PASSWORD")
            .env_remove("PINECONE_API_KEY")
            .env_remove("PINECONE_INDEX_HOST")
            .env_remove("PINECONE_NAMESPACE")
            .env_remove("PINECONE_VECTOR_DIMENSION")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: Value = serde_json::from_slice(&output)?;

        assert_eq!(
            json["format"].as_str(),
            Some("continuitydb.live_benchmark_run")
        );
        assert_eq!(json["format_version"].as_u64(), Some(1));
        assert_eq!(json["valid"].as_bool(), Some(true));
        assert_eq!(json["target"]["provider"].as_str(), Some(target));
        assert_eq!(
            json["target"]["live_config_available"].as_bool(),
            Some(false)
        );
        assert_eq!(json["run_mode"].as_str(), Some("offline_dry_run"));
        assert_eq!(json["corpus"]["cell_count"].as_u64(), Some(16));
        assert!(
            json["metrics"]["latency"]["continuitydb_checkout_elapsed_nanos"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        assert_eq!(
            json["metrics"]["retrieval_quality"]["continuitydb_score_bps"].as_u64(),
            Some(10_000)
        );
        assert!(json["artifacts"]["manifest_path"].as_str().is_some());
        assert!(artifact_dir.join("live-benchmark-run.json").exists());
        assert!(artifact_dir
            .join("live-benchmark-corpus.manifest.json")
            .exists());
        assert!(artifact_dir.join("target-request.json").exists());

        fs::remove_dir_all(artifact_dir)?;
    }
    Ok(())
}

#[test]
fn cli_live_benchmark_run_retains_target_specific_payloads(
) -> Result<(), Box<dyn std::error::Error>> {
    let neo4j_dir = std::env::temp_dir().join(format!(
        "continuitydb-live-benchmark-neo4j-payloads-{}",
        StateCellId::new()
    ));
    let neo4j_output = Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-run")
        .arg("--target")
        .arg("neo4j")
        .arg("--cells")
        .arg("18")
        .arg("--artifact-dir")
        .arg(&neo4j_dir)
        .env_remove("NEO4J_URI")
        .env_remove("NEO4J_USERNAME")
        .env_remove("NEO4J_PASSWORD")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let neo4j: Value = serde_json::from_slice(&neo4j_output)?;
    let neo4j_payload_path = neo4j_dir.join("neo4j-cypher-payloads.json");
    assert!(neo4j_payload_path.exists());
    let neo4j_payload: Value = serde_json::from_slice(&fs::read(&neo4j_payload_path)?)?;
    assert_eq!(
        neo4j["target_result"]["request"]["cypher_payloads_path"].as_str(),
        Some(neo4j_payload_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        neo4j_payload["format"].as_str(),
        Some("continuitydb.live_benchmark.neo4j_payloads")
    );
    assert!(
        neo4j_payload["setup_cypher"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
            >= 2
    );
    assert!(
        neo4j_payload["load_cypher"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
            >= 2
    );
    assert!(neo4j_payload["benchmark_cypher"]
        .as_array()
        .is_some_and(|queries| queries
            .iter()
            .any(|query| query["id"].as_str() == Some("conflict_supersession_audit"))));

    let pinecone_dir = std::env::temp_dir().join(format!(
        "continuitydb-live-benchmark-pinecone-payloads-{}",
        StateCellId::new()
    ));
    let pinecone_output = Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-run")
        .arg("--target")
        .arg("pinecone")
        .arg("--cells")
        .arg("18")
        .arg("--artifact-dir")
        .arg(&pinecone_dir)
        .env_remove("PINECONE_API_KEY")
        .env_remove("PINECONE_INDEX_HOST")
        .env_remove("PINECONE_NAMESPACE")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let pinecone: Value = serde_json::from_slice(&pinecone_output)?;
    let pinecone_vectors_path = pinecone_dir.join("pinecone-vectors.jsonl");
    assert!(pinecone_vectors_path.exists());
    assert_eq!(
        pinecone["target_result"]["request"]["vectors_path"].as_str(),
        Some(pinecone_vectors_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        pinecone["target_result"]["request"]["vector_count"].as_u64(),
        Some(18)
    );
    assert_eq!(
        pinecone["target_result"]["request"]["batch_count"].as_u64(),
        Some(1)
    );
    let vector_lines = fs::read_to_string(&pinecone_vectors_path)?;
    assert_eq!(vector_lines.lines().count(), 18);
    assert!(vector_lines.contains("\"frontier\""));
    assert!(vector_lines.contains("\"supersession\""));

    fs::remove_dir_all(neo4j_dir)?;
    fs::remove_dir_all(pinecone_dir)?;
    Ok(())
}

#[test]
fn cli_live_benchmark_run_require_live_fails_without_config(
) -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-run")
        .arg("--target")
        .arg("neo4j")
        .arg("--cells")
        .arg("8")
        .arg("--require-live")
        .env_remove("NEO4J_URI")
        .env_remove("NEO4J_USERNAME")
        .env_remove("NEO4J_PASSWORD")
        .assert()
        .failure()
        .stderr(contains("NEO4J_URI"));

    Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-run")
        .arg("--target")
        .arg("pinecone")
        .arg("--cells")
        .arg("8")
        .arg("--require-live")
        .env_remove("PINECONE_API_KEY")
        .env_remove("PINECONE_INDEX_HOST")
        .env_remove("PINECONE_NAMESPACE")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .failure()
        .stderr(contains("PINECONE_API_KEY"));

    Ok(())
}

#[test]
fn cli_live_benchmark_smoke_report_validates_target_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-live-benchmark-smoke-report-{}",
        StateCellId::new()
    ));
    let report_path = artifact_dir.join("live-benchmark-smoke-report.json");
    let summary_path = artifact_dir.join("live-benchmark-smoke-report.md");

    let output = Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-smoke-report")
        .arg("--cells")
        .arg("20")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .env_remove("NEO4J_URI")
        .env_remove("NEO4J_USERNAME")
        .env_remove("NEO4J_PASSWORD")
        .env_remove("PINECONE_API_KEY")
        .env_remove("PINECONE_INDEX_HOST")
        .env_remove("PINECONE_NAMESPACE")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.live_benchmark_smoke_report")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["cell_count"].as_u64(), Some(20));
    assert_eq!(
        json["verdict"].as_str(),
        Some("dry_run_ready_live_evidence_blocked")
    );
    assert_eq!(json["targets"].as_array().map(Vec::len), Some(2));
    assert_eq!(json["aggregate"]["target_count"].as_u64(), Some(2));
    assert_eq!(json["aggregate"]["valid_target_count"].as_u64(), Some(2));
    assert_eq!(
        json["aggregate"]["live_executed_target_count"].as_u64(),
        Some(0)
    );
    let targets = json["targets"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing smoke report targets"))?;
    assert!(targets
        .iter()
        .all(|target| target["correctness"]["all_checks_passed"].as_bool() == Some(true)));
    assert!(targets.iter().any(|target| {
        target["provider"].as_str() == Some("neo4j")
            && target["correctness"]["checks"]["neo4j_cypher_payload_retained"].as_bool()
                == Some(true)
    }));
    assert!(targets.iter().any(|target| {
        target["provider"].as_str() == Some("pinecone")
            && target["correctness"]["checks"]["pinecone_vectors_retained"].as_bool() == Some(true)
    }));
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?,
        json
    );
    let summary = fs::read_to_string(&summary_path)?;
    assert!(summary.contains("# ContinuityDB Live Benchmark Smoke Report"));
    assert!(summary.contains("| neo4j | offline_dry_run | no | pass |"));
    assert!(summary.contains("| pinecone | offline_dry_run | no | pass |"));
    assert!(summary.contains("dry-run ready but live evidence is blocked"));
    assert!(artifact_dir.join("neo4j/live-benchmark-run.json").exists());
    assert!(artifact_dir
        .join("pinecone/live-benchmark-run.json")
        .exists());

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_live_benchmark_quality_report_scores_head_to_head() -> Result<(), Box<dyn std::error::Error>>
{
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-live-benchmark-quality-report-{}",
        StateCellId::new()
    ));
    let neo4j_dir = artifact_dir.join("neo4j");
    let pinecone_dir = artifact_dir.join("pinecone");
    let report_path = artifact_dir.join("quality-report.json");
    let summary_path = artifact_dir.join("quality-report.md");

    Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-run")
        .arg("--target")
        .arg("neo4j")
        .arg("--cells")
        .arg("24")
        .arg("--artifact-dir")
        .arg(&neo4j_dir)
        .env_remove("NEO4J_URI")
        .env_remove("NEO4J_USERNAME")
        .env_remove("NEO4J_PASSWORD")
        .assert()
        .success();
    Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-run")
        .arg("--target")
        .arg("pinecone")
        .arg("--cells")
        .arg("24")
        .arg("--artifact-dir")
        .arg(&pinecone_dir)
        .env_remove("PINECONE_API_KEY")
        .env_remove("PINECONE_INDEX_HOST")
        .env_remove("PINECONE_NAMESPACE")
        .env_remove("PINECONE_VECTOR_DIMENSION")
        .assert()
        .success();

    let neo4j_report_path = neo4j_dir.join("live-benchmark-run.json");
    let mut neo4j_report: Value = serde_json::from_slice(&fs::read(&neo4j_report_path)?)?;
    let neo4j_relationship_total = neo4j_report["corpus"]["relationship_counts"]["total"]
        .as_u64()
        .unwrap_or_default();
    neo4j_report["run_mode"] = Value::from("live_external_target");
    neo4j_report["target_result"]["live_executed"] = Value::from(true);
    neo4j_report["target_result"]["metrics"]["request_count"] = Value::from(6);
    neo4j_report["target_result"]["metrics"]["loaded_cell_count"] = Value::from(24);
    neo4j_report["target_result"]["metrics"]["loaded_relationship_count"] =
        Value::from(neo4j_relationship_total);
    neo4j_report["target_result"]["query_results"] = serde_json::json!([
        {"id": "frontier_context_checkout", "response": {"data": {"values": vec![Value::from(1); 20]}}},
        {"id": "conflict_supersession_audit", "response": {"data": {"values": vec![Value::from(1); 50]}}},
        {"id": "hybrid_vector_graph_context", "response": {"data": {"values": vec![Value::from(1); 50]}}}
    ]);
    fs::write(
        &neo4j_report_path,
        serde_json::to_string_pretty(&neo4j_report)?,
    )?;

    let pinecone_report_path = pinecone_dir.join("live-benchmark-run.json");
    let mut pinecone_report: Value = serde_json::from_slice(&fs::read(&pinecone_report_path)?)?;
    pinecone_report["run_mode"] = Value::from("live_external_target");
    pinecone_report["target_result"]["live_executed"] = Value::from(true);
    pinecone_report["target_result"]["metrics"]["request_count"] = Value::from(2);
    pinecone_report["target_result"]["metrics"]["upserted_count"] = Value::from(24);
    pinecone_report["target_result"]["query"]["matches"] = serde_json::json!([
        {"id": "cell-a", "metadata": {"frontier": false, "conflict_or_supersession": true}},
        {"id": "cell-b", "metadata": {"frontier": false, "conflict_or_supersession": true}}
    ]);
    fs::write(
        &pinecone_report_path,
        serde_json::to_string_pretty(&pinecone_report)?,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("live-benchmark-quality-report")
        .arg("--neo4j-report")
        .arg(&neo4j_report_path)
        .arg("--pinecone-report")
        .arg(&pinecone_report_path)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.live_benchmark_quality_report")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["cell_count"].as_u64(), Some(24));
    assert_eq!(json["winner"].as_str(), Some("continuitydb"));
    assert_eq!(json["strategies"].as_array().map(Vec::len), Some(3));
    assert_eq!(
        json["strategies"][0]["provider"].as_str(),
        Some("continuitydb")
    );
    assert_eq!(
        json["strategies"][0]["quality"]["overall_quality_bps"].as_u64(),
        Some(10_000)
    );
    assert!(
        json["strategies"][1]["quality"]["overall_quality_bps"]
            .as_u64()
            .unwrap_or_default()
            > json["strategies"][2]["quality"]["overall_quality_bps"]
                .as_u64()
                .unwrap_or_default()
    );
    assert_eq!(
        json["strategies"][2]["quality"]["native_statecell_contract_bps"].as_u64(),
        Some(0)
    );
    assert_eq!(
        json["comparison"]["continuitydb_vs_neo4j_delta_bps"].as_i64(),
        Some(
            json["strategies"][0]["quality"]["overall_quality_bps"]
                .as_i64()
                .unwrap_or_default()
                - json["strategies"][1]["quality"]["overall_quality_bps"]
                    .as_i64()
                    .unwrap_or_default()
        )
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?,
        json
    );
    let summary = fs::read_to_string(&summary_path)?;
    assert!(summary.contains("# ContinuityDB Live Benchmark Quality Report"));
    assert!(summary.contains("| continuitydb |"));
    assert!(summary.contains("| neo4j |"));
    assert!(summary.contains("| pinecone |"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_representative_benchmark_report_covers_agent_memory_workload(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-representative-benchmark-report-{}",
        StateCellId::new()
    ));
    let report_path = artifact_dir.join("representative-report.json");
    let summary_path = artifact_dir.join("representative-report.md");

    let output = Command::cargo_bin("continuitydb")?
        .arg("representative-benchmark-report")
        .arg("--cells")
        .arg("10000")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.representative_benchmark_report")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["cell_count"].as_u64(), Some(10_000));
    assert_eq!(json["winner"].as_str(), Some("continuitydb"));
    assert_eq!(
        json["corpus"]["scale"]["total_cells"].as_u64(),
        Some(10_000)
    );
    assert_eq!(json["corpus"]["families"].as_array().map(Vec::len), Some(6));
    assert_eq!(json["query_families"].as_array().map(Vec::len), Some(6));
    assert!(json["query_families"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing query families"))?
        .iter()
        .any(|family| family["id"].as_str() == Some("what_should_i_believe_now")));
    assert_eq!(json["strategies"].as_array().map(Vec::len), Some(3));
    assert_eq!(
        json["strategies"][0]["provider"].as_str(),
        Some("continuitydb")
    );
    assert!(
        json["strategies"][0]["score"]["overall_bps"]
            .as_u64()
            .unwrap_or_default()
            > json["strategies"][1]["score"]["overall_bps"]
                .as_u64()
                .unwrap_or_default()
    );
    assert!(
        json["strategies"][1]["score"]["overall_bps"]
            .as_u64()
            .unwrap_or_default()
            > json["strategies"][2]["score"]["overall_bps"]
                .as_u64()
                .unwrap_or_default()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?,
        json
    );
    let summary = fs::read_to_string(&summary_path)?;
    assert!(summary.contains("# ContinuityDB Representative Benchmark Report"));
    assert!(summary.contains("What should I believe now?"));
    assert!(summary.contains("| continuitydb |"));
    assert!(summary.contains("| neo4j |"));
    assert!(summary.contains("| pinecone |"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_adversarial_validation_report_challenges_the_hypothesis(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-adversarial-validation-report-{}",
        StateCellId::new()
    ));
    let report_path = artifact_dir.join("adversarial-report.json");
    let summary_path = artifact_dir.join("adversarial-report.md");

    let output = Command::cargo_bin("continuitydb")?
        .arg("adversarial-validation-report")
        .arg("--cells")
        .arg("10000")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--summary-path")
        .arg(&summary_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.adversarial_validation_report")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["cell_count"].as_u64(), Some(10_000));
    assert_eq!(
        json["verdict"].as_str(),
        Some("hypothesis_survives_adversarial_validation")
    );
    let baselines = json["baselines"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing adversarial baselines"))?;
    assert_eq!(baselines.len(), 3);
    assert!(baselines.iter().any(|baseline| {
        baseline["id"].as_str() == Some("expert_neo4j_graphrag")
            && baseline["concessions"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default()
                > 0
    }));
    assert!(baselines.iter().any(|baseline| {
        baseline["id"].as_str() == Some("real_embedding_pinecone")
            && baseline["remaining_gap"]
                .as_str()
                .unwrap_or_default()
                .contains("native revision")
    }));
    assert_eq!(json["blind_answer_quality"]["case_count"].as_u64(), Some(6));
    assert_eq!(
        json["blind_answer_quality"]["winner"].as_str(),
        Some("continuitydb")
    );
    assert!(
        json["blind_answer_quality"]["scores"]["continuitydb"]["overall_bps"]
            .as_u64()
            .unwrap_or_default()
            > json["blind_answer_quality"]["scores"]["neo4j"]["overall_bps"]
                .as_u64()
                .unwrap_or_default()
    );
    assert_eq!(json["limitations"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?,
        json
    );
    let summary = fs::read_to_string(&summary_path)?;
    assert!(summary.contains("# ContinuityDB Adversarial Validation Report"));
    assert!(summary.contains("expert Neo4j"));
    assert!(summary.contains("real embedding Pinecone"));
    assert!(summary.contains("Blind Answer Quality"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_adversarial_task_harness_report_scores_downstream_context_tasks(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-adversarial-task-harness-report-{}",
        StateCellId::new()
    ));
    let report_path = artifact_dir.join("adversarial-task-harness.json");

    let output = Command::cargo_bin("continuitydb")?
        .arg("adversarial-task-harness-report")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.adversarial_task_harness_report")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["report"]["case_count"].as_u64(), Some(7));
    assert_eq!(json["report"]["adaptive"]["total_count"].as_u64(), Some(9));
    assert_eq!(json["report"]["adaptive"]["passed_count"].as_u64(), Some(9));
    assert_eq!(
        json["report"]["adaptive"]["score_basis_points"].as_u64(),
        Some(10_000)
    );
    assert_eq!(json["report"]["baseline"]["total_count"].as_u64(), Some(9));
    assert!(
        json["report"]["baseline"]["score_basis_points"]
            .as_u64()
            .unwrap_or_default()
            < json["report"]["adaptive"]["score_basis_points"]
                .as_u64()
                .unwrap_or_default()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?,
        json
    );

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_proof_obligations_outputs_thesis_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("proof-obligations")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.thesis_proof_obligations")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("proof-obligations")
    );
    assert_eq!(json["thesis_document"].as_str(), Some("docs/thesis.md"));
    assert_eq!(json["summary"]["obligation_count"].as_u64(), Some(8));
    assert_eq!(json["summary"]["implemented_count"].as_u64(), Some(8));
    assert_eq!(
        json["summary"]["requires_local_model_feature_count"].as_u64(),
        Some(2)
    );

    let obligations = json["obligations"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing proof obligations"))?;
    assert_eq!(obligations.len(), 8);
    assert_eq!(
        obligations[0]["id"].as_str(),
        Some("deterministic_workload_replay")
    );
    assert_eq!(
        obligations[2]["id"].as_str(),
        Some("deterministic_continuity_checkout")
    );
    assert_eq!(
        obligations[2]["proof_surfaces"],
        serde_json::json!([
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
        ])
    );
    assert_eq!(
        obligations[3]["id"].as_str(),
        Some("context_collapse_prevention_drill")
    );
    assert_eq!(
        obligations[3]["proof_surfaces"],
        serde_json::json!([
            "context-collapse-drill",
            "proof-obligations",
            "examples/alpha_workflow.sh"
        ])
    );
    assert_eq!(
        obligations[6]["id"].as_str(),
        Some("local_model_acceptance_evidence")
    );
    assert_eq!(
        obligations[6]["requires_feature"].as_str(),
        Some("local-model")
    );
    assert_eq!(
        obligations[7]["id"].as_str(),
        Some("local_model_quality_gate_rejection")
    );
    assert_eq!(
        obligations[7]["artifacts"]
            .as_array()
            .and_then(|artifacts| artifacts.get(3))
            .and_then(Value::as_str),
        Some("aggregate acceptance_coverage")
    );
    Ok(())
}

#[test]
fn cli_validate_proof_obligations_accepts_current_matrix() -> Result<(), Box<dyn std::error::Error>>
{
    let report_path = temp_store_path("continuitydb-cli-proof-obligations-report");
    let validation_report_path =
        temp_store_path("continuitydb-cli-proof-obligations-validation-report");

    let report_output = Command::cargo_bin("continuitydb")?
        .arg("proof-obligations")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, report_output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-proof-obligations")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.thesis_proof_obligations_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(json["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(json["summary"]["obligation_count"].as_u64(), Some(8));

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_proof_obligations_rejects_tampered_matrix() -> Result<(), Box<dyn std::error::Error>>
{
    let report_path = temp_store_path("continuitydb-cli-proof-obligations-tamper-report");
    let failure_report_path = temp_store_path("continuitydb-cli-proof-obligations-tamper-failure");

    let report_output = Command::cargo_bin("continuitydb")?
        .arg("proof-obligations")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&report_output)?;
    report["obligations"][0]["status"] = Value::from("unverified");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-proof-obligations")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("proof-obligations obligation matrix mismatch"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("proof_obligations_validation")
    );
    assert_eq!(
        failure_report["proof_obligations_report"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-report").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-validation").with_extension("json");
    let inventory = sample_ci_artifact_inventory_json();
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.ci_artifact_inventory_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["contract_summary"]["total_required_artifact_count"].as_u64(),
        Some(87)
    );
    assert_eq!(
        json["contract_summary"]["total_required_check_count"].as_u64(),
        Some(276)
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_rejects_missing_context_gap_packet_check(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-context-gap-check")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-context-gap-check-failure")
            .with_extension("json");
    let mut inventory = sample_ci_artifact_inventory_json();
    let Some(alpha_checks) = inventory["required_checks"]["alpha_workflow"].as_array_mut() else {
        return Err("sample inventory missing alpha required checks".into());
    };
    let Some(context_gap_check) = alpha_checks.iter_mut().find(|check| {
        check["path"].as_str() == Some("target/alpha-workflow/query/checkout-context-packets.json")
            && check["required_pattern"].as_str() == Some("\"context_gaps\"")
    }) else {
        return Err("sample inventory missing context_gaps check".into());
    };
    context_gap_check["required_pattern"] = Value::from("\"context_gapz\"");
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("context_gaps"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("ci_artifact_inventory_validation")
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_rejects_missing_invalidation_condition_packet_check(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-invalidation-check")
            .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-ci-artifact-inventory-missing-invalidation-check-failure",
    )
    .with_extension("json");
    let mut inventory = sample_ci_artifact_inventory_json();
    let Some(alpha_checks) = inventory["required_checks"]["alpha_workflow"].as_array_mut() else {
        return Err("sample inventory missing alpha required checks".into());
    };
    let Some(invalidation_check) = alpha_checks.iter_mut().find(|check| {
        check["path"].as_str() == Some("target/alpha-workflow/query/checkout-context-packets.json")
            && check["required_pattern"].as_str() == Some("\"invalidation_conditions\"")
    }) else {
        return Err("sample inventory missing invalidation_conditions check".into());
    };
    invalidation_check["required_pattern"] = Value::from("\"invalidation_conditionz\"");
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("invalidation_conditions"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("ci_artifact_inventory_validation")
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_rejects_missing_trajectory_memory_packet_check(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-trajectory-check")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-trajectory-check-failure")
            .with_extension("json");
    let mut inventory = sample_ci_artifact_inventory_json();
    let Some(alpha_checks) = inventory["required_checks"]["alpha_workflow"].as_array_mut() else {
        return Err("sample inventory missing alpha required checks".into());
    };
    let Some(trajectory_check) = alpha_checks.iter_mut().find(|check| {
        check["path"].as_str() == Some("target/alpha-workflow/query/checkout-context-packets.json")
            && check["required_pattern"].as_str() == Some("\"trajectory_memory\"")
    }) else {
        return Err("sample inventory missing trajectory_memory check".into());
    };
    trajectory_check["required_pattern"] = Value::from("\"trajectory_memories\"");
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("trajectory_memory"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("ci_artifact_inventory_validation")
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_rejects_missing_context_compiler_schema_version_checks(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-context-compiler-version")
            .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-ci-artifact-inventory-missing-context-compiler-version-failure",
    )
    .with_extension("json");
    let mut inventory = sample_ci_artifact_inventory_json();
    let Some(smoke_checks) =
        inventory["required_checks"]["local_model_quality_gate_smoke"].as_array_mut()
    else {
        return Err("sample inventory missing local model smoke required checks".into());
    };
    let Some(version_check) = smoke_checks.iter_mut().find(|check| {
        check["path"].as_str()
            == Some(
                "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
            )
            && check["required_pattern"].as_str()
                == Some("\"context_compiler_schema_version\": 2")
    }) else {
        return Err("sample inventory missing context compiler schema version check".into());
    };
    version_check["required_pattern"] = Value::from("\"context_compiler_schema_version\": 1");
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("context_compiler_schema_version"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("ci_artifact_inventory_validation")
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_rejects_contract_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-tamper").with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-failure").with_extension("json");
    let mut inventory = sample_ci_artifact_inventory_json();
    inventory["contract_summary"]["total_required_check_count"] = Value::from(14);
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "ci inventory numeric mismatch at contract_summary.total_required_check_count",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("ci_artifact_inventory_validation")
    );
    assert_eq!(
        failure_report["ci_artifact_inventory_report"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_ci_artifact_inventory_rejects_missing_bundle_validation_summary(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-bundle-summary")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-ci-artifact-inventory-missing-bundle-summary-failure")
            .with_extension("json");
    let mut inventory = sample_ci_artifact_inventory_json();
    let Some(smoke_evidence) =
        inventory["verified_evidence"]["local_model_quality_gate_smoke"].as_object_mut()
    else {
        return Err("sample inventory missing local model smoke evidence object".into());
    };
    smoke_evidence.remove("operator_gate_candidate_bundle_validation_count");
    fs::write(&report_path, serde_json::to_string_pretty(&inventory)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-ci-artifact-inventory")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("operator_gate_candidate_bundle_validation_count"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("ci_artifact_inventory_validation")
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_release_assets_accepts_current_contract() -> Result<(), Box<dyn std::error::Error>>
{
    let artifact_dir = temp_store_path("continuitydb-cli-release-assets-dir").with_extension("dir");
    let manifest_path = artifact_dir.join("release-assets.json");
    let validation_report_path = artifact_dir.join("release-assets-validation.json");
    fs::create_dir_all(&artifact_dir)?;
    let manifest = write_sample_release_assets_manifest(&artifact_dir, "0.1.0")?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-release-assets")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.release_assets_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["asset_count"].as_u64(), Some(12));
    assert_eq!(json["crate_archive_count"].as_u64(), Some(10));
    assert_eq!(json["release_preflight_evidence_count"].as_u64(), Some(2));

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_release_assets_rejects_digest_drift() -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-release-assets-tamper-dir").with_extension("dir");
    let manifest_path = artifact_dir.join("release-assets.json");
    let failure_report_path = artifact_dir.join("release-assets-failure.json");
    fs::create_dir_all(&artifact_dir)?;
    let mut manifest = write_sample_release_assets_manifest(&artifact_dir, "0.1.0")?;
    manifest["assets"][0]["sha256"] =
        Value::from("0000000000000000000000000000000000000000000000000000000000000000");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-release-assets")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("release asset manifest sha256 mismatch"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("release_assets_validation")
    );
    assert_eq!(
        failure_report["release_asset_manifest"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_release_upload_report_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-release-upload-report-dir").with_extension("dir");
    let manifest_path = artifact_dir.join("release-assets.json");
    let report_path = artifact_dir.join("release-upload-report.json");
    let validation_report_path = artifact_dir.join("release-upload-report-validation.json");
    fs::create_dir_all(&artifact_dir)?;
    let manifest = write_sample_release_assets_manifest(&artifact_dir, "0.1.0")?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    let report = write_sample_release_upload_report(&manifest_path, "v0.1.0")?;
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-release-upload-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.release_upload_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["release_tag"].as_str(), Some("v0.1.0"));
    assert_eq!(
        json["release_repository"].as_str(),
        Some("syndicat/continuitydb")
    );
    assert_eq!(json["asset_count"].as_u64(), Some(12));
    assert_eq!(json["manifest_asset_count"].as_u64(), Some(12));

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_release_upload_report_accepts_failed_upload_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-release-upload-failure-report-dir").with_extension("dir");
    let manifest_path = artifact_dir.join("release-assets.json");
    let report_path = artifact_dir.join("release-upload-failure-report.json");
    let validation_report_path = artifact_dir.join("release-upload-failure-report-validation.json");
    fs::create_dir_all(&artifact_dir)?;
    let manifest = write_sample_release_assets_manifest(&artifact_dir, "0.1.0")?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    let mut report = write_sample_release_upload_report(&manifest_path, "v0.1.0")?;
    report["valid"] = Value::from(false);
    report["failure_stage"] = Value::from("upload");
    report["error"] =
        Value::from("gh release upload failed for v0.1.0 in repository syndicat/continuitydb");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-release-upload-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.release_upload_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["release_upload_succeeded"].as_bool(), Some(false));
    assert_eq!(json["failure_stage"].as_str(), Some("upload"));
    assert_eq!(json["asset_count"].as_u64(), Some(12));
    assert_eq!(json["manifest_asset_count"].as_u64(), Some(12));

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_report["release_upload_succeeded"].as_bool(),
        Some(false)
    );

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_release_upload_report_rejects_manifest_digest_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir =
        temp_store_path("continuitydb-cli-release-upload-report-tamper-dir").with_extension("dir");
    let manifest_path = artifact_dir.join("release-assets.json");
    let report_path = artifact_dir.join("release-upload-report.json");
    let failure_report_path = artifact_dir.join("release-upload-report-failure.json");
    fs::create_dir_all(&artifact_dir)?;
    let manifest = write_sample_release_assets_manifest(&artifact_dir, "0.1.0")?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    let mut report = write_sample_release_upload_report(&manifest_path, "v0.1.0")?;
    report["manifest"]["manifest_sha256"] =
        Value::from("0000000000000000000000000000000000000000000000000000000000000000");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-release-upload-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("release upload report manifest sha256 mismatch"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("release_upload_validation")
    );
    assert_eq!(
        failure_report["release_upload_report"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_release_upload_test_report_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-release-upload-test-report").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-release-upload-test-validation").with_extension("json");
    let report = sample_release_upload_test_report_json();
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-release-upload-test-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.release_upload_test_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["release_tag"].as_str(), Some("v0.1.0"));
    assert_eq!(
        json["release_repository"].as_str(),
        Some("syndicat/continuitydb")
    );
    assert_eq!(json["checked_condition_count"].as_u64(), Some(12));

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_release_upload_test_report_rejects_validation_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-release-upload-test-report-drift").with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-release-upload-test-failure").with_extension("json");
    let mut report = sample_release_upload_test_report_json();
    report["failure_report_validation_checked"] = Value::from(false);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-release-upload-test-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "release upload test report check failed: failure_report_validation_checked",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("release_upload_test_validation")
    );
    assert_eq!(
        failure_report["release_upload_test_report"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_reports_memory_kernel_json() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["kernel"].as_str(), Some("memory"));
    assert!(json["store_path"].is_null());
    assert_eq!(json["workload"]["cell_count"].as_u64(), Some(8));
    assert_eq!(json["workload"]["frontier_count"].as_u64(), Some(2));
    assert_eq!(json["revision_link_count"].as_u64(), Some(6));
    assert_eq!(json["ingest"]["operation_count"].as_u64(), Some(8));
    assert_eq!(
        json["checkout_operation"]["operation_count"].as_u64(),
        Some(1)
    );
    assert_eq!(json["checkout"]["matched_count"].as_u64(), Some(8));
    assert_eq!(json["checkout"]["selected_count"].as_u64(), Some(3));
    assert_eq!(json["checkout"]["alternative_count"].as_u64(), Some(5));
    Ok(())
}

#[test]
fn cli_measure_workload_reports_file_kernel_json() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-measure-workload-file");
    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("file")
        .arg("--store-path")
        .arg(&store_path)
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["kernel"].as_str(), Some("file"));
    assert_eq!(
        json["store_path"].as_str(),
        Some(store_path.display().to_string().as_str())
    );
    assert_eq!(json["workload"]["cell_count"].as_u64(), Some(8));
    assert_eq!(json["revision_link_count"].as_u64(), Some(6));
    assert_eq!(json["checkout"]["matched_count"].as_u64(), Some(8));
    assert_eq!(
        json["lookup_plan"]["indexed_constraints"],
        serde_json::json!(["scope", "minimum_confidence"])
    );
    assert_eq!(json["lookup_plan"]["candidate_count"].as_u64(), Some(8));
    assert_eq!(json["lookup_plan"]["full_scan"].as_bool(), Some(false));
    assert!(store_path.exists());

    fs::remove_file(store_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_requires_file_store_path() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("file")
        .assert()
        .failure()
        .stderr(contains("store path is required"));

    Ok(())
}

#[test]
fn cli_measure_workload_records_baseline_for_memory_kernel(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-measure-workload-memory-baseline");
    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-small")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let baseline_text = fs::read_to_string(&baseline_path)?;
    let records: Vec<Value> = baseline_text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    assert_eq!(
        json["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(json["baseline_label"].as_str(), Some("memory-small"));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["label"].as_str(), Some("memory-small"));
    assert_eq!(records[0]["kernel"].as_str(), Some("memory"));
    assert_eq!(
        records[0]["snapshot"]["workload"]["cell_count"].as_u64(),
        Some(8)
    );
    assert_eq!(
        records[0]["snapshot"]["checkout"]["matched_count"].as_u64(),
        Some(8)
    );

    fs::remove_file(baseline_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_report_path_writes_json_artifact() -> Result<(), Box<dyn std::error::Error>>
{
    let report_path = temp_store_path("continuitydb-cli-measure-workload-report")
        .with_extension("dir")
        .join("workload-report.json");
    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(report_json, stdout_json);
    assert_eq!(report_json["kernel"].as_str(), Some("memory"));
    assert_eq!(report_json["workload"]["cell_count"].as_u64(), Some(8));

    fs::remove_file(&report_path)?;
    fs::remove_dir(
        report_path
            .parent()
            .ok_or_else(|| std::io::Error::other("report path should have a parent directory"))?,
    )?;
    Ok(())
}

#[test]
fn cli_measure_workload_artifact_dir_writes_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-measure-workload-artifact-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_path = artifact_dir.join("workload-report.json");
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let cells_path = artifact_dir.join("workload-cells.json");
    let checkout_request_path = artifact_dir.join("checkout-request.json");

    assert_eq!(stdout_json, report_json);
    assert_eq!(stdout_json["kernel"].as_str(), Some("memory"));
    assert_eq!(stdout_json["workload"]["cell_count"].as_u64(), Some(8));
    assert_eq!(
        stdout_json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        stdout_json["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert!(stdout_json["bundle_manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(stdout_json["bundle_manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        stdout_json["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert_eq!(
        stdout_json["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(checkout_request_path.display().to_string().as_str())
    );
    assert!(stdout_json["workload_artifacts"]["cells_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(
        stdout_json["workload_artifacts"]["checkout_request_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );

    let bundle_manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    assert_eq!(
        bundle_manifest_json["format"].as_str(),
        Some("continuitydb.workload.bundle")
    );
    assert_eq!(bundle_manifest_json["format_version"].as_u64(), Some(1));
    assert_eq!(
        bundle_manifest_json["workload_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(bundle_manifest_json["workload_report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(bundle_manifest_json["workload_report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(bundle_manifest_json["kernel"].as_str(), Some("memory"));
    assert_eq!(
        bundle_manifest_json["baseline_comparison"],
        serde_json::Value::Null
    );
    assert_eq!(
        bundle_manifest_json["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest_json["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(checkout_request_path.display().to_string().as_str())
    );
    let cells_json: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    assert_eq!(
        cells_json["format"].as_str(),
        Some("continuitydb.workload.cells")
    );
    assert_eq!(cells_json["format_version"].as_u64(), Some(1));
    assert_eq!(cells_json["cells"].as_array().map(Vec::len), Some(8));
    let checkout_request_json: Value =
        serde_json::from_str(&fs::read_to_string(&checkout_request_path)?)?;
    assert_eq!(
        checkout_request_json["format"].as_str(),
        Some("continuitydb.workload.checkout_request")
    );
    assert_eq!(checkout_request_json["format_version"].as_u64(), Some(1));
    assert_eq!(
        checkout_request_json["request"]["scope"]["Project"].as_str(),
        Some("continuitydb")
    );
    assert_eq!(
        checkout_request_json["request"]["minimum_confidence"].as_f64(),
        Some(0.0)
    );
    assert_eq!(
        checkout_request_json["request"]["token_budget"].as_i64(),
        Some(400)
    );
    assert!(checkout_request_json["request"]["revision_related_cell"].is_null());
    assert!(checkout_request_json["request"]["revision_link_kind"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_replays_artifact_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-artifact-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let replay_report_path =
        temp_store_path("continuitydb-cli-replay-workload-report").with_extension("json");
    if replay_report_path.exists() {
        fs::remove_file(&replay_report_path)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&replay_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["kernel"].as_str(), Some("memory"));
    assert_eq!(
        json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(json["workload"]["cell_count"].as_u64(), Some(8));
    assert_eq!(json["checkout"]["matched_count"].as_u64(), Some(8));
    assert_eq!(json["checkout"]["selected_count"].as_u64(), Some(3));
    assert_eq!(json["checkout"]["alternative_count"].as_u64(), Some(5));
    assert_eq!(
        json["workload_artifacts"]["cells_path"].as_str(),
        Some(
            artifact_dir
                .join("workload-cells.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert!(json["workload_artifacts"]["cells_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["report_path"].as_str(),
        Some(replay_report_path.display().to_string().as_str())
    );
    let replay_report: Value = serde_json::from_str(&fs::read_to_string(&replay_report_path)?)?;
    assert_eq!(replay_report["kernel"].as_str(), Some("memory"));
    assert_eq!(
        replay_report["checkout"]["selected_count"].as_u64(),
        Some(3)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(replay_report_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_tampered_fixture(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-require-manifest-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    let mut cells_artifact: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    cells_artifact["summary"]["cell_count"] = Value::from(7);
    fs::write(&cells_path, serde_json::to_string_pretty(&cells_artifact)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_fixture_path_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-path-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_artifacts"]["cells_path"] = Value::from(
        artifact_dir
            .join("unexpected-workload-cells.json")
            .display()
            .to_string(),
    );
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest path mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_report_path_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-report-path-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_report_path"] = Value::from(
        artifact_dir
            .join("unexpected-workload-report.json")
            .display()
            .to_string(),
    );
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest report path mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_artifact_dir_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-artifact-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["artifact_dir"] = Value::from(
        artifact_dir
            .join("unexpected-source-dir")
            .display()
            .to_string(),
    );
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest directory mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_workload_summary_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-summary-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload"]["cell_count"] = Value::from(7);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains(
            "workload artifact manifest workload summary mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_fixture_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_artifacts"]["cells_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest byte count mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_report_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-report-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest byte count mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_report_fingerprint_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-report-fingerprint-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_report_fingerprint"] = Value::from("fnv1a64:0000000000000000");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_rejects_report_content_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-report-content-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let report_path = artifact_dir.join("workload-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    report["workload"]["cell_count"] = Value::from(7);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    refresh_workload_manifest_report_metadata(&artifact_dir, &report)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains(
            "workload artifact manifest report content mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_reports_lookup_plan_content_mismatch_key(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-lookup-plan-content-dir-{}",
        std::process::id()
    ));
    let store_path = temp_store_path("continuitydb-cli-replay-workload-manifest-lookup-plan-store");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if store_path.exists() {
        fs::remove_file(&store_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("file")
        .arg("--store-path")
        .arg(&store_path)
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let report_path = artifact_dir.join("workload-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    report["lookup_plan"]["candidate_selectivity_basis_points"] = Value::from(1);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    refresh_workload_manifest_report_metadata(&artifact_dir, &report)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("file")
        .arg("--store-path")
        .arg(&store_path)
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .assert()
        .failure()
        .stderr(contains(
            "workload artifact manifest report content mismatch",
        ))
        .stderr(contains("lookup_plan"));

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(store_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_failure_report_records_validation_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-failure-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-replay-workload-manifest-failure").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut cells_artifact: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    cells_artifact["summary"]["cell_count"] = Value::from(7);
    fs::write(&cells_path, serde_json::to_string_pretty(&cells_artifact)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("input_manifest_validation")
    );
    assert_eq!(
        failure_report["failure"]["message"].as_str(),
        Some("workload artifact manifest fingerprint mismatch")
    );
    assert_eq!(
        failure_report["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_artifacts"]["cells_parse_error"].is_null());
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_artifacts"]["checkout_request_parse_error"].is_null());
    assert_eq!(
        failure_report["input_bundle_manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert!(
        failure_report["input_bundle_manifest"]["manifest_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert!(failure_report["input_bundle_manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["input_bundle_manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["input_bundle_manifest"]["parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_failure_report_records_cells_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-cells-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-replay-workload-cells-parse").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    fs::write(&cells_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("input_manifest_validation")
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["workload_artifacts"]["cells_parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert_eq!(
        failure_report["workload_artifacts"]["cells_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_failure_report_records_checkout_request_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-request-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-replay-workload-request-parse").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let request_path = artifact_dir.join("checkout-request.json");
    fs::write(&request_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("input_manifest_validation")
    );
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(request_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_parseable"].as_bool(),
        Some(false)
    );
    assert!(
        failure_report["workload_artifacts"]["checkout_request_parse_error"]
            .as_str()
            .is_some_and(|message| message.contains("line 1 column"))
    );
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_failure_report_records_manifest_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-replay-workload-manifest-parse").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    fs::write(&manifest_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("input_manifest_validation")
    );
    assert_eq!(
        failure_report["input_bundle_manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["input_bundle_manifest"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["input_bundle_manifest"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(
        failure_report["input_bundle_manifest"]["manifest_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert_eq!(
        failure_report["input_bundle_manifest"]["manifest_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_writes_validation_failure_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-failure-bundle-dir-{}",
        std::process::id()
    ));
    let replay_artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-manifest-failure-replay-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if replay_artifact_dir.exists() {
        fs::remove_dir_all(&replay_artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    let mut cells_artifact: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    cells_artifact["summary"]["cell_count"] = Value::from(7);
    fs::write(&cells_path, serde_json::to_string_pretty(&cells_artifact)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .arg("--replay-artifact-dir")
        .arg(&replay_artifact_dir)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    let replay_report_path = replay_artifact_dir.join("replay-report.json");
    let replay_manifest_path =
        replay_artifact_dir.join("continuitydb-workload-replay.manifest.json");
    let replay_report: Value = serde_json::from_str(&fs::read_to_string(&replay_report_path)?)?;
    let replay_manifest: Value = serde_json::from_str(&fs::read_to_string(&replay_manifest_path)?)?;

    assert_eq!(
        replay_report["failure"]["stage"].as_str(),
        Some("input_manifest_validation")
    );
    assert_eq!(
        replay_report["replay_bundle_manifest"]["manifest_path"].as_str(),
        Some(replay_manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        replay_manifest["failure"]["stage"].as_str(),
        Some("input_manifest_validation")
    );
    assert_eq!(
        replay_manifest["input_artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        replay_manifest["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_dir_all(replay_artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_require_manifest_reports_validated_manifest(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-input-manifest-dir-{}",
        std::process::id()
    ));
    let replay_artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-input-manifest-replay-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if replay_artifact_dir.exists() {
        fs::remove_dir_all(&replay_artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let input_manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let output = Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--require-manifest")
        .arg("--replay-artifact-dir")
        .arg(&replay_artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        report["input_bundle_manifest"]["manifest_path"].as_str(),
        Some(input_manifest_path.display().to_string().as_str())
    );
    assert!(report["input_bundle_manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(report["input_bundle_manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(report["revision_link_count"].as_u64(), Some(6));

    let replay_manifest_path =
        replay_artifact_dir.join("continuitydb-workload-replay.manifest.json");
    let replay_manifest: Value = serde_json::from_str(&fs::read_to_string(replay_manifest_path)?)?;
    assert_eq!(
        replay_manifest["input_bundle_manifest"]["manifest_path"].as_str(),
        Some(input_manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        replay_manifest["input_bundle_manifest"]["manifest_fingerprint"],
        report["input_bundle_manifest"]["manifest_fingerprint"]
    );
    assert_eq!(replay_manifest["revision_link_count"].as_u64(), Some(6));

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_dir_all(replay_artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_compares_archived_report() -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-compare-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();
    let report_path = artifact_dir.join("workload-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    report["checkout"]["selected_count"] = Value::from(2);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--compare-report")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    assert_eq!(json["replay_comparison"]["passed"].as_bool(), Some(false));
    assert_eq!(
        json["replay_comparison"]["mismatches"][0]["CheckoutSelectedCountChanged"]["previous"]
            .as_u64(),
        Some(2)
    );
    assert_eq!(
        json["replay_comparison"]["mismatches"][0]["CheckoutSelectedCountChanged"]["current"]
            .as_u64(),
        Some(3)
    );

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--compare-report")
        .arg("--fail-on-mismatch")
        .assert()
        .failure()
        .stderr(contains("workload replay mismatch detected"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_failure_report_path_records_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-failure-report-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-replay-workload-failure-report").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();
    let report_path = artifact_dir.join("workload-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    report["checkout"]["selected_count"] = Value::from(2);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--fail-on-mismatch")
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("workload replay mismatch detected"));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["replay_comparison"]["passed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        failure_report["replay_comparison"]["mismatches"][0]["CheckoutSelectedCountChanged"]
            ["previous"]
            .as_u64(),
        Some(2)
    );
    assert_eq!(
        failure_report["replay_comparison"]["mismatches"][0]["CheckoutSelectedCountChanged"]
            ["current"]
            .as_u64(),
        Some(3)
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_artifacts"]["cells_parse_error"].is_null());
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_artifacts"]["checkout_request_parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_replay_workload_artifact_dir_writes_mismatch_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-bundle-input-dir-{}",
        std::process::id()
    ));
    let replay_artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-bundle-output-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if replay_artifact_dir.exists() {
        fs::remove_dir_all(&replay_artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();
    let archived_report_path = artifact_dir.join("workload-report.json");
    let mut archived_report: Value =
        serde_json::from_str(&fs::read_to_string(&archived_report_path)?)?;
    archived_report["checkout"]["selected_count"] = Value::from(2);
    fs::write(
        &archived_report_path,
        serde_json::to_string_pretty(&archived_report)?,
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--fail-on-mismatch")
        .arg("--replay-artifact-dir")
        .arg(&replay_artifact_dir)
        .assert()
        .failure()
        .stderr(contains("workload replay mismatch detected"));

    let replay_report_path = replay_artifact_dir.join("replay-report.json");
    let replay_manifest_path =
        replay_artifact_dir.join("continuitydb-workload-replay.manifest.json");
    let replay_report: Value = serde_json::from_str(&fs::read_to_string(&replay_report_path)?)?;
    let replay_manifest: Value = serde_json::from_str(&fs::read_to_string(&replay_manifest_path)?)?;

    assert_eq!(
        replay_report["replay_artifact_dir"].as_str(),
        Some(replay_artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        replay_report["replay_bundle_manifest"]["manifest_path"].as_str(),
        Some(replay_manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        replay_report["replay_comparison"]["passed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        replay_manifest["format"].as_str(),
        Some("continuitydb.workload.replay_bundle")
    );
    assert_eq!(replay_manifest["format_version"].as_u64(), Some(1));
    assert_eq!(
        replay_manifest["replay_report_path"].as_str(),
        Some(replay_report_path.display().to_string().as_str())
    );
    assert!(replay_manifest["replay_report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(replay_manifest["replay_report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        replay_manifest["replay_report_parseable"].as_bool(),
        Some(true)
    );
    assert!(replay_manifest["replay_report_parse_error"].is_null());
    assert_eq!(
        replay_manifest["input_artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        replay_manifest["replay_comparison"]["mismatches"][0]["CheckoutSelectedCountChanged"]
            ["previous"]
            .as_u64(),
        Some(2)
    );
    assert!(
        replay_report["replay_bundle_manifest"]["manifest_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert!(replay_report["replay_bundle_manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        replay_report["replay_bundle_manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(replay_report["replay_bundle_manifest"]["parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_dir_all(replay_artifact_dir)?;
    Ok(())
}

#[test]
fn cli_replay_workload_artifact_dir_rejects_input_directory(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-replay-workload-same-bundle-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    Command::cargo_bin("continuitydb")?
        .arg("replay-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--replay-artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "--replay-artifact-dir must differ from --artifact-dir",
        ));

    assert!(!artifact_dir
        .join("continuitydb-workload-replay.manifest.json")
        .exists());

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_records_baseline() -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-baseline");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let baseline_text = fs::read_to_string(&baseline_path)?;
    let records: Vec<Value> = baseline_text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    assert_eq!(
        json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["candidate_role"].as_str(), Some("default-feasibility"));
    assert_eq!(json["passed"].as_bool(), Some(true));
    assert_eq!(json["passed_cases"].as_u64(), Some(9));
    assert_eq!(json["failed_cases"].as_u64(), Some(0));
    assert_eq!(json["total_cases"].as_u64(), Some(9));
    assert_eq!(json["pass_rate"].as_f64(), Some(1.0));
    assert_eq!(
        json["evaluation"]["case_reports"][0]["name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][0]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][1]["name"].as_str(),
        Some("conflict classification")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][1]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][2]["name"].as_str(),
        Some("supersession classification")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][2]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][3]["name"].as_str(),
        Some("unsupported claim boundary")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][3]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][4]["name"].as_str(),
        Some("confidence adjustment")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][4]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][5]["name"].as_str(),
        Some("targeted verification request")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][5]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][6]["name"].as_str(),
        Some("new evidence draft creation")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][6]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][7]["name"].as_str(),
        Some("multi-source citation preservation")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][7]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["evaluation"]["case_reports"][8]["name"].as_str(),
        Some("policy rejection avoidance")
    );
    assert_eq!(
        json["evaluation"]["case_reports"][8]["failures"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(json["response_schema_version"].as_u64(), Some(1));
    assert!(json["evaluation_suite_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(json["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["schema_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
    assert!(json["grammar_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert!(json["prompt_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["response_fingerprints"][0]["case_name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert_eq!(
        json["response_fingerprints"][0]["captured"].as_bool(),
        Some(true)
    );
    assert!(json["response_fingerprints"][0]["response_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["response_fingerprints"][0]["response_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        json["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(
        json["runtime"]["executable"].as_str(),
        Some(executable_path.display().to_string().as_str())
    );
    assert_eq!(json["runtime"]["arguments"][0].as_str(), Some("--model"));
    assert_eq!(
        json["runtime"]["arguments"][1].as_str(),
        Some("/models/qwen.gguf")
    );
    assert_eq!(json["runtime"]["arguments"][2].as_str(), Some("--temp"));
    assert_eq!(json["runtime"]["arguments"][3].as_str(), Some("0"));

    assert_eq!(records.len(), 1);
    assert_eq!(
        json["candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        records[0]["candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        records[0]["candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        records[0]["runtime"]["executable"].as_str(),
        Some(executable_path.display().to_string().as_str())
    );
    assert_eq!(records[0]["runtime"]["arguments"][3].as_str(), Some("0"));
    assert_eq!(records[0]["response_schema_version"].as_u64(), Some(1));
    assert_eq!(
        records[0]["evaluation_suite_fingerprint"].as_str(),
        json["evaluation_suite_fingerprint"].as_str()
    );
    assert_eq!(
        records[0]["schema_fingerprint"].as_str(),
        json["schema_fingerprint"].as_str()
    );
    assert_eq!(
        records[0]["grammar_fingerprint"].as_str(),
        json["grammar_fingerprint"].as_str()
    );
    assert_eq!(records[0]["schema_bytes"], json["schema_bytes"]);
    assert_eq!(records[0]["grammar_bytes"], json["grammar_bytes"]);
    assert_eq!(
        records[0]["response_fingerprints"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        records[0]["response_fingerprints"][0]["response_fingerprint"].as_str(),
        json["response_fingerprints"][0]["response_fingerprint"].as_str()
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-preflight-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-preflight-baseline");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    let before = fs::read_to_string(&baseline_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--compare-baseline")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["baseline_preflight"]["compared"].as_bool(), Some(true));
    assert_eq!(
        json["baseline_preflight"]["compatible_baseline_found"].as_bool(),
        Some(true)
    );
    assert!(json["baseline_preflight"]["previous_recorded_at"].is_string());
    assert_eq!(
        json["baseline_preflight"]["previous_schema_bytes"].as_u64(),
        json["schema_bytes"].as_u64()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_grammar_bytes"].as_u64(),
        json["grammar_bytes"].as_u64()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_response_schema_version"].as_u64(),
        json["response_schema_version"].as_u64()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_evaluation_suite_fingerprint"].as_str(),
        json["evaluation_suite_fingerprint"].as_str()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_schema_fingerprint"].as_str(),
        json["schema_fingerprint"].as_str()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_grammar_fingerprint"].as_str(),
        json["grammar_fingerprint"].as_str()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_prompt_fingerprint"].as_str(),
        json["prompt_fingerprint"].as_str()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        json["baseline_preflight"]["previous_candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["baseline_preflight"]["previous_runtime"]["executable"].as_str(),
        json["runtime"]["executable"].as_str()
    );
    assert_eq!(
        json["baseline_preflight"]["previous_runtime"]["arguments"],
        json["runtime"]["arguments"]
    );
    assert_eq!(fs::read_to_string(&baseline_path)?, before);

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-dry-run-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["will_record_baseline"].as_bool(), Some(false));
    assert_eq!(
        json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["candidate_role"].as_str(), Some("default-feasibility"));
    assert_eq!(json["response_schema_version"].as_u64(), Some(1));
    assert!(json.get("baseline_preflight").is_none());
    assert!(json["evaluation_suite_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(json["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["schema_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
    assert!(json["grammar_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert!(json["prompt_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(
        json["runtime"]["executable"].as_str(),
        Some("/missing/local-model-runner")
    );
    assert_eq!(json["runtime"]["arguments"][0].as_str(), Some("--model"));
    assert_eq!(
        json["runtime"]["arguments"][1].as_str(),
        Some("/models/qwen.gguf")
    );
    assert_eq!(json["runtime"]["arguments"][2].as_str(), Some("--temp"));
    assert_eq!(json["runtime"]["arguments"][3].as_str(), Some("0"));
    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_report_path_writes_dry_run_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-report-dry-run-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-report-dry-run");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(stdout_json, report_json);
    assert_eq!(report_json["dry_run"].as_bool(), Some(true));
    assert_eq!(
        report_json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(report_json["will_record_baseline"].as_bool(), Some(false));
    assert!(!baseline_path.exists());

    fs::remove_file(report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_benchmark_report_accepts_dry_run_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-report-validation-baseline");
    let report_path =
        temp_store_path("continuitydb-cli-local-model-report-validation").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-report-validation-output")
            .with_extension("json");

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-benchmark-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_benchmark_report_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(
        json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["evaluation_suite_fingerprint"].as_str(),
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?["evaluation_suite_fingerprint"]
            .as_str()
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_report_path_writes_passing_run_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-report-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-report-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-report");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(stdout_json, report_json);
    assert_eq!(report_json["passed"].as_bool(), Some(true));
    assert_eq!(report_json["total_cases"].as_u64(), Some(9));
    assert_eq!(
        report_json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert!(baseline_path.exists());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_response_dir_writes_response_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-response-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-response-baseline");
    let response_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-response-dir-{}",
        std::process::id()
    ));
    if response_dir.exists() {
        fs::remove_dir_all(&response_dir)?;
    }
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--response-dir")
        .arg(&response_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let artifacts = json["response_artifacts"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing response artifacts"))?;

    assert_eq!(artifacts.len(), 9);
    assert_eq!(
        artifacts[0]["case_name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert_eq!(artifacts[0]["captured"].as_bool(), Some(true));
    assert!(artifacts[0]["response_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(artifacts[0]["response_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    let first_response_path = artifacts[0]["response_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing response path"))?;
    let first_response = fs::read_to_string(first_response_path)?;
    assert!(first_response.contains("Gather additional source evidence."));
    assert!(first_response.contains("continuitydb://evaluation/thin-evidence"));
    let manifest = &json["response_artifact_manifest"];
    let manifest_path = manifest["manifest_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing response artifact manifest path"))?;
    assert!(manifest_path.ends_with("local-model-responses.manifest.json"));
    assert!(manifest["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(manifest["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    let manifest_json: Value = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    assert_eq!(
        manifest_json["format"].as_str(),
        Some("continuitydb.local_model.responses")
    );
    assert_eq!(manifest_json["format_version"].as_u64(), Some(1));
    assert_eq!(manifest_json["artifacts"].as_array().map(Vec::len), Some(9));
    assert_eq!(
        manifest_json["artifacts"][0]["case_name"].as_str(),
        artifacts[0]["case_name"].as_str()
    );
    assert_eq!(
        manifest_json["artifacts"][0]["response_path"].as_str(),
        artifacts[0]["response_path"].as_str()
    );
    assert_eq!(
        manifest_json["artifacts"][0]["response_fingerprint"].as_str(),
        artifacts[0]["response_fingerprint"].as_str()
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(response_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_artifact_dir_writes_real_run_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-artifact-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-artifact-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-artifact-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_path = artifact_dir.join("benchmark-report.json");
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");

    assert_eq!(stdout_json, report_json);
    assert_eq!(stdout_json["passed"].as_bool(), Some(true));
    assert!(bundle_manifest_path.exists());
    assert_eq!(
        stdout_json["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert!(stdout_json["bundle_manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(stdout_json["bundle_manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    let bundle_manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    assert_eq!(
        bundle_manifest_json["format"].as_str(),
        Some("continuitydb.local_model.benchmark_bundle")
    );
    assert_eq!(bundle_manifest_json["format_version"].as_u64(), Some(1));
    assert_eq!(
        bundle_manifest_json["benchmark_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(bundle_manifest_json["benchmark_report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(bundle_manifest_json["benchmark_report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        bundle_manifest_json["prompt_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert_eq!(
        bundle_manifest_json["response_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert!(
        bundle_manifest_json["response_artifact_manifest"]["manifest_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("responses/local-model-responses.manifest.json"))
    );
    assert_eq!(
        stdout_json["contract_artifacts"]["schema_path"].as_str(),
        Some(
            artifact_dir
                .join("contracts/local-model-response.schema.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        stdout_json["prompt_artifacts"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        stdout_json["response_artifacts"].as_array().map(Vec::len),
        Some(9)
    );
    assert!(artifact_dir
        .join("responses/local-model-responses.manifest.json")
        .exists());
    assert!(stdout_json["response_artifact_manifest"]["manifest_path"]
        .as_str()
        .is_some_and(|path| path.ends_with("responses/local-model-responses.manifest.json")));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_artifact_dir_writes_dry_run_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-artifact-dry-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-artifact-dry-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_path = artifact_dir.join("benchmark-report.json");
    let validation_report_path = artifact_dir.join("benchmark-report-validation.json");
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");

    assert_eq!(stdout_json, report_json);
    assert_eq!(stdout_json["dry_run"].as_bool(), Some(true));
    assert!(bundle_manifest_path.exists());
    assert_eq!(
        stdout_json["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert!(stdout_json["bundle_manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    let bundle_manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    assert_eq!(
        bundle_manifest_json["format"].as_str(),
        Some("continuitydb.local_model.benchmark_bundle")
    );
    assert_eq!(
        bundle_manifest_json["benchmark_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest_json["prompt_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert_eq!(
        bundle_manifest_json["response_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(bundle_manifest_json["response_artifact_manifest"].is_null());
    assert!(artifact_dir
        .join("contracts/local-model-response.schema.json")
        .exists());
    assert_eq!(
        stdout_json["prompt_artifacts"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        stdout_json["response_artifacts"].as_array().map(Vec::len),
        Some(0)
    );
    assert!(stdout_json["response_artifact_manifest"].is_null());
    assert!(!artifact_dir.join("responses").exists());
    assert!(!baseline_path.exists());

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-benchmark-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validation_json: Value = serde_json::from_slice(&validation_output)?;
    assert_eq!(
        validation_json["format"].as_str(),
        Some("continuitydb.local_model_benchmark_report_validation")
    );
    assert_eq!(validation_json["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_json["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert!(validation_report_path.exists());

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_accepts_manifest_metadata() -> Result<(), Box<dyn std::error::Error>>
{
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.workload.bundle_validation")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["manifest_path"].as_str(),
        Some(
            artifact_dir
                .join("continuitydb-workload.manifest.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        json["workload_report"]["report_path"].as_str(),
        Some(
            artifact_dir
                .join("workload-report.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        json["workload_report"]["revision_link_count"].as_u64(),
        Some(6)
    );
    assert_eq!(
        json["workload_artifacts"]["cells_path"].as_str(),
        Some(
            artifact_dir
                .join("workload-cells.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        json["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(
            artifact_dir
                .join("checkout-request.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        json["workload_artifacts"]["cells_parseable"].as_bool(),
        Some(true)
    );
    assert!(json["workload_artifacts"]["cells_parse_error"].is_null());
    assert_eq!(
        json["workload_artifacts"]["checkout_request_parseable"].as_bool(),
        Some(true)
    );
    assert!(json["workload_artifacts"]["checkout_request_parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_rejects_tampered_fixture() -> Result<(), Box<dyn std::error::Error>>
{
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-tampered-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    let mut cells_artifact: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    cells_artifact["summary"]["cell_count"] = Value::from(7);
    fs::write(&cells_path, serde_json::to_string_pretty(&cells_artifact)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_rejects_revision_link_count_not_derived_from_fixture(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-revision-link-count-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let report_path = artifact_dir.join("workload-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    assert_eq!(report["workload"]["dependency_count"].as_u64(), Some(6));
    assert_eq!(report["revision_link_count"].as_u64(), Some(6));
    report["revision_link_count"] = Value::from(0);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    refresh_workload_manifest_report_metadata(&artifact_dir, &report)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "workload artifact report revision link count mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_report_path_writes_validation_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-report-dir-{}",
        std::process::id()
    ));
    let report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-report").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if report_path.exists() {
        fs::remove_file(&report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(report_json, stdout_json);
    assert_eq!(
        report_json["manifest"]["manifest_path"].as_str(),
        Some(
            artifact_dir
                .join("continuitydb-workload.manifest.json")
                .display()
                .to_string()
                .as_str()
        )
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_path_records_validation_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-failure-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-failure").with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    let mut cells_artifact: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    cells_artifact["summary"]["cell_count"] = Value::from(7);
    fs::write(&cells_path, serde_json::to_string_pretty(&cells_artifact)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("workload_bundle_validation")
    );
    assert_eq!(
        failure_report["failure"]["message"].as_str(),
        Some("workload artifact manifest fingerprint mismatch")
    );
    assert_eq!(
        failure_report["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_fixture_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-failure-evidence-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-failure-evidence")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    let request_path = artifact_dir.join("checkout-request.json");
    let mut cells_artifact: Value = serde_json::from_str(&fs::read_to_string(&cells_path)?)?;
    cells_artifact["summary"]["cell_count"] = Value::from(7);
    fs::write(&cells_path, serde_json::to_string_pretty(&cells_artifact)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert!(failure_report["workload_artifacts"]["cells_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(failure_report["workload_artifacts"]["cells_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(request_path.display().to_string().as_str())
    );
    assert!(
        failure_report["workload_artifacts"]["checkout_request_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert!(
        failure_report["workload_artifacts"]["checkout_request_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0)
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_artifacts"]["cells_parse_error"].is_null());
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_artifacts"]["checkout_request_parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_fixture_cells_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-cells-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-cells-parse")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let cells_path = artifact_dir.join("workload-cells.json");
    fs::write(&cells_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_artifacts"]["cells_parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["workload_artifacts"]["cells_parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(failure_report["workload_artifacts"]["cells_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["workload_artifacts"]["cells_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_checkout_request_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-request-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-request-parse")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let request_path = artifact_dir.join("checkout-request.json");
    fs::write(&request_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(request_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_parseable"].as_bool(),
        Some(false)
    );
    assert!(
        failure_report["workload_artifacts"]["checkout_request_parse_error"]
            .as_str()
            .is_some_and(|message| message.contains("line 1 column"))
    );
    assert!(
        failure_report["workload_artifacts"]["checkout_request_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert_eq!(
        failure_report["workload_artifacts"]["checkout_request_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_manifest_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-manifest-evidence-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-manifest-evidence")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["artifact_dir"] = Value::from("/tmp/wrong-workload-artifact-dir");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest directory mismatch"));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert!(failure_report["manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(failure_report["manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["manifest"]["parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_manifest_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-manifest-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-manifest-parse")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    fs::write(&manifest_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["manifest"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["manifest"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(failure_report["manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["manifest"]["manifest_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_workload_report_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-report-evidence-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-report-evidence")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let report_path = artifact_dir.join("workload-report.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["workload_report_fingerprint"] = Value::from("fnv1a64:0000000000000000");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("workload artifact manifest fingerprint mismatch"));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["workload_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(failure_report["workload_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(failure_report["workload_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["workload_report"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["workload_report"]["parse_error"].is_null());

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_workload_bundle_failure_report_records_workload_report_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-validate-workload-bundle-report-parse-dir-{}",
        std::process::id()
    ));
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-workload-bundle-report-parse")
            .with_extension("json");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--cells")
        .arg("8")
        .arg("--token-budget")
        .arg("400")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let report_path = artifact_dir.join("workload-report.json");
    fs::write(&report_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-workload-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["workload_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["workload_report"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["workload_report"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(failure_report["workload_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["workload_report"]["report_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_accepts_report_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-validate-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let report_path = artifact_dir.join("benchmark-report.json");
    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_bundle_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert!(json["manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        json["benchmark_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(json["benchmark_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["benchmark_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(json["contract_artifacts"], report["contract_artifacts"]);

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_report_path_writes_validation_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-validate-report-baseline");
    let report_path =
        temp_store_path("continuitydb-cli-local-model-validate-report").with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-report-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if report_path.exists() {
        fs::remove_file(&report_path)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout_json: Value = serde_json::from_slice(&output)?;
    let report_json: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(report_json, stdout_json);
    assert_eq!(
        report_json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_failure_report_path_records_validation_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-validate-failure-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-failure-report")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-failure-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest byte count mismatch",
        ));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("local_model_bundle_validation")
    );
    assert_eq!(
        failure_report["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert!(failure_report["manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(failure_report["manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["manifest"]["parse_error"].is_null());
    let benchmark_report_path = artifact_dir.join("benchmark-report.json");
    assert_eq!(
        failure_report["benchmark_report"]["report_path"].as_str(),
        Some(benchmark_report_path.display().to_string().as_str())
    );
    assert!(failure_report["benchmark_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(failure_report["benchmark_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["benchmark_report"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["benchmark_report"]["parse_error"].is_null());
    assert!(failure_report["failure"]["message"].as_str().is_some_and(
        |message| message.contains("local model benchmark manifest byte count mismatch")
    ));

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_manifest_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-manifest-parse-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-manifest-parse-failure")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-manifest-parse-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    fs::write(&manifest_path, "{not valid json")?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["manifest"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["manifest"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(failure_report["manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["manifest"]["manifest_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_contract_artifact_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-contract-failure-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-contract-failure-report")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-contract-failure-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let schema_path = report["contract_artifacts"]["schema_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing schema path"))?;
    let grammar_path = report["contract_artifacts"]["grammar_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing grammar path"))?;
    let schema_text = fs::read_to_string(schema_path)?;
    let grammar_text = fs::read_to_string(grammar_path)?;
    let tampered_schema = "x".repeat(schema_text.len());
    fs::write(schema_path, &tampered_schema)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model contract artifact schema fingerprint mismatch",
        ));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["contract_artifacts"]["schema_path"].as_str(),
        Some(schema_path)
    );
    assert!(failure_report["contract_artifacts"]["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["contract_artifacts"]["schema_bytes"].as_u64(),
        Some(tampered_schema.len() as u64)
    );
    assert_eq!(
        failure_report["contract_artifacts"]["grammar_path"].as_str(),
        Some(grammar_path)
    );
    assert!(failure_report["contract_artifacts"]["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["contract_artifacts"]["grammar_bytes"].as_u64(),
        Some(grammar_text.len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_benchmark_report_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-report-parse-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-report-parse-failure")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-report-parse-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    fs::write(&report_path, "{not valid json")?;
    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_bytes"] = Value::from("{not valid json".len());
    manifest["benchmark_report_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint("{not valid json"));
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["benchmark_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["benchmark_report"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["benchmark_report"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(failure_report["benchmark_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["benchmark_report"]["report_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_changed_case_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-failure-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-failure-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-failure-report")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-failure-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest byte count mismatch",
        ));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    assert_eq!(
        failure_report["changed_case_report"]["report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["changed_case_report"]["changed_case_report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["changed_case_report"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["changed_case_report"]["parse_error"].is_null());
    assert!(failure_report["changed_case_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(failure_report["changed_case_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        failure_report["changed_case_report"]["format"].as_str(),
        Some("continuitydb.local_model.changed_cases")
    );
    assert_eq!(
        failure_report["changed_case_report"]["format_version"].as_u64(),
        Some(1)
    );
    assert_eq!(
        failure_report["changed_case_report"]["candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        failure_report["changed_case_report"]["candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        failure_report["changed_case_report"]["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        failure_report["changed_case_report"]["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["changed_case_report"]["comparison"]["changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        failure_report["changed_case_report"]["comparison"]["response_changed_cases"].as_u64(),
        Some(0)
    );
    assert!(failure_report["changed_case_report"]["comparison"].is_object());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_response_manifest_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-failure-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-failure-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-failure-report")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-failure-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest byte count mismatch",
        ));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    assert_eq!(
        failure_report["response_artifact_manifest"]["manifest_path"].as_str(),
        Some(response_manifest_path.display().to_string().as_str())
    );
    assert!(
        failure_report["response_artifact_manifest"]["manifest_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert!(
        failure_report["response_artifact_manifest"]["manifest_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0)
    );
    assert_eq!(
        failure_report["response_artifact_manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(failure_report["response_artifact_manifest"]["parse_error"].is_null());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_response_artifact_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-failure-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-failure-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-failure-report")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-artifact-failure-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    let response_path = response_manifest["artifacts"][0]["response_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing response path"))?;
    let original_response = fs::read_to_string(response_path)?;
    fs::write(response_path, "x".repeat(original_response.len()))?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model response artifact fingerprint mismatch",
        ));

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["response_artifacts"][0]["case_name"].as_str(),
        response_manifest["artifacts"][0]["case_name"].as_str()
    );
    assert_eq!(
        failure_report["response_artifacts"][0]["captured"].as_bool(),
        Some(true)
    );
    assert_eq!(
        failure_report["response_artifacts"][0]["response_path"].as_str(),
        Some(response_path)
    );
    assert!(
        failure_report["response_artifacts"][0]["response_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert_eq!(
        failure_report["response_artifacts"][0]["response_bytes"].as_u64(),
        Some(original_response.len() as u64)
    );
    assert_eq!(
        failure_report["response_artifacts"][0]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["response_artifacts"][0]["parse_error"]
        .as_str()
        .is_some_and(|error| error.contains("expected value")));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_report_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-validate-bytes-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest byte count mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_report_fingerprint_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-fingerprint-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-fingerprint-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_fingerprint"] = Value::from("fnv1a64:0000000000000000");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest fingerprint mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_tampered_prompt_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-prompt-tampered-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-prompt-tampered-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let prompt_path = report["prompt_artifacts"][0]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let original_prompt = fs::read_to_string(prompt_path)?;
    fs::write(prompt_path, "x".repeat(original_prompt.len()))?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains("local model prompt artifact fingerprint mismatch"));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_tampered_contract_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-contract-tampered-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-contract-tampered-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let schema_path = report["contract_artifacts"]["schema_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing schema path"))?;
    let schema_text = fs::read_to_string(schema_path)?;
    fs::write(schema_path, "x".repeat(schema_text.len()))?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model contract artifact schema fingerprint mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_tampered_context_compiler_contract_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path(
        "continuitydb-cli-local-model-validate-context-compiler-contract-tampered-baseline",
    );
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-context-compiler-contract-tampered-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let schema_path = report["contract_artifacts"]["context_compiler_schema_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing context compiler schema path"))?;
    let schema_text = fs::read_to_string(schema_path)?;
    fs::write(schema_path, "x".repeat(schema_text.len()))?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model context compiler contract artifact schema fingerprint mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_context_compiler_contract_version_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-context-contract-version-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-context-contract-version-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    report["contract_artifacts"]["context_compiler_schema_version"] = Value::from(1);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["contract_artifacts"] = report["contract_artifacts"].clone();
    let mut canonical_report = report.clone();
    canonical_report["bundle_manifest"] = Value::Null;
    let canonical_report_text = serde_json::to_string_pretty(&canonical_report)?;
    manifest["benchmark_report_bytes"] = Value::from(canonical_report_text.len());
    manifest["benchmark_report_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&canonical_report_text));
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model context compiler contract artifact schema version mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_bundle_rejects_contract_artifact_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-contract-bytes-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-contract-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let report_path = artifact_dir.join("benchmark-report.json");
    let mut report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    report["contract_artifacts"]["schema_bytes"] = Value::from(1);
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["contract_artifacts"] = report["contract_artifacts"].clone();
    let mut canonical_report = report.clone();
    canonical_report["bundle_manifest"] = Value::Null;
    let canonical_report_text = serde_json::to_string_pretty(&canonical_report)?;
    manifest["benchmark_report_bytes"] = Value::from(canonical_report_text.len());
    manifest["benchmark_report_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&canonical_report_text));
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model contract artifact schema byte count mismatch",
        ));

    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_accepts_changed_case_report_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-validate-changed-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-validate-changed-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let changed_case_report_path = artifact_dir.join("changed-cases.json");

    assert_eq!(
        json["changed_case_report"]["report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["changed_case_report"]["changed_case_report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["changed_case_report"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(json["changed_case_report"]["parse_error"].is_null());
    assert!(json["changed_case_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["changed_case_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        json["changed_case_report"]["format"].as_str(),
        Some("continuitydb.local_model.changed_cases")
    );
    assert_eq!(
        json["changed_case_report"]["format_version"].as_u64(),
        Some(1)
    );
    assert_eq!(
        json["changed_case_report"]["candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        json["changed_case_report"]["candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["changed_case_report"]["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["changed_case_report"]["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(
        json["changed_case_report"]["comparison"]["changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        json["changed_case_report"]["comparison"]["response_changed_cases"].as_u64(),
        Some(0)
    );
    assert!(json["changed_case_report"]["comparison"].is_object());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_changed_case_report_path_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-path-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-path-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-path-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["changed_case_report"]["report_path"] = Value::from(
        artifact_dir
            .join("wrong-changed-cases.json")
            .display()
            .to_string(),
    );
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest changed-case report path mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_changed_case_report_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-bytes-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-bytes-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["changed_case_report"]["report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest changed-case report byte count mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_changed_case_report_fingerprint_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-fingerprint-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-fingerprint-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-fingerprint-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["changed_case_report"]["report_fingerprint"] = Value::from("fnv1a64:0000000000000000");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest changed-case report fingerprint mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_changed_case_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-parse-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-parse-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-parse-failure")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-parse-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    fs::write(&changed_case_report_path, "{not valid json")?;
    refresh_local_model_changed_case_report_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["changed_case_report"]["report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["changed_case_report"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["changed_case_report"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(failure_report["changed_case_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        failure_report["changed_case_report"]["report_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_changed_case_report_candidate_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-candidate-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-candidate-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-candidate-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    let mut changed_case_report: Value =
        serde_json::from_str(&fs::read_to_string(&changed_case_report_path)?)?;
    changed_case_report["candidate_model_id"] = Value::from("Qwen/tampered-model");
    fs::write(
        &changed_case_report_path,
        serde_json::to_string_pretty(&changed_case_report)?,
    )?;
    refresh_local_model_changed_case_report_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains("local model changed-case report content mismatch"));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_changed_case_report_candidate_selection_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-selection-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-selection-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-selection-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    let mut changed_case_report: Value =
        serde_json::from_str(&fs::read_to_string(&changed_case_report_path)?)?;
    changed_case_report["candidate_selection"]["source"] = Value::from("tampered");
    fs::write(
        &changed_case_report_path,
        serde_json::to_string_pretty(&changed_case_report)?,
    )?;
    refresh_local_model_changed_case_report_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains("local model changed-case report content mismatch"));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_changed_case_report_comparison_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-comparison-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-changed-comparison-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-changed-comparison-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_changed_case_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    let mut changed_case_report: Value =
        serde_json::from_str(&fs::read_to_string(&changed_case_report_path)?)?;
    let original_changed_cases = changed_case_report["comparison"]["changed_cases"]
        .as_u64()
        .ok_or_else(|| std::io::Error::other("missing changed case count"))?;
    changed_case_report["comparison"]["changed_cases"] = Value::from(original_changed_cases + 1);
    fs::write(
        &changed_case_report_path,
        serde_json::to_string_pretty(&changed_case_report)?,
    )?;
    refresh_local_model_changed_case_report_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains("local model changed-case report content mismatch"));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_accepts_response_artifact_manifest_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-validate-response-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-validate-response-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");

    assert_eq!(
        json["response_artifact_manifest"]["manifest_path"].as_str(),
        Some(response_manifest_path.display().to_string().as_str())
    );
    assert!(json["response_artifact_manifest"]["manifest_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["response_artifact_manifest"]["manifest_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(
        json["response_artifact_manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(json["response_artifact_manifest"]["parse_error"].is_null());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_accepts_response_artifact_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifacts-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifacts-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-artifacts-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let response_artifacts = json["response_artifacts"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing response artifacts"))?;
    let first_response = &response_artifacts[0];

    assert_eq!(response_artifacts.len(), 9);
    assert_eq!(first_response["captured"].as_bool(), Some(true));
    assert!(first_response["case_name"]
        .as_str()
        .is_some_and(|case_name| !case_name.is_empty()));
    assert!(first_response["response_path"]
        .as_str()
        .is_some_and(|path| path.ends_with(".response.json")));
    assert!(first_response["response_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(first_response["response_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_artifact_manifest_path_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-path-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-path-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-path-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["response_artifact_manifest"]["manifest_path"] = Value::from(
        artifact_dir
            .join("responses")
            .join("wrong-local-model-responses.manifest.json")
            .display()
            .to_string(),
    );
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest response artifact manifest path mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_artifact_manifest_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-bytes-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-bytes-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["response_artifact_manifest"]["manifest_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest response artifact manifest byte count mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_artifact_manifest_fingerprint_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-fingerprint-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-fingerprint-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-fingerprint-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["response_artifact_manifest"]["manifest_fingerprint"] =
        Value::from("fnv1a64:0000000000000000");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark manifest response artifact manifest fingerprint mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_failure_report_records_response_manifest_parse_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-parse-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-parse-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-parse-failure")
            .with_extension("json");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-parse-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    if failure_report_path.exists() {
        fs::remove_file(&failure_report_path)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    fs::write(&response_manifest_path, "{not valid json")?;
    refresh_local_model_response_manifest_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure();

    let failure_report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        failure_report["response_artifact_manifest"]["manifest_path"].as_str(),
        Some(response_manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["response_artifact_manifest"]["parseable"].as_bool(),
        Some(false)
    );
    assert!(failure_report["response_artifact_manifest"]["parse_error"]
        .as_str()
        .is_some_and(|message| message.contains("line 1 column")));
    assert!(
        failure_report["response_artifact_manifest"]["manifest_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert_eq!(
        failure_report["response_artifact_manifest"]["manifest_bytes"].as_u64(),
        Some("{not valid json".len() as u64)
    );
    assert!(failure_report["response_artifacts"].is_null());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-manifest-case-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-manifest-case-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-manifest-case-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let mut response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    response_manifest["artifacts"][0]["case_name"] = Value::from("tampered-case-name");
    fs::write(
        &response_manifest_path,
        serde_json::to_string_pretty(&response_manifest)?,
    )?;
    refresh_local_model_response_manifest_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model response artifact manifest content mismatch: artifacts",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_manifest_capture_state_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-manifest-captured-runner");
    let baseline_path = temp_store_path(
        "continuitydb-cli-local-model-validate-response-manifest-captured-baseline",
    );
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-manifest-captured-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let mut response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    response_manifest["artifacts"][0]["captured"] = Value::from(false);
    fs::write(
        &response_manifest_path,
        serde_json::to_string_pretty(&response_manifest)?,
    )?;
    refresh_local_model_response_manifest_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model response artifact manifest content mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_tampered_response_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-artifact-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    let response_path = response_manifest["artifacts"][0]["response_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing response path"))?;
    let original_response = fs::read_to_string(response_path)?;
    fs::write(response_path, "x".repeat(original_response.len()))?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model response artifact fingerprint mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_artifact_byte_count_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-bytes-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-bytes-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-artifact-bytes-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let mut response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    response_manifest["artifacts"][0]["response_bytes"] = Value::from(1);
    fs::write(
        &response_manifest_path,
        serde_json::to_string_pretty(&response_manifest)?,
    )?;
    refresh_local_model_response_manifest_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model response artifact byte count mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_artifact_fingerprint_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path(
        "continuitydb-cli-local-model-validate-response-artifact-fingerprint-runner",
    );
    let baseline_path = temp_store_path(
        "continuitydb-cli-local-model-validate-response-artifact-fingerprint-baseline",
    );
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-artifact-fingerprint-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let mut response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    response_manifest["artifacts"][0]["response_fingerprint"] =
        Value::from("fnv1a64:0000000000000000");
    fs::write(
        &response_manifest_path,
        serde_json::to_string_pretty(&response_manifest)?,
    )?;
    refresh_local_model_response_manifest_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains(
            "local model response artifact fingerprint mismatch",
        ));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_validate_local_model_bundle_rejects_response_artifact_path_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-path-runner");
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-validate-response-artifact-path-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-validate-response-artifact-path-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }

    write_real_local_model_bundle(&artifact_dir, &baseline_path, &executable_path)?;

    let response_manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let mut response_manifest: Value =
        serde_json::from_str(&fs::read_to_string(&response_manifest_path)?)?;
    response_manifest["artifacts"][0]["response_path"] = Value::from(
        artifact_dir
            .join("outside-response-artifact.json")
            .display()
            .to_string(),
    );
    fs::write(
        &response_manifest_path,
        serde_json::to_string_pretty(&response_manifest)?,
    )?;
    refresh_local_model_response_manifest_metadata(&artifact_dir)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .failure()
        .stderr(contains("local model response artifact path mismatch"));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_failure_report_path_dry_run_reports_gate(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-failed-cases-dry-run");
    let report_path = temp_store_path("continuitydb-cli-local-model-failed-cases-dry-run-report");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--fail-on-failed-cases")
        .arg("--failure-report-path")
        .arg(&report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["fail_on_failed_cases"].as_bool(), Some(true));
    assert_eq!(
        json["failure_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(!baseline_path.exists());
    assert!(!report_path.exists());
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_failure_report_path_rejects_failed_evaluation_without_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-failed-cases-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-failed-cases-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-failed-cases-report");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--fail-on-failed-cases")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--failure-report-path")
        .arg(&report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark fixed evaluation cases failed",
        ));

    assert!(!baseline_path.exists());
    let report_text = fs::read_to_string(&report_path)?;
    let report: Value = serde_json::from_str(&report_text)?;

    assert_eq!(report["passed"].as_bool(), Some(false));
    assert_eq!(report["failed_cases"].as_u64(), Some(8));
    assert_eq!(
        report["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(
        report["evaluation"]["case_reports"][1]["name"].as_str(),
        Some("conflict classification")
    );
    assert!(report["evaluation"]["case_reports"][1]["failures"]
        .as_array()
        .is_some_and(|failures| !failures.is_empty()));
    assert_eq!(
        report["failure_counts"]["missing_expected_action"].as_u64(),
        Some(8)
    );
    assert_eq!(
        report["failure_counts"]["missing_citation"].as_u64(),
        Some(9)
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_artifact_dir_writes_failed_case_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-failed-bundle-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-failed-bundle-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-failed-bundle-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--fail-on-failed-cases")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model benchmark fixed evaluation cases failed",
        ));

    assert!(!baseline_path.exists());
    let report_path = artifact_dir.join("benchmark-report.json");
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest: Value = serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;

    assert_eq!(report["passed"].as_bool(), Some(false));
    assert_eq!(report["failed_cases"].as_u64(), Some(8));
    assert_eq!(
        report["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert_eq!(report["bundle_manifest"]["parseable"].as_bool(), Some(true));
    assert!(report["bundle_manifest"]["parse_error"].is_null());
    assert_eq!(
        bundle_manifest["format"].as_str(),
        Some("continuitydb.local_model.benchmark_bundle")
    );
    assert_eq!(
        bundle_manifest["benchmark_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["benchmark_report_parseable"].as_bool(),
        Some(true)
    );
    assert!(bundle_manifest["benchmark_report_parse_error"].is_null());
    assert_eq!(
        bundle_manifest["prompt_artifacts"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        bundle_manifest["response_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert_eq!(
        report["response_artifacts"][0]["parseable"].as_bool(),
        Some(true)
    );
    assert!(report["response_artifacts"][0]["parse_error"].is_null());
    assert_eq!(
        bundle_manifest["response_artifacts"][0]["parseable"].as_bool(),
        Some(true)
    );
    assert!(bundle_manifest["response_artifacts"][0]["parse_error"].is_null());
    assert!(
        bundle_manifest["response_artifact_manifest"]["manifest_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("responses/local-model-responses.manifest.json"))
    );
    assert_eq!(
        report["response_artifact_manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert!(report["response_artifact_manifest"]["parse_error"].is_null());
    assert!(artifact_dir
        .join("contracts/local-model-response.schema.json")
        .exists());
    assert!(artifact_dir
        .join("responses/local-model-responses.manifest.json")
        .exists());

    fs::remove_file(executable_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_artifact_dir_writes_regression_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-regression-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-regression-bundle-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-regression-bundle-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    fs::write(&executable_path, passing_local_model_runner_script())?;
    let regressed_script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]}]}'
"#;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    fs::write(&executable_path, regressed_script)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--fail-on-regression")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("local model benchmark regression detected"));

    let baseline_text = fs::read_to_string(&baseline_path)?;
    assert_eq!(baseline_text.lines().count(), 1);
    let report_path = artifact_dir.join("benchmark-report.json");
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest: Value = serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;

    assert_eq!(report["passed"].as_bool(), Some(false));
    assert_eq!(
        report["baseline_comparison"]["regressed"].as_bool(),
        Some(true)
    );
    assert_eq!(
        report["baseline_comparison"]["previous_candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        report["baseline_comparison"]["previous_candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        report["baseline_comparison"]["current_candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        report["baseline_comparison"]["current_candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        report["baseline_comparison"]["previous_passed_cases"].as_u64(),
        Some(9)
    );
    assert_eq!(
        report["baseline_comparison"]["current_passed_cases"].as_u64(),
        Some(1)
    );
    assert_eq!(
        report["baseline_comparison"]["pass_count_delta"].as_i64(),
        Some(-8)
    );
    assert_eq!(
        report["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["benchmark_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["response_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert!(
        bundle_manifest["response_artifact_manifest"]["manifest_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("responses/local-model-responses.manifest.json"))
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_failure_report_path_records_regression_gate(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-regression-report-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-regression-report-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-regression-report");
    fs::write(&executable_path, passing_local_model_runner_script())?;
    let regressed_script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]}]}'
"#;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    fs::write(&executable_path, regressed_script)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--fail-on-regression")
        .arg("--failure-report-path")
        .arg(&report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("local model benchmark regression detected"));

    let baseline_text = fs::read_to_string(&baseline_path)?;
    assert_eq!(baseline_text.lines().count(), 1);
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

    assert_eq!(report["passed"].as_bool(), Some(false));
    assert_eq!(
        report["baseline_comparison"]["regressed"].as_bool(),
        Some(true)
    );
    assert_eq!(
        report["baseline_comparison"]["previous_passed_cases"].as_u64(),
        Some(9)
    );
    assert_eq!(
        report["baseline_comparison"]["current_passed_cases"].as_u64(),
        Some(1)
    );
    assert_eq!(
        report["baseline_comparison"]["pass_count_delta"].as_i64(),
        Some(-8)
    );
    assert_eq!(
        report["baseline_comparison"]["previous_failure_counts"]
            .as_object()
            .map(serde_json::Map::len),
        Some(0)
    );
    assert_eq!(
        report["baseline_comparison"]["current_failure_counts"]["missing_expected_action"].as_u64(),
        Some(8)
    );
    assert_eq!(
        report["baseline_comparison"]["current_failure_counts"]["missing_citation"].as_u64(),
        Some(9)
    );
    assert_eq!(
        report["baseline_comparison"]["failure_count_deltas"]["missing_expected_action"].as_i64(),
        Some(8)
    );
    assert_eq!(
        report["baseline_comparison"]["failure_count_deltas"]["missing_citation"].as_i64(),
        Some(9)
    );
    assert_eq!(
        report["baseline_comparison"]["regressed_cases"]
            .as_array()
            .map(Vec::len),
        Some(8)
    );
    assert!(
        report["baseline_comparison"]["regressed_cases"]
            .as_array()
            .is_some_and(
                |cases| cases.contains(&Value::String("conflict classification".to_string()))
            )
    );
    assert_eq!(
        report["baseline_comparison"]["recovered_cases"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    let changed_cases = report["baseline_comparison"]["changed_case_summaries"]
        .as_array()
        .ok_or("missing changed case summaries")?;
    let conflict_case = changed_cases
        .iter()
        .find(|case| case["case_name"] == "conflict classification")
        .ok_or("missing conflict case summary")?;
    assert_eq!(conflict_case["previous_passed"].as_bool(), Some(true));
    assert_eq!(conflict_case["current_passed"].as_bool(), Some(false));
    assert_eq!(
        conflict_case["failure_count_deltas"]["missing_expected_action"].as_i64(),
        Some(1)
    );
    assert!(report["bundle_manifest"].is_null());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_reports_same_outcome_failure_changes(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-same-outcome-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-same-outcome-baseline");
    let previous_script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status remains on the frontier.","citations":["continuitydb://evaluation/unrelated"]}]}'
"#;
    let current_script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[]}'
"#;
    fs::write(&executable_path, previous_script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    fs::write(&executable_path, current_script)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--compare-baseline")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let report: Value = serde_json::from_slice(&output)?;
    assert_eq!(
        report["baseline_comparison"]["regressed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["baseline_comparison"]["pass_count_delta"].as_i64(),
        Some(0)
    );
    assert_eq!(
        report["baseline_comparison"]["changed_cases"].as_u64(),
        Some(9)
    );
    assert_eq!(
        report["baseline_comparison"]["outcome_changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["baseline_comparison"]["failure_count_changed_cases"].as_u64(),
        Some(1)
    );
    assert_eq!(
        report["baseline_comparison"]["response_changed_cases"].as_u64(),
        Some(9)
    );
    let changed_cases = report["baseline_comparison"]["changed_case_summaries"]
        .as_array()
        .ok_or("missing changed case summaries")?;
    assert!(
        changed_cases.iter().any(|case| {
            case["previous_passed"].as_bool() == Some(false)
                && case["current_passed"].as_bool() == Some(false)
                && case["failure_count_deltas"]
                    .as_object()
                    .is_some_and(|deltas| !deltas.is_empty())
        }),
        "expected same-outcome changed failure summary in {changed_cases:?}"
    );
    let changed_case = changed_cases
        .iter()
        .find(|case| {
            case["previous_passed"].as_bool() == Some(false)
                && case["current_passed"].as_bool() == Some(false)
                && case["failure_count_deltas"]
                    .as_object()
                    .is_some_and(|deltas| !deltas.is_empty())
        })
        .ok_or("missing same-outcome changed failure summary")?;
    assert!(changed_case["previous_response_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(changed_case["current_response_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_ne!(
        changed_case["previous_response_fingerprint"].as_str(),
        changed_case["current_response_fingerprint"].as_str()
    );
    assert!(changed_case["previous_response_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert!(changed_case["current_response_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_reports_passing_response_changes(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-response-change-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-response-change-baseline");
    let current_script = passing_local_model_runner_script().replace(
        "The evidence is thin, so uncertainty remains.",
        "Uncertainty remains because the evidence is thin.",
    );
    fs::write(&executable_path, passing_local_model_runner_script())?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    fs::write(&executable_path, current_script)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--compare-baseline")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let report: Value = serde_json::from_slice(&output)?;
    assert_eq!(report["passed"].as_bool(), Some(true));
    assert_eq!(
        report["baseline_comparison"]["regressed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["baseline_comparison"]["pass_count_delta"].as_i64(),
        Some(0)
    );
    assert_eq!(
        report["baseline_comparison"]["changed_cases"].as_u64(),
        Some(9)
    );
    assert_eq!(
        report["baseline_comparison"]["outcome_changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["baseline_comparison"]["failure_count_changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["baseline_comparison"]["response_changed_cases"].as_u64(),
        Some(9)
    );
    let changed_cases = report["baseline_comparison"]["changed_case_summaries"]
        .as_array()
        .ok_or("missing changed case summaries")?;
    let changed_case = changed_cases
        .iter()
        .find(|case| {
            case["previous_passed"].as_bool() == Some(true)
                && case["current_passed"].as_bool() == Some(true)
                && case["failure_count_deltas"]
                    .as_object()
                    .is_some_and(serde_json::Map::is_empty)
        })
        .ok_or("missing passing response-change summary")?;
    assert_eq!(changed_case["outcome_changed"].as_bool(), Some(false));
    assert_eq!(
        changed_case["failure_counts_changed"].as_bool(),
        Some(false)
    );
    assert_eq!(changed_case["response_changed"].as_bool(), Some(true));
    assert_ne!(
        changed_case["previous_response_fingerprint"].as_str(),
        changed_case["current_response_fingerprint"].as_str()
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_changed_case_report_path_writes_compact_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-changed-report-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-changed-report-baseline");
    let changed_case_report_path =
        temp_store_path("continuitydb-cli-local-model-changed-report").with_extension("json");
    let current_script = passing_local_model_runner_script().replace(
        "The evidence is thin, so uncertainty remains.",
        "Uncertainty remains because the evidence is thin.",
    );
    fs::write(&executable_path, passing_local_model_runner_script())?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    fs::write(&executable_path, current_script)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--compare-baseline")
        .arg("--changed-case-report-path")
        .arg(&changed_case_report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout_report: Value = serde_json::from_slice(&output)?;
    assert_eq!(
        stdout_report["changed_case_report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    let report: Value = serde_json::from_str(&fs::read_to_string(&changed_case_report_path)?)?;
    assert_eq!(
        report["format"].as_str(),
        Some("continuitydb.local_model.changed_cases")
    );
    assert_eq!(report["format_version"].as_u64(), Some(1));
    assert_eq!(
        report["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        report["candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        report["candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        report["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(report["comparison"]["changed_cases"].as_u64(), Some(9));
    assert_eq!(
        report["comparison"]["outcome_changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["comparison"]["failure_count_changed_cases"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["comparison"]["response_changed_cases"].as_u64(),
        Some(9)
    );
    assert_eq!(
        report["comparison"]["changed_case_summaries"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_file(changed_case_report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_changed_case_report_path_requires_comparison(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-changed-report-requirement");
    let changed_case_report_path =
        temp_store_path("continuitydb-cli-local-model-changed-report-without-comparison")
            .with_extension("json");

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--changed-case-report-path")
        .arg(&changed_case_report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/bin/echo")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains(
            "--changed-case-report-path requires --compare-baseline or --fail-on-regression",
        ));

    assert!(!changed_case_report_path.exists());
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_artifact_dir_writes_changed_case_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-changed-bundle-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-changed-bundle-baseline");
    let contract_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-changed-bundle-contracts-{}",
        std::process::id()
    ));
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-changed-bundle-dir-{}",
        std::process::id()
    ));
    if contract_dir.exists() {
        fs::remove_dir_all(&contract_dir)?;
    }
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    let current_script = passing_local_model_runner_script().replace(
        "The evidence is thin, so uncertainty remains.",
        "Uncertainty remains because the evidence is thin.",
    );
    fs::write(&executable_path, passing_local_model_runner_script())?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--contract-dir")
        .arg(&contract_dir)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    fs::write(&executable_path, current_script)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--compare-baseline")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--contract-dir")
        .arg(&contract_dir)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout_report: Value = serde_json::from_slice(&output)?;
    let changed_case_report_path = artifact_dir.join("changed-cases.json");
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    assert_eq!(
        stdout_report["changed_case_report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        stdout_report["changed_case_report"]["report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert!(stdout_report["changed_case_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(stdout_report["changed_case_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    let changed_case_report: Value =
        serde_json::from_str(&fs::read_to_string(&changed_case_report_path)?)?;
    assert_eq!(
        changed_case_report["format"].as_str(),
        Some("continuitydb.local_model.changed_cases")
    );
    assert_eq!(
        changed_case_report["comparison"]["changed_cases"].as_u64(),
        Some(9)
    );
    let bundle_manifest: Value = serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    assert_eq!(
        bundle_manifest["changed_case_report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["changed_case_report"]["report_path"].as_str(),
        Some(changed_case_report_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["changed_case_report"]["report_fingerprint"],
        stdout_report["changed_case_report"]["report_fingerprint"]
    );
    assert_eq!(
        bundle_manifest["changed_case_report"]["report_bytes"],
        stdout_report["changed_case_report"]["report_bytes"]
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(contract_dir)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_failure_report_path_records_passing_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-passing-gate-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-passing-gate-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-passing-gate-report");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--fail-on-failed-cases")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--failure-report-path")
        .arg(&report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let baseline_text = fs::read_to_string(&baseline_path)?;
    let records: Vec<Value> = baseline_text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    assert_eq!(json["passed"].as_bool(), Some(true));
    assert_eq!(json["failed_cases"].as_u64(), Some(0));
    assert_eq!(records.len(), 1);
    assert!(!report_path.exists());

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_stability_dry_run_outputs_preflight_without_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-stability-dry-run-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--stability-trials")
        .arg("3")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["stability_preflight"]["trials"].as_u64(), Some(3));
    assert_eq!(
        json["stability_preflight"]["will_execute"].as_bool(),
        Some(false)
    );
    assert!(json.get("stability").is_none());
    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_stability_run_outputs_report() -> Result<(), Box<dyn std::error::Error>>
{
    let executable_path = temp_store_path("continuitydb-cli-local-model-stability-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-stability-baseline");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--stability-trials")
        .arg("2")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let baseline_text = fs::read_to_string(&baseline_path)?;
    let records: Vec<Value> = baseline_text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    assert_eq!(json["passed"].as_bool(), Some(true));
    assert_eq!(json["stability"]["trials"].as_u64(), Some(2));
    assert_eq!(json["stability"]["stable"].as_bool(), Some(true));
    assert_eq!(
        json["stability"]["case_reports"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        json["stability"]["case_reports"][0]["name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert_eq!(
        json["stability"]["case_reports"][0]["changed_trials"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        json["stability"]["case_reports"][0]["proposal_fingerprints"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(records.len(), 1);

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_stability_rejects_zero_trials_without_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-stability-zero-baseline");

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--stability-trials")
        .arg("0")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("--stability-trials must be greater than zero"));

    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_fail_on_unstable_requires_stability_trials(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-fail-on-unstable-missing-trials-baseline");

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--fail-on-unstable")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("--fail-on-unstable requires --stability-trials"));

    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_fail_on_unstable_dry_run_reports_gate(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-fail-on-unstable-dry-run");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--stability-trials")
        .arg("2")
        .arg("--fail-on-unstable")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["stability_preflight"]["trials"].as_u64(), Some(2));
    assert_eq!(
        json["stability_preflight"]["fail_on_unstable"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["stability_preflight"]["will_execute"].as_bool(),
        Some(false)
    );
    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_fail_on_unstable_rejects_drift_without_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-unstable-runner");
    let counter_path = temp_store_path("continuitydb-cli-local-model-unstable-counter");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-unstable-baseline");
    let script = format!(
        r#"#!/usr/bin/env sh
cat >/dev/null
counter_path='{counter_path}'
count=0
if test -f "$counter_path"; then
    count=$(cat "$counter_path")
fi
count=$((count + 1))
printf '%s' "$count" >"$counter_path"
if test "$count" -le 5; then
    thin_rationale='The evidence is thin, so uncertainty remains.'
else
    thin_rationale='The evidence is thin, so uncertainty still remains.'
fi
printf '%s\n' '{{"proposals":[{{"action":{{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."}},"rationale":"'"$thin_rationale"'","citations":["continuitydb://evaluation/thin-evidence"]}},{{"action":{{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"}},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]}},{{"action":{{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."}},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]}},{{"action":{{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"}},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]}},{{"action":{{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."}},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}}]}}'
"#,
        counter_path = counter_path.display()
    );
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--stability-trials")
        .arg("2")
        .arg("--fail-on-unstable")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("local model benchmark stability check failed"));

    assert!(!baseline_path.exists());

    fs::remove_file(executable_path)?;
    fs::remove_file(counter_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_failure_report_path_records_instability_gate(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-unstable-report-runner");
    let counter_path = temp_store_path("continuitydb-cli-local-model-unstable-report-counter");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-unstable-report-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-unstable-report");
    let script = format!(
        r#"#!/usr/bin/env sh
cat >/dev/null
counter_path='{counter_path}'
count=0
if test -f "$counter_path"; then
    count=$(cat "$counter_path")
fi
count=$((count + 1))
printf '%s' "$count" >"$counter_path"
if test "$count" -le 5; then
    thin_rationale='The evidence is thin, so uncertainty remains.'
else
    thin_rationale='The evidence is thin, so uncertainty still remains.'
fi
printf '%s\n' '{{"proposals":[{{"action":{{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."}},"rationale":"'"$thin_rationale"'","citations":["continuitydb://evaluation/thin-evidence"]}},{{"action":{{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"}},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]}},{{"action":{{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."}},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]}},{{"action":{{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"}},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]}},{{"action":{{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."}},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}}]}}'
"#,
        counter_path = counter_path.display()
    );
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--stability-trials")
        .arg("2")
        .arg("--fail-on-unstable")
        .arg("--failure-report-path")
        .arg(&report_path)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("local model benchmark stability check failed"));

    assert!(!baseline_path.exists());
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    assert_eq!(report["stability"]["stable"].as_bool(), Some(false));
    assert_eq!(report["stability"]["trials"].as_u64(), Some(2));
    assert!(report["stability"]["case_reports"]
        .as_array()
        .is_some_and(|cases| cases
            .iter()
            .any(|case| case["stable"].as_bool() == Some(false))));
    assert!(report["bundle_manifest"].is_null());

    fs::remove_file(executable_path)?;
    fs::remove_file(counter_path)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_artifact_dir_writes_instability_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-unstable-bundle-runner");
    let counter_path = temp_store_path("continuitydb-cli-local-model-unstable-bundle-counter");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-unstable-bundle-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-unstable-bundle-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    let script = format!(
        r#"#!/usr/bin/env sh
cat >/dev/null
counter_path='{counter_path}'
count=0
if test -f "$counter_path"; then
    count=$(cat "$counter_path")
fi
count=$((count + 1))
printf '%s' "$count" >"$counter_path"
if test "$count" -le 5; then
    thin_rationale='The evidence is thin, so uncertainty remains.'
else
    thin_rationale='The evidence is thin, so uncertainty still remains.'
fi
printf '%s\n' '{{"proposals":[{{"action":{{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."}},"rationale":"'"$thin_rationale"'","citations":["continuitydb://evaluation/thin-evidence"]}},{{"action":{{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"}},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]}},{{"action":{{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."}},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]}},{{"action":{{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"}},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]}},{{"action":{{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."}},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}}]}}'
"#,
        counter_path = counter_path.display()
    );
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--stability-trials")
        .arg("2")
        .arg("--fail-on-unstable")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("local model benchmark stability check failed"));

    assert!(!baseline_path.exists());
    let report_path = artifact_dir.join("benchmark-report.json");
    let bundle_manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest: Value = serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;

    assert_eq!(report["stability"]["stable"].as_bool(), Some(false));
    assert_eq!(report["stability"]["trials"].as_u64(), Some(2));
    assert!(report["stability"]["case_reports"]
        .as_array()
        .is_some_and(|cases| cases
            .iter()
            .any(|case| case["stable"].as_bool() == Some(false))));
    assert_eq!(
        report["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["benchmark_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        bundle_manifest["response_artifacts"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert!(
        bundle_manifest["response_artifact_manifest"]["manifest_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("responses/local-model-responses.manifest.json"))
    );

    fs::remove_file(executable_path)?;
    fs::remove_file(counter_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-dry-run-missing-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--compare-baseline")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("llama-cli")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["baseline_preflight"]["compared"].as_bool(), Some(true));
    assert_eq!(
        json["baseline_preflight"]["compatible_baseline_found"].as_bool(),
        Some(false)
    );
    assert!(json["baseline_preflight"]["previous_recorded_at"].is_null());
    assert!(json["baseline_preflight"]["previous_schema_bytes"].is_null());
    assert!(json["baseline_preflight"]["previous_grammar_bytes"].is_null());
    assert!(json["baseline_preflight"]["previous_response_schema_version"].is_null());
    assert!(json["baseline_preflight"]["previous_evaluation_suite_fingerprint"].is_null());
    assert!(json["baseline_preflight"]["previous_schema_fingerprint"].is_null());
    assert!(json["baseline_preflight"]["previous_grammar_fingerprint"].is_null());
    assert!(json["baseline_preflight"]["previous_prompt_fingerprint"].is_null());
    assert!(json["baseline_preflight"]["previous_candidate_selection"].is_null());
    assert!(json["baseline_preflight"]["previous_runtime"].is_null());
    assert!(!baseline_path.exists());

    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_uses_candidate_defaults(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-defaults-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--threads")
        .arg("--arg")
        .arg("2")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["candidate_selection"]["source"].as_str(),
        Some("explicit")
    );
    assert_eq!(
        json["candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["runtime"]["arguments"][0].as_str(), Some("--model"));
    assert_eq!(
        json["runtime"]["arguments"][1].as_str(),
        Some("/models/qwen.gguf")
    );
    assert_eq!(json["runtime"]["arguments"][2].as_str(), Some("--ctx-size"));
    assert_eq!(json["runtime"]["arguments"][3].as_str(), Some("32768"));
    assert_eq!(json["runtime"]["arguments"][4].as_str(), Some("--temp"));
    assert_eq!(json["runtime"]["arguments"][5].as_str(), Some("0"));
    assert_eq!(json["runtime"]["arguments"][6].as_str(), Some("--prompt"));
    assert_eq!(json["runtime"]["arguments"][7].as_str(), Some("-"));
    assert_eq!(json["runtime"]["arguments"][8].as_str(), Some("--threads"));
    assert_eq!(json["runtime"]["arguments"][9].as_str(), Some("2"));
    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_uses_default_ci_candidate_without_explicit_candidate(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-default-ci-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidate_selection"]["source"].as_str(),
        Some("default_ci_candidate")
    );
    assert_eq!(
        json["candidate_selection"]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_includes_grammar_path(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-grammar-baseline");
    let grammar_path = temp_store_path("continuitydb-cli-local-model-response-grammar");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--grammar-path")
        .arg(&grammar_path)
        .arg("--arg")
        .arg("--threads")
        .arg("--arg")
        .arg("2")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["runtime"]["arguments"][6].as_str(), Some("--prompt"));
    assert_eq!(json["runtime"]["arguments"][7].as_str(), Some("-"));
    assert_eq!(
        json["runtime"]["arguments"][8].as_str(),
        Some("--grammar-file")
    );
    assert_eq!(
        json["runtime"]["arguments"][9].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert_eq!(json["runtime"]["arguments"][10].as_str(), Some("--threads"));
    assert_eq!(json["runtime"]["arguments"][11].as_str(), Some("2"));
    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_enforces_candidate_grammar_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-enforced-baseline");

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--enforce-candidate-requirements")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("missing --grammar-path"));

    assert!(!baseline_path.exists());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-contract-dir-baseline");
    let contract_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-contract-dir-{}",
        std::process::id()
    ));
    if contract_dir.exists() {
        fs::remove_dir_all(&contract_dir)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--enforce-candidate-requirements")
        .arg("--contract-dir")
        .arg(&contract_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let schema_path = contract_dir.join("local-model-response.schema.json");
    let grammar_path = contract_dir.join("local-model-response.gbnf");
    let context_schema_path =
        contract_dir.join("local-model-context-compiler-response.schema.json");
    let context_grammar_path = contract_dir.join("local-model-context-compiler-response.gbnf");

    assert!(schema_path.exists());
    assert!(grammar_path.exists());
    assert!(context_schema_path.exists());
    assert!(context_grammar_path.exists());
    assert_eq!(
        json["contract_artifacts"]["schema_path"].as_str(),
        Some(schema_path.display().to_string().as_str())
    );
    assert_eq!(
        json["contract_artifacts"]["grammar_path"].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert_eq!(
        json["contract_artifacts"]["context_compiler_schema_path"].as_str(),
        Some(context_schema_path.display().to_string().as_str())
    );
    assert_eq!(
        json["contract_artifacts"]["context_compiler_schema_version"].as_u64(),
        Some(2)
    );
    assert_eq!(
        json["contract_artifacts"]["context_compiler_grammar_path"].as_str(),
        Some(context_grammar_path.display().to_string().as_str())
    );
    assert!(json["contract_artifacts"]["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["contract_artifacts"]["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["contract_artifacts"]["schema_bytes"].as_u64(),
        Some(fs::read_to_string(&schema_path)?.len() as u64)
    );
    assert_eq!(
        json["contract_artifacts"]["grammar_bytes"].as_u64(),
        Some(fs::read_to_string(&grammar_path)?.len() as u64)
    );
    assert_eq!(
        json["contract_artifacts"]["context_compiler_schema_bytes"].as_u64(),
        Some(fs::read_to_string(&context_schema_path)?.len() as u64)
    );
    assert_eq!(
        json["contract_artifacts"]["context_compiler_grammar_bytes"].as_u64(),
        Some(fs::read_to_string(&context_grammar_path)?.len() as u64)
    );
    assert_eq!(
        json["runtime"]["arguments"][8].as_str(),
        Some("--grammar-file")
    );
    assert_eq!(
        json["runtime"]["arguments"][9].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert!(!baseline_path.exists());

    fs::remove_dir_all(contract_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_prompt_dir_writes_prompt_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-prompt-dir-baseline");
    let prompt_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-prompt-dir-{}",
        std::process::id()
    ));
    if prompt_dir.exists() {
        fs::remove_dir_all(&prompt_dir)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--prompt-dir")
        .arg(&prompt_dir)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let artifacts = json["prompt_artifacts"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing prompt artifacts"))?;

    assert_eq!(artifacts.len(), 9);
    assert_eq!(
        artifacts[0]["case_name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert!(artifacts[0]["prompt_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(artifacts[0]["prompt_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    let first_prompt_path = artifacts[0]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let first_prompt = fs::read_to_string(first_prompt_path)?;
    assert!(first_prompt.contains("You are the ContinuityDB database Steward."));
    assert!(first_prompt.contains("Assess whether thin evidence needs verification."));
    assert!(first_prompt.contains("continuitydb://evaluation/thin-evidence"));
    assert!(first_prompt.contains("One weak source mentions the claim without corroboration."));
    assert_eq!(
        artifacts[2]["case_name"].as_str(),
        Some("supersession classification")
    );
    let third_prompt_path = artifacts[2]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let third_prompt = fs::read_to_string(third_prompt_path)?;
    assert!(third_prompt
        .contains("Classify whether newer release evidence supersedes the older status."));
    assert!(third_prompt.contains("continuitydb://evaluation/supersession-evidence"));
    assert_eq!(
        artifacts[3]["case_name"].as_str(),
        Some("unsupported claim boundary")
    );
    let fourth_prompt_path = artifacts[3]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let fourth_prompt = fs::read_to_string(fourth_prompt_path)?;
    assert!(fourth_prompt
        .contains("Check whether release evidence supports a shipped deployment claim."));
    assert!(fourth_prompt.contains("continuitydb://evaluation/unsupported-release-claim"));
    assert_eq!(
        artifacts[4]["case_name"].as_str(),
        Some("confidence adjustment")
    );
    let fifth_prompt_path = artifacts[4]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let fifth_prompt = fs::read_to_string(fifth_prompt_path)?;
    assert!(fifth_prompt.contains("Adjust confidence for stale deployment status evidence."));
    assert!(fifth_prompt.contains("continuitydb://evaluation/confidence-evidence"));
    assert_eq!(
        artifacts[5]["case_name"].as_str(),
        Some("targeted verification request")
    );
    let sixth_prompt_path = artifacts[5]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let sixth_prompt = fs::read_to_string(sixth_prompt_path)?;
    assert!(sixth_prompt.contains("Request verification for a stale high-impact frontier cell."));
    assert!(sixth_prompt.contains("continuitydb://evaluation/targeted-verification-evidence"));
    assert_eq!(
        artifacts[6]["case_name"].as_str(),
        Some("new evidence draft creation")
    );
    let seventh_prompt_path = artifacts[6]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let seventh_prompt = fs::read_to_string(seventh_prompt_path)?;
    assert!(seventh_prompt.contains("Draft a StateCell from new benchmark evidence."));
    assert!(seventh_prompt.contains("continuitydb://evaluation/new-benchmark-evidence"));
    assert_eq!(
        artifacts[7]["case_name"].as_str(),
        Some("multi-source citation preservation")
    );
    let eighth_prompt_path = artifacts[7]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let eighth_prompt = fs::read_to_string(eighth_prompt_path)?;
    assert!(eighth_prompt
        .contains("Decide whether a release-status change should stay on the active frontier."));
    assert!(eighth_prompt.contains("continuitydb://evaluation/release-build-source"));
    assert!(eighth_prompt.contains("continuitydb://evaluation/release-incident-source"));
    assert_eq!(
        artifacts[8]["case_name"].as_str(),
        Some("policy rejection avoidance")
    );
    let ninth_prompt_path = artifacts[8]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let ninth_prompt = fs::read_to_string(ninth_prompt_path)?;
    assert!(ninth_prompt.contains(
        "Handle invalid answerability-label evidence without emitting an invalid label."
    ));
    assert!(ninth_prompt.contains("continuitydb://evaluation/invalid-answerability-label"));
    assert!(!baseline_path.exists());

    fs::remove_dir_all(prompt_dir)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_acceptance_criteria_outputs_required_manifest(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-acceptance-criteria")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_acceptance_criteria")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["required_criteria_count"].as_u64(), Some(7));
    assert_eq!(
        json["criteria"],
        serde_json::json!([
            "valid_json_schema_conformance",
            "conflict_versus_supersession_classification",
            "evidence_citation_preservation",
            "unsupported_claim_avoidance",
            "stable_low_temperature_output",
            "explicit_uncertainty_for_insufficient_evidence",
            "deterministic_policy_rejection_avoidance"
        ])
    );
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_acceptance_criteria_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-local-model-acceptance-criteria-report")
        .with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-acceptance-criteria-validation")
            .with_extension("json");

    let report_output = Command::cargo_bin("continuitydb")?
        .arg("local-model-acceptance-criteria")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, report_output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-acceptance-criteria")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_acceptance_criteria_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(json["required_criteria_count"].as_u64(), Some(7));
    assert_eq!(
        json["criteria"],
        serde_json::json!([
            "valid_json_schema_conformance",
            "conflict_versus_supersession_classification",
            "evidence_citation_preservation",
            "unsupported_claim_avoidance",
            "stable_low_temperature_output",
            "explicit_uncertainty_for_insufficient_evidence",
            "deterministic_policy_rejection_avoidance"
        ])
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));
    Ok(())
}

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
    assert_eq!(json["total_cases"].as_u64(), Some(9));
    assert_eq!(
        json["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["acceptance_coverage"]["covered"]
            .as_array()
            .map(Vec::len),
        Some(7)
    );
    assert_eq!(
        json["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(json["evaluation_suite_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
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
    assert!(json["cases"][0]["acceptance_criteria"]
        .as_array()
        .is_some_and(
            |criteria| criteria.iter().any(|criterion| criterion.as_str()
                == Some("explicit_uncertainty_for_insufficient_evidence"))
        ));
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
    assert!(json["cases"][1]["acceptance_criteria"]
        .as_array()
        .is_some_and(|criteria| criteria
            .iter()
            .any(|criterion| criterion.as_str()
                == Some("conflict_versus_supersession_classification"))));
    assert_eq!(
        json["cases"][1]["forbidden_rationale_terms"][0].as_str(),
        Some("verified in production")
    );
    assert_eq!(
        json["cases"][2]["name"].as_str(),
        Some("supersession classification")
    );
    assert_eq!(
        json["cases"][2]["task"].as_str(),
        Some("Classify whether newer release evidence supersedes the older status.")
    );
    assert_eq!(
        json["cases"][2]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/supersession-evidence")
    );
    assert_eq!(
        json["cases"][2]["expected_actions"][0]["type"].as_str(),
        Some("link_revision")
    );
    assert_eq!(
        json["cases"][2]["expected_actions"][0]["kind"].as_str(),
        Some("supersedes")
    );
    assert!(json["cases"][2]["acceptance_criteria"]
        .as_array()
        .is_some_and(|criteria| criteria
            .iter()
            .any(|criterion| criterion.as_str()
                == Some("conflict_versus_supersession_classification"))));
    assert_eq!(
        json["cases"][2]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/supersession-evidence")
    );
    assert_eq!(
        json["cases"][2]["required_rationale_terms"][0].as_str(),
        Some("supersedes")
    );
    assert_eq!(
        json["cases"][2]["forbidden_rationale_terms"][0].as_str(),
        Some("conflicts with")
    );
    assert_eq!(
        json["cases"][3]["name"].as_str(),
        Some("unsupported claim boundary")
    );
    assert_eq!(
        json["cases"][3]["task"].as_str(),
        Some("Check whether release evidence supports a shipped deployment claim.")
    );
    assert_eq!(
        json["cases"][3]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/unsupported-release-claim")
    );
    assert_eq!(
        json["cases"][3]["expected_actions"][0]["type"].as_str(),
        Some("request_verification")
    );
    assert!(json["cases"][3]["acceptance_criteria"]
        .as_array()
        .is_some_and(|criteria| criteria
            .iter()
            .any(|criterion| criterion.as_str() == Some("unsupported_claim_avoidance"))));
    assert_eq!(
        json["cases"][3]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/unsupported-release-claim")
    );
    assert_eq!(
        json["cases"][3]["required_rationale_terms"][0].as_str(),
        Some("unsupported")
    );
    assert_eq!(
        json["cases"][3]["forbidden_rationale_terms"][0].as_str(),
        Some("deployed to all customers")
    );
    assert_eq!(
        json["cases"][4]["name"].as_str(),
        Some("confidence adjustment")
    );
    assert_eq!(
        json["cases"][4]["task"].as_str(),
        Some("Adjust confidence for stale deployment status evidence.")
    );
    assert_eq!(
        json["cases"][4]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/confidence-evidence")
    );
    assert_eq!(
        json["cases"][4]["expected_actions"][0]["type"].as_str(),
        Some("adjust_confidence")
    );
    let proposed_confidence = json["cases"][4]["expected_actions"][0]["proposed_confidence"]
        .as_f64()
        .ok_or_else(|| std::io::Error::other("missing proposed confidence"))?;
    assert!((proposed_confidence - 0.42).abs() < 0.000001);
    assert_eq!(
        json["cases"][4]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/confidence-evidence")
    );
    assert_eq!(
        json["cases"][4]["required_rationale_terms"][0].as_str(),
        Some("confidence")
    );
    assert_eq!(
        json["cases"][4]["forbidden_rationale_terms"][0].as_str(),
        Some("fully trusted")
    );
    assert_eq!(
        json["cases"][5]["name"].as_str(),
        Some("targeted verification request")
    );
    assert_eq!(
        json["cases"][5]["task"].as_str(),
        Some("Request verification for a stale high-impact frontier cell.")
    );
    assert_eq!(
        json["cases"][5]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/targeted-verification-evidence")
    );
    assert_eq!(
        json["cases"][5]["expected_actions"][0]["type"].as_str(),
        Some("request_verification")
    );
    assert_eq!(
        json["cases"][5]["expected_actions"][0]["cell_id"].as_str(),
        Some("00000000-0000-0000-0000-000000000007")
    );
    assert_eq!(
        json["cases"][5]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/targeted-verification-evidence")
    );
    assert_eq!(
        json["cases"][5]["required_rationale_terms"][0].as_str(),
        Some("refresh")
    );
    assert_eq!(
        json["cases"][5]["forbidden_rationale_terms"][0].as_str(),
        Some("no target")
    );
    assert_eq!(
        json["cases"][6]["name"].as_str(),
        Some("new evidence draft creation")
    );
    assert_eq!(
        json["cases"][6]["task"].as_str(),
        Some("Draft a StateCell from new benchmark evidence.")
    );
    assert_eq!(
        json["cases"][6]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/new-benchmark-evidence")
    );
    assert_eq!(
        json["cases"][6]["expected_actions"][0]["type"].as_str(),
        Some("create_cell_draft")
    );
    assert_eq!(
        json["cases"][6]["expected_actions"][0]["anchors"][0].as_str(),
        Some("project:continuitydb:benchmark-result")
    );
    assert_eq!(
        json["cases"][6]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/new-benchmark-evidence")
    );
    assert_eq!(
        json["cases"][6]["required_rationale_terms"][0].as_str(),
        Some("draft")
    );
    assert_eq!(
        json["cases"][6]["forbidden_rationale_terms"][0].as_str(),
        Some("committed")
    );
    assert_eq!(
        json["cases"][7]["name"].as_str(),
        Some("multi-source citation preservation")
    );
    assert_eq!(
        json["cases"][7]["task"].as_str(),
        Some("Decide whether a release-status change should stay on the active frontier.")
    );
    assert_eq!(
        json["cases"][7]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/release-build-source")
    );
    assert_eq!(
        json["cases"][7]["evidence"][1]["locator"].as_str(),
        Some("continuitydb://evaluation/release-incident-source")
    );
    assert_eq!(
        json["cases"][7]["expected_actions"][0]["type"].as_str(),
        Some("mark_frontier")
    );
    assert_eq!(
        json["cases"][7]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/release-build-source")
    );
    assert_eq!(
        json["cases"][7]["required_citations"][1].as_str(),
        Some("continuitydb://evaluation/release-incident-source")
    );
    assert_eq!(
        json["cases"][7]["required_rationale_terms"][0].as_str(),
        Some("frontier")
    );
    assert_eq!(
        json["cases"][8]["name"].as_str(),
        Some("policy rejection avoidance")
    );
    assert_eq!(
        json["cases"][8]["task"].as_str(),
        Some("Handle invalid answerability-label evidence without emitting an invalid label.")
    );
    assert_eq!(
        json["cases"][8]["evidence"][0]["locator"].as_str(),
        Some("continuitydb://evaluation/invalid-answerability-label")
    );
    assert_eq!(
        json["cases"][8]["expected_actions"][0]["type"].as_str(),
        Some("request_verification")
    );
    assert!(json["cases"][8]["acceptance_criteria"]
        .as_array()
        .is_some_and(|criteria| criteria.iter().any(
            |criterion| criterion.as_str() == Some("deterministic_policy_rejection_avoidance")
        )));
    assert_eq!(
        json["cases"][8]["required_citations"][0].as_str(),
        Some("continuitydb://evaluation/invalid-answerability-label")
    );
    assert_eq!(
        json["cases"][8]["required_rationale_terms"][0].as_str(),
        Some("invalid")
    );
    assert_eq!(
        json["cases"][8]["forbidden_rationale_terms"][0].as_str(),
        Some("label applied")
    );
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_candidates_outputs_fixed_registry() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-candidates")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["default_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["default_quality_gate_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["default_ci_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["quality_gate_candidates"],
        serde_json::json!(["Qwen/Qwen2.5-0.5B-Instruct", "Qwen/Qwen3-0.6B"])
    );
    assert_eq!(
        json["ci_candidates"],
        serde_json::json!(["Qwen/Qwen2.5-0.5B-Instruct", "Qwen/Qwen3-0.6B"])
    );
    assert_eq!(json["total_candidates"].as_u64(), Some(4));
    assert_eq!(
        json["candidates"][0]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["role"].as_str(),
        Some("default-feasibility")
    );
    assert_eq!(
        json["candidates"][0]["evaluation_priority"].as_u64(),
        Some(1)
    );
    assert_eq!(
        json["candidates"][0]["evaluation_tier"].as_str(),
        Some("default")
    );
    assert_eq!(json["candidates"][0]["ci_suitable"].as_bool(), Some(true));
    assert_eq!(
        json["candidates"][0]["quality_gate_eligible"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["candidates"][0]["default_quality_gate_candidate"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["candidates"][0]["evaluation_use"].as_str(),
        Some("first real Steward proposal experiments")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runtime"].as_str(),
        Some("llama.cpp")
    );
    assert_eq!(
        json["candidates"][0]["compatible_runtimes"],
        serde_json::json!(["llama.cpp", "mistral.rs"])
    );
    assert_eq!(
        json["candidates"][0]["artifact_format"].as_str(),
        Some("GGUF")
    );
    assert_eq!(
        json["candidates"][0]["license"].as_str(),
        Some("Apache-2.0")
    );
    assert_eq!(
        json["candidates"][0]["parameter_count_millions"].as_u64(),
        Some(490)
    );
    assert_eq!(
        json["candidates"][0]["context_window_tokens"].as_u64(),
        Some(32_768)
    );
    assert_eq!(
        json["candidates"][0]["model_card_url"].as_str(),
        Some("https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["recommended_temperature"].as_f64(),
        Some(0.0)
    );
    assert_eq!(
        json["candidates"][0]["requires_grammar"].as_bool(),
        Some(true)
    );
    assert!(json["candidates"][0]["notes"]
        .as_str()
        .is_some_and(|notes| !notes.is_empty()));
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments"][0].as_str(),
        Some("--model")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments"][1].as_str(),
        Some("<model.gguf>")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments"][2].as_str(),
        Some("--ctx-size")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments"][3].as_str(),
        Some("32768")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments"][5].as_str(),
        Some("0")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments_with_grammar"][6].as_str(),
        Some("--grammar-file")
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments_with_grammar"][7].as_str(),
        Some("<steward-response.gbnf>")
    );
    assert_eq!(
        json["candidates"][0]["recommended_mistral_runner_arguments"][0].as_str(),
        Some("--model")
    );
    assert_eq!(
        json["candidates"][0]["recommended_mistral_runner_arguments"][2].as_str(),
        Some("--max-seq-len")
    );
    assert_eq!(
        json["candidates"][0]["recommended_mistral_runner_arguments"][3].as_str(),
        Some("32768")
    );
    assert_eq!(
        json["candidates"][0]["recommended_mistral_runner_arguments"][6].as_str(),
        Some("--json-output")
    );
    assert!(json["candidates"].as_array().is_some_and(|candidates| {
        candidates.iter().any(|candidate| {
            candidate["model_id"].as_str() == Some("HuggingFaceTB/SmolLM2-360M-Instruct")
                && candidate["role"].as_str() == Some("ultra-small-experimental")
                && candidate["evaluation_use"].as_str()
                    == Some("measure the lower bound for constrained proposal quality")
        })
    }));
    assert!(json["candidates"].as_array().is_some_and(|candidates| {
        candidates.iter().any(|candidate| {
            candidate["role"].as_str() == Some("smoke-test-only")
                && candidate["evaluation_priority"].as_u64() == Some(4)
                && candidate["evaluation_tier"].as_str() == Some("smoke-test")
                && candidate["ci_suitable"].as_bool() == Some(false)
                && candidate["quality_gate_eligible"].as_bool() == Some(false)
                && candidate["default_quality_gate_candidate"].as_bool() == Some(false)
        })
    }));
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_candidates_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-local-model-candidates-report").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-candidates-validation")
            .with_extension("json");

    let report_output = Command::cargo_bin("continuitydb")?
        .arg("local-model-candidates")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, report_output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-candidates")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_candidates_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(json["total_candidates"].as_u64(), Some(4));
    assert_eq!(json["quality_gate_candidate_count"].as_u64(), Some(2));
    assert_eq!(json["ci_candidate_count"].as_u64(), Some(2));
    assert_eq!(
        json["default_quality_gate_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_plan_outputs_ordered_benchmark_plan(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-plan")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_plan")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("local-model-quality-gate-plan")
    );
    assert_eq!(
        json["execution_summary"],
        serde_json::json!({
            "candidate_count": 2,
            "steps_per_candidate": 2,
            "total_steps": 4,
            "sequence": ["benchmark", "validate_bundle"]
        })
    );
    assert_eq!(json["candidate_set"].as_str(), Some("quality_gate"));
    assert_eq!(
        json["default_quality_gate_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["total_candidates"].as_u64(), Some(2));
    assert_eq!(
        json["required_inputs"],
        serde_json::json!([
            {
                "placeholder": "<runner>",
                "description": "Executable path for the local model runner used by benchmark-local-model --executable."
            },
            {
                "placeholder": "<model.gguf>",
                "description": "Candidate model artifact path passed to benchmark-local-model --model-path."
            },
            {
                "placeholder": "<baselines.jsonl>",
                "description": "Local model baseline store path passed to benchmark-local-model --baseline-path."
            },
            {
                "placeholder": "<artifacts>",
                "description": "Artifact root directory used to isolate benchmark and validation outputs per candidate."
            },
            {
                "placeholder": "<steward-response.gbnf>",
                "description": "Grammar file path supplied to grammar-constrained runners."
            }
        ])
    );
    assert_eq!(
        json["environment_bindings"],
        serde_json::json!([
            {
                "placeholder": "<runner>",
                "env": "CONTINUITYDB_LOCAL_MODEL_RUNNER"
            },
            {
                "placeholder": "<model.gguf>",
                "env": "CONTINUITYDB_LOCAL_MODEL_PATH"
            },
            {
                "placeholder": "<baselines.jsonl>",
                "env": "CONTINUITYDB_LOCAL_MODEL_BASELINES"
            },
            {
                "placeholder": "<artifacts>",
                "env": "CONTINUITYDB_LOCAL_MODEL_ARTIFACTS"
            },
            {
                "placeholder": "<steward-response.gbnf>",
                "env": "CONTINUITYDB_LOCAL_MODEL_GRAMMAR"
            }
        ])
    );
    assert_eq!(
        json["substitution_contract"],
        serde_json::json!({
            "mode": "replace_all_placeholders_before_execution",
            "unresolved_placeholder_policy": "fail_before_launch",
            "placeholders": [
                "<runner>",
                "<model.gguf>",
                "<baselines.jsonl>",
                "<artifacts>",
                "<steward-response.gbnf>"
            ]
        })
    );
    assert_eq!(
        json["candidates"][0]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["evaluation_priority"].as_u64(),
        Some(1)
    );
    assert_eq!(
        json["candidates"][0]["job_id"].as_str(),
        Some("local-model-qwen-qwen2-5-0-5b-instruct")
    );
    assert_eq!(
        json["candidates"][0]["job_name"].as_str(),
        Some("Local model quality gate: Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["artifact_upload_name"].as_str(),
        Some("continuitydb-local-model-qwen-qwen2-5-0-5b-instruct")
    );
    assert_eq!(
        json["candidates"][0]["artifact_retention"],
        serde_json::json!({
            "class": "local_model_quality_gate_bundle",
            "days": 30,
            "upload_path": "<artifacts>/qwen-qwen2-5-0-5b-instruct"
        })
    );
    assert_eq!(
        json["candidates"][0]["artifact_download"],
        serde_json::json!({
            "artifact_name": "continuitydb-local-model-qwen-qwen2-5-0-5b-instruct",
            "download_path": "<artifacts>/downloaded/qwen-qwen2-5-0-5b-instruct",
            "restore_path": "<artifacts>/qwen-qwen2-5-0-5b-instruct"
        })
    );
    assert_eq!(
        json["candidates"][0]["result_artifacts"],
        serde_json::json!({
            "benchmark_report": "<artifacts>/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
            "benchmark_failure_report": "<artifacts>/qwen-qwen2-5-0-5b-instruct/failure-report.json",
            "validation_report": "<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-report.json",
            "validation_failure_report": "<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-failure-report.json",
            "bundle_manifest": "<artifacts>/qwen-qwen2-5-0-5b-instruct/manifest.json"
        })
    );
    assert_eq!(
        json["candidates"][0]["benchmark_command"],
        serde_json::json!([
            "continuitydb",
            "benchmark-local-model",
            "--candidate",
            "Qwen/Qwen2.5-0.5B-Instruct",
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
            "<artifacts>/qwen-qwen2-5-0-5b-instruct",
            "--report-path",
            "<artifacts>/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
            "--failure-report-path",
            "<artifacts>/qwen-qwen2-5-0-5b-instruct/failure-report.json"
        ])
    );
    assert_eq!(
        json["candidates"][0]["artifact_dir"].as_str(),
        Some("<artifacts>/qwen-qwen2-5-0-5b-instruct")
    );
    assert_eq!(
        json["candidates"][0]["report_path"].as_str(),
        Some("<artifacts>/qwen-qwen2-5-0-5b-instruct/benchmark-report.json")
    );
    assert_eq!(
        json["candidates"][0]["failure_report_path"].as_str(),
        Some("<artifacts>/qwen-qwen2-5-0-5b-instruct/failure-report.json")
    );
    assert_eq!(
        json["candidates"][0]["validation_report_path"].as_str(),
        Some("<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-report.json")
    );
    assert_eq!(
        json["candidates"][0]["validation_command"],
        serde_json::json!([
            "continuitydb",
            "validate-local-model-bundle",
            "--artifact-dir",
            "<artifacts>/qwen-qwen2-5-0-5b-instruct",
            "--report-path",
            "<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-report.json",
            "--failure-report-path",
            "<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-failure-report.json"
        ])
    );
    assert_eq!(
        json["candidates"][0]["ci_sequence"],
        serde_json::json!([
            {
                "step": "benchmark",
                "command": [
                    "continuitydb",
                    "benchmark-local-model",
                    "--candidate",
                    "Qwen/Qwen2.5-0.5B-Instruct",
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
                    "<artifacts>/qwen-qwen2-5-0-5b-instruct",
                    "--report-path",
                    "<artifacts>/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
                    "--failure-report-path",
                    "<artifacts>/qwen-qwen2-5-0-5b-instruct/failure-report.json"
                ]
            },
            {
                "step": "validate_bundle",
                "depends_on": "benchmark",
                "command": [
                    "continuitydb",
                    "validate-local-model-bundle",
                    "--artifact-dir",
                    "<artifacts>/qwen-qwen2-5-0-5b-instruct",
                    "--report-path",
                    "<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "--failure-report-path",
                    "<artifacts>/qwen-qwen2-5-0-5b-instruct/validation-failure-report.json"
                ]
            }
        ])
    );
    assert_eq!(
        json["candidates"][0]["recommended_runner_arguments_with_grammar"][0].as_str(),
        Some("llama-cli")
    );
    assert_eq!(
        json["candidates"][1]["model_id"].as_str(),
        Some("Qwen/Qwen3-0.6B")
    );
    assert_eq!(
        json["candidates"][1]["job_id"].as_str(),
        Some("local-model-qwen-qwen3-0-6b")
    );
    assert_eq!(
        json["candidates"][1]["job_name"].as_str(),
        Some("Local model quality gate: Qwen/Qwen3-0.6B")
    );
    assert_eq!(
        json["candidates"][1]["artifact_upload_name"].as_str(),
        Some("continuitydb-local-model-qwen-qwen3-0-6b")
    );
    assert_eq!(
        json["candidates"][1]["artifact_retention"],
        serde_json::json!({
            "class": "local_model_quality_gate_bundle",
            "days": 30,
            "upload_path": "<artifacts>/qwen-qwen3-0-6b"
        })
    );
    assert_eq!(
        json["candidates"][1]["artifact_download"],
        serde_json::json!({
            "artifact_name": "continuitydb-local-model-qwen-qwen3-0-6b",
            "download_path": "<artifacts>/downloaded/qwen-qwen3-0-6b",
            "restore_path": "<artifacts>/qwen-qwen3-0-6b"
        })
    );
    assert_eq!(
        json["candidates"][1]["result_artifacts"],
        serde_json::json!({
            "benchmark_report": "<artifacts>/qwen-qwen3-0-6b/benchmark-report.json",
            "benchmark_failure_report": "<artifacts>/qwen-qwen3-0-6b/failure-report.json",
            "validation_report": "<artifacts>/qwen-qwen3-0-6b/validation-report.json",
            "validation_failure_report": "<artifacts>/qwen-qwen3-0-6b/validation-failure-report.json",
            "bundle_manifest": "<artifacts>/qwen-qwen3-0-6b/manifest.json"
        })
    );
    assert_eq!(json["ci_matrix"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        json["ci_matrix"][0]["job_id"].as_str(),
        Some("local-model-qwen-qwen2-5-0-5b-instruct")
    );
    assert_eq!(
        json["ci_matrix"][0]["job_name"].as_str(),
        Some("Local model quality gate: Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["ci_matrix"][0]["artifact_upload_name"].as_str(),
        Some("continuitydb-local-model-qwen-qwen2-5-0-5b-instruct")
    );
    assert_eq!(
        json["ci_matrix"][0]["artifact_retention"],
        json["candidates"][0]["artifact_retention"]
    );
    assert_eq!(
        json["ci_matrix"][0]["artifact_download"],
        json["candidates"][0]["artifact_download"]
    );
    assert_eq!(
        json["ci_matrix"][0]["result_artifacts"],
        json["candidates"][0]["result_artifacts"]
    );
    assert_eq!(
        json["ci_matrix"][0]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["ci_matrix"][0]["evaluation_priority"].as_u64(),
        Some(1)
    );
    assert_eq!(
        json["ci_matrix"][0]["artifact_dir"].as_str(),
        Some("<artifacts>/qwen-qwen2-5-0-5b-instruct")
    );
    assert_eq!(
        json["ci_matrix"][0]["steps"],
        json["candidates"][0]["ci_sequence"]
    );
    assert_eq!(
        json["ci_matrix"][1]["model_id"].as_str(),
        Some("Qwen/Qwen3-0.6B")
    );
    assert_eq!(
        json["ci_matrix"][1]["job_id"].as_str(),
        Some("local-model-qwen-qwen3-0-6b")
    );
    assert_eq!(
        json["ci_matrix"][1]["job_name"].as_str(),
        Some("Local model quality gate: Qwen/Qwen3-0.6B")
    );
    assert_eq!(
        json["ci_matrix"][1]["artifact_upload_name"].as_str(),
        Some("continuitydb-local-model-qwen-qwen3-0-6b")
    );
    assert_eq!(
        json["ci_matrix"][1]["artifact_retention"],
        json["candidates"][1]["artifact_retention"]
    );
    assert_eq!(
        json["ci_matrix"][1]["artifact_download"],
        json["candidates"][1]["artifact_download"]
    );
    assert_eq!(
        json["ci_matrix"][1]["result_artifacts"],
        json["candidates"][1]["result_artifacts"]
    );
    assert_eq!(
        json["ci_matrix"][1]["evaluation_priority"].as_u64(),
        Some(2)
    );
    assert_eq!(
        json["ci_matrix"][1]["artifact_dir"].as_str(),
        Some("<artifacts>/qwen-qwen3-0-6b")
    );

    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_quality_gate_plan_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-local-model-quality-gate-plan").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-quality-gate-plan-validation")
            .with_extension("json");

    let report_output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-plan")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, report_output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-plan")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_plan_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(json["candidate_set"].as_str(), Some("quality_gate"));
    assert_eq!(json["candidate_count"].as_u64(), Some(2));
    assert_eq!(json["total_steps"].as_u64(), Some(4));
    assert_eq!(
        json["default_quality_gate_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_run_local_model_quality_gate_dry_run_executes_candidate_set(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-gate-run-report");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-validation-report");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--dry-run")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("Qwen/Qwen2.5-0.5B-Instruct=/models/qwen2.5.gguf")
        .arg("--model-path")
        .arg("Qwen/Qwen3-0.6B=/models/qwen3.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--require-ready")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_run")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("run-local-model-quality-gate")
    );
    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["candidate_count"].as_u64(), Some(2));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    let report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    assert_eq!(
        report_json["generated_by_command"].as_str(),
        Some("run-local-model-quality-gate")
    );
    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-run-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validation_json: Value = serde_json::from_slice(&validation_output)?;
    assert_eq!(
        validation_json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_run_report_validation")
    );
    assert_eq!(validation_json["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_json["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["candidate_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["complete_candidate_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["incomplete_candidate_count"].as_u64(),
        Some(0)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["covered"]
            .as_array()
            .map(Vec::len),
        Some(7)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        validation_json["candidate_validations"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        validation_json["candidate_validations"][0]["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        validation_json["candidate_validations"][0]["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(validation_report_path.exists());
    assert_eq!(
        json["runtime_preflight"]["runner"],
        serde_json::json!({
            "path": "/missing/local-model-runner",
            "required": false,
            "available": true
        })
    );
    assert_eq!(
        json["runtime_preflight"]["model_artifacts"][0],
        serde_json::json!({
            "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
            "model_path": "/models/qwen2.5.gguf",
            "exists": false,
            "required": false
        })
    );
    assert_eq!(json["status"]["summary"]["ready"].as_bool(), Some(true));
    assert_eq!(json["status"]["summary"]["passed"].as_u64(), Some(2));
    assert_eq!(
        json["candidates"][0]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["candidates"][0]["status"].as_str(), Some("passed"));
    assert_eq!(
        json["candidates"][0]["benchmark"]["status"].as_str(),
        Some("passed")
    );
    assert_eq!(
        json["candidates"][0]["validation"]["status"].as_str(),
        Some("passed")
    );
    assert_eq!(
        json["candidates"][0]["validation_report_path"].as_str(),
        Some(
            artifact_root
                .join("qwen-qwen2-5-0-5b-instruct")
                .join("validation-report.json")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert!(artifact_root
        .join("qwen-qwen2-5-0-5b-instruct")
        .join("benchmark-report.json")
        .exists());
    assert!(artifact_root
        .join("qwen-qwen2-5-0-5b-instruct")
        .join("validation-report.json")
        .exists());
    assert!(artifact_root
        .join("qwen-qwen3-0-6b")
        .join("benchmark-report.json")
        .exists());
    assert!(artifact_root
        .join("qwen-qwen3-0-6b")
        .join("validation-report.json")
        .exists());

    if baseline_path.exists() {
        fs::remove_file(baseline_path)?;
    }
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_quality_gate_run_output_validates_retained_stdout(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-output-baseline");
    let run_output_path = temp_store_path("continuitydb-cli-local-model-gate-run-output");
    let report_path = temp_store_path("continuitydb-cli-local-model-gate-run-output-report");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-output-validation");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-output-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--dry-run")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("Qwen/Qwen2.5-0.5B-Instruct=/models/qwen2.5.gguf")
        .arg("--model-path")
        .arg("Qwen/Qwen3-0.6B=/models/qwen3.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--require-ready")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&run_output_path, output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-run-output")
        .arg("--report-path")
        .arg(&run_output_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validation_json: Value = serde_json::from_slice(&validation_output)?;
    assert_eq!(
        validation_json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_run_output_validation")
    );
    assert_eq!(validation_json["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_json["run_output"]["generated_by_command"].as_str(),
        Some("run-local-model-quality-gate")
    );
    assert_eq!(
        validation_json["run_output"]["dry_run"].as_bool(),
        Some(true)
    );
    assert_eq!(
        validation_json["run_output"]["candidate_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["run_output"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        validation_json["report_parity"]["matches_report_file"].as_bool(),
        Some(true)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["complete_candidate_count"].as_u64(),
        Some(2)
    );
    assert!(validation_report_path.exists());

    if baseline_path.exists() {
        fs::remove_file(baseline_path)?;
    }
    fs::remove_file(run_output_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_run_local_model_quality_gate_rejects_missing_model_binding(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-missing-baseline");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-missing-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--dry-run")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("Qwen/Qwen2.5-0.5B-Instruct=/models/qwen2.5.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .failure()
        .stderr(contains("missing --model-path binding for Qwen/Qwen3-0.6B"));

    if artifact_root.exists() {
        fs::remove_dir_all(artifact_root)?;
    }
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_quality_gate_run_report_rejects_candidate_tamper(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-tamper-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-gate-run-tamper-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-tamper-failure");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-tamper-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--dry-run")
        .arg("--executable")
        .arg("/bin/echo")
        .arg("--model-path")
        .arg("Qwen/Qwen2.5-0.5B-Instruct=/models/qwen2.5.gguf")
        .arg("--model-path")
        .arg("Qwen/Qwen3-0.6B=/models/qwen3.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let mut report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    report["candidates"][0]["model_id"] = Value::from("Qwen/Tampered");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-run-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model quality-gate run report candidate model mismatch",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("local_model_quality_gate_run_report_validation")
    );

    if baseline_path.exists() {
        fs::remove_file(baseline_path)?;
    }
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_quality_gate_run_report_rejects_acceptance_coverage_tamper(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-coverage-tamper-baseline");
    let report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-coverage-tamper-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-coverage-tamper-failure");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-coverage-tamper-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--dry-run")
        .arg("--executable")
        .arg("/bin/echo")
        .arg("--model-path")
        .arg("Qwen/Qwen2.5-0.5B-Instruct=/models/qwen2.5.gguf")
        .arg("--model-path")
        .arg("Qwen/Qwen3-0.6B=/models/qwen3.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let benchmark_report_path = artifact_root
        .join("qwen-qwen2-5-0-5b-instruct")
        .join("benchmark-report.json");
    let mut benchmark_report: Value = serde_json::from_slice(&fs::read(&benchmark_report_path)?)?;
    benchmark_report["acceptance_coverage"]["complete"] = Value::from(false);
    benchmark_report["acceptance_coverage"]["missing"] =
        serde_json::json!(["evidence_citation_preservation"]);
    fs::write(
        &benchmark_report_path,
        serde_json::to_string_pretty(&benchmark_report)?,
    )?;
    refresh_local_model_benchmark_report_metadata(
        &artifact_root.join("qwen-qwen2-5-0-5b-instruct"),
        &benchmark_report,
    )?;

    let status_output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output)?;
    let mut run_report: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    run_report["status"] = status_json;
    fs::write(&report_path, serde_json::to_string_pretty(&run_report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-run-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model quality-gate run report benchmark acceptance coverage incomplete",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("local_model_quality_gate_run_report_validation")
    );

    if baseline_path.exists() {
        fs::remove_file(baseline_path)?;
    }
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_run_local_model_quality_gate_fail_on_not_ready_writes_failure_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-not-ready-baseline");
    let report_path = temp_store_path("continuitydb-cli-local-model-gate-run-not-ready-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-not-ready-failure");
    let qwen25_path = temp_store_path("continuitydb-cli-local-model-gate-not-ready-qwen25");
    let qwen3_path = temp_store_path("continuitydb-cli-local-model-gate-not-ready-qwen3");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-not-ready-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::write(&qwen25_path, "fake model artifact")?;
    fs::write(&qwen3_path, "fake model artifact")?;

    Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--executable")
        .arg("/bin/false")
        .arg("--model-path")
        .arg(format!(
            "Qwen/Qwen2.5-0.5B-Instruct={}",
            qwen25_path.display()
        ))
        .arg("--model-path")
        .arg(format!("Qwen/Qwen3-0.6B={}", qwen3_path.display()))
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .arg("--fail-on-not-ready")
        .assert()
        .failure()
        .stderr(contains("local model quality gate is not ready"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("local_model_quality_gate_readiness")
    );
    assert_eq!(
        failure_report["status"]["summary"]["ready"].as_bool(),
        Some(false)
    );
    assert_eq!(
        failure_report["status"]["summary"]["failed"].as_u64(),
        Some(2)
    );
    assert_eq!(
        failure_report["candidates"][0]["status"].as_str(),
        Some("benchmark_failed")
    );
    assert!(report_path.exists());

    if baseline_path.exists() {
        fs::remove_file(baseline_path)?;
    }
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    fs::remove_file(qwen25_path)?;
    fs::remove_file(qwen3_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_run_local_model_quality_gate_preflights_real_model_paths_before_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-preflight-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-preflight-failure");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-gate-run-preflight-validation");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-preflight-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }

    Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--executable")
        .arg("/bin/echo")
        .arg("--model-path")
        .arg("Qwen/Qwen2.5-0.5B-Instruct=/missing/qwen2.5.gguf")
        .arg("--model-path")
        .arg("Qwen/Qwen3-0.6B=/missing/qwen3.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "local model artifact is required for Qwen/Qwen2.5-0.5B-Instruct",
        ));

    assert!(!artifact_root.exists());
    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(
        failure_report["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_run")
    );
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("local_model_quality_gate_run")
    );
    assert!(failure_report["failure"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("local model artifact is required")));
    assert_eq!(
        failure_report["runtime_preflight"]["runner"],
        serde_json::json!({
            "path": "/bin/echo",
            "required": true,
            "available": true
        })
    );
    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-run-report")
        .arg("--report-path")
        .arg(&failure_report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validation_json: Value = serde_json::from_slice(&validation_output)?;
    assert_eq!(validation_json["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_json["acceptance_coverage"]["complete"].as_bool(),
        Some(false)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["candidate_count"].as_u64(),
        Some(0)
    );
    assert_eq!(
        validation_json["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        validation_json["run_report"]["failure"]["stage"].as_str(),
        Some("local_model_quality_gate_run")
    );
    fs::remove_file(failure_report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_run_local_model_quality_gate_preflights_real_runner_before_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-gate-run-runner-baseline");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-run-runner-{}",
        std::process::id()
    ));
    let qwen25_path = temp_store_path("continuitydb-cli-local-model-gate-qwen25");
    let qwen3_path = temp_store_path("continuitydb-cli-local-model-gate-qwen3");
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::write(&qwen25_path, "fake model artifact")?;
    fs::write(&qwen3_path, "fake model artifact")?;

    Command::cargo_bin("continuitydb")?
        .arg("run-local-model-quality-gate")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg(format!(
            "Qwen/Qwen2.5-0.5B-Instruct={}",
            qwen25_path.display()
        ))
        .arg("--model-path")
        .arg(format!("Qwen/Qwen3-0.6B={}", qwen3_path.display()))
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .failure()
        .stderr(contains("local model runner executable is required"));

    assert!(!artifact_root.exists());
    fs::remove_file(qwen25_path)?;
    fs::remove_file(qwen3_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_evaluation_suite_accepts_current_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-local-model-evaluation-suite-report")
        .with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-evaluation-suite-validation")
            .with_extension("json");

    let report_output = Command::cargo_bin("continuitydb")?
        .arg("local-model-evaluation-suite")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, report_output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-evaluation-suite")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_evaluation_suite_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(json["total_cases"].as_u64(), Some(9));
    assert_eq!(
        json["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["acceptance_coverage"]["covered"]
            .as_array()
            .map(Vec::len),
        Some(7)
    );
    assert_eq!(
        json["evaluation_suite_fingerprint"].as_str(),
        serde_json::from_slice::<Value>(&fs::read(&report_path)?)?["evaluation_suite_fingerprint"]
            .as_str()
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_status_reports_missing_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-empty-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_status")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["generated_by_command"].as_str(),
        Some("local-model-quality-gate-status")
    );
    assert_eq!(
        json["artifact_root"].as_str(),
        Some(artifact_root.display().to_string().as_str())
    );
    assert_eq!(
        json["summary"],
        serde_json::json!({
            "candidate_count": 2,
            "ready": false,
            "passed": 0,
            "failed": 0,
            "missing": 2,
            "status_counts": {
                "passed": 0,
                "missing_artifacts": 2,
                "invalid_bundle": 0,
                "benchmark_failed": 0,
                "validation_failed": 0
            },
            "passed_models": [],
            "passing_report_summaries": [],
            "failed_models": [],
            "failure_report_summaries": [],
            "invalid_bundle_summaries": [],
            "missing_artifact_summaries": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen2-5-0-5b-instruct")
                        .display()
                        .to_string(),
                    "missing_artifacts": [
                        "artifact_dir",
                        "benchmark_report",
                        "bundle_manifest"
                    ]
                },
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "missing_artifacts": [
                        "artifact_dir",
                        "benchmark_report",
                        "bundle_manifest"
                    ]
                }
            ],
            "readiness_blockers": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen2-5-0-5b-instruct")
                        .display()
                        .to_string(),
                    "recommended_action": "run_quality_gate_candidate"
                },
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "recommended_action": "run_quality_gate_candidate"
                }
            ],
            "readiness_action_counts": {
                "run_quality_gate_candidate": 2,
                "regenerate_quality_gate_bundle": 0,
                "inspect_benchmark_failure_report": 0,
                "inspect_validation_failure_report": 0
            },
            "missing_models": [
                "Qwen/Qwen2.5-0.5B-Instruct",
                "Qwen/Qwen3-0.6B"
            ]
        })
    );
    assert_eq!(
        json["candidates"][0]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["artifact_dir"].as_str(),
        Some(
            artifact_root
                .join("qwen-qwen2-5-0-5b-instruct")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        json["candidates"][0]["status"].as_str(),
        Some("missing_artifacts")
    );
    assert_eq!(json["candidates"][0]["ready"].as_bool(), Some(false));
    assert_eq!(
        json["candidates"][0]["missing_artifacts"],
        serde_json::json!(["artifact_dir", "benchmark_report", "bundle_manifest"])
    );
    assert_eq!(
        json["candidates"][1]["model_id"].as_str(),
        Some("Qwen/Qwen3-0.6B")
    );
    assert_eq!(
        json["candidates"][1]["status"].as_str(),
        Some("missing_artifacts")
    );
    assert_eq!(json["candidates"][1]["ready"].as_bool(), Some(false));

    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_quality_gate_status_accepts_missing_artifact_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-validation-{}",
        std::process::id()
    ));
    let report_path = temp_store_path("continuitydb-cli-local-model-quality-gate-status-report")
        .with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-quality-gate-status-validation-report")
            .with_extension("json");
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;

    let status_output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, status_output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-quality-gate-status")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validation_json: Value = serde_json::from_slice(&validation_output)?;
    assert_eq!(
        validation_json["format"].as_str(),
        Some("continuitydb.local_model_quality_gate_status_validation")
    );
    assert_eq!(validation_json["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_json["status_report"]["generated_by_command"].as_str(),
        Some("local-model-quality-gate-status")
    );
    assert_eq!(
        validation_json["status_report"]["artifact_root"].as_str(),
        Some(artifact_root.display().to_string().as_str())
    );
    assert_eq!(
        validation_json["status_report"]["candidate_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["status_report"]["ready"].as_bool(),
        Some(false)
    );
    assert_eq!(
        validation_json["status_report"]["missing"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["status_report"]["readiness_blocker_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["status_report"]["run_quality_gate_candidate_actions"].as_u64(),
        Some(2)
    );
    assert!(validation_report_path.exists());

    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_status_require_ready_fails_when_not_ready(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-require-ready-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--require-ready")
        .assert()
        .failure()
        .stderr(contains("local model quality gate is not ready"))
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["summary"]["ready"].as_bool(), Some(false));
    assert_eq!(json["summary"]["missing"].as_u64(), Some(2));
    assert_eq!(
        json["summary"]["readiness_blockers"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_validate_local_model_require_ready_status_accepts_failed_gate_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-require-ready-validation-{}",
        std::process::id()
    ));
    let report_path = temp_store_path("continuitydb-cli-local-model-require-ready-status-report")
        .with_extension("json");
    let stderr_path =
        temp_store_path("continuitydb-cli-local-model-require-ready-stderr").with_extension("txt");
    let validation_report_path =
        temp_store_path("continuitydb-cli-local-model-require-ready-validation-report")
            .with_extension("json");
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .arg("--require-ready")
        .assert()
        .failure()
        .get_output()
        .clone();
    fs::write(&report_path, &output.stdout)?;
    fs::write(&stderr_path, &output.stderr)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-local-model-require-ready-status")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--stderr-path")
        .arg(&stderr_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validation_json: Value = serde_json::from_slice(&validation_output)?;
    assert_eq!(
        validation_json["format"].as_str(),
        Some("continuitydb.local_model_require_ready_status_validation")
    );
    assert_eq!(validation_json["valid"].as_bool(), Some(true));
    assert_eq!(
        validation_json["status_report"]["ready"].as_bool(),
        Some(false)
    );
    assert_eq!(
        validation_json["status_report"]["missing"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["status_report"]["readiness_blocker_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        validation_json["stderr"]["contains_readiness_error"].as_bool(),
        Some(true)
    );
    assert!(validation_report_path.exists());

    fs::remove_file(report_path)?;
    fs::remove_file(stderr_path)?;
    fs::remove_file(validation_report_path)?;
    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_status_rejects_invalid_existing_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-status-invalid-baseline");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-invalid-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;
    let artifact_dir = artifact_root.join("qwen-qwen2-5-0-5b-instruct");

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;
    let manifest_path = artifact_dir.join("local-model-benchmark.manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest["benchmark_report_bytes"] = Value::from(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["summary"],
        serde_json::json!({
            "candidate_count": 2,
            "ready": false,
            "passed": 0,
            "failed": 1,
            "missing": 1,
            "status_counts": {
                "passed": 0,
                "missing_artifacts": 1,
                "invalid_bundle": 1,
                "benchmark_failed": 0,
                "validation_failed": 0
            },
            "passed_models": [],
            "passing_report_summaries": [],
            "failed_models": [
                "Qwen/Qwen2.5-0.5B-Instruct"
            ],
            "failure_report_summaries": [],
            "invalid_bundle_summaries": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "invalid_bundle",
                    "artifact_dir": artifact_dir.display().to_string(),
                    "validation_error": json["candidates"][0]["validation_error"].clone()
                }
            ],
            "missing_artifact_summaries": [
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "missing_artifacts": [
                        "artifact_dir",
                        "benchmark_report",
                        "bundle_manifest"
                    ]
                }
            ],
            "readiness_blockers": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "invalid_bundle",
                    "artifact_dir": artifact_dir.display().to_string(),
                    "recommended_action": "regenerate_quality_gate_bundle"
                },
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "recommended_action": "run_quality_gate_candidate"
                }
            ],
            "readiness_action_counts": {
                "run_quality_gate_candidate": 1,
                "regenerate_quality_gate_bundle": 1,
                "inspect_benchmark_failure_report": 0,
                "inspect_validation_failure_report": 0
            },
            "missing_models": [
                "Qwen/Qwen3-0.6B"
            ]
        })
    );
    assert_eq!(
        json["candidates"][0]["status"].as_str(),
        Some("invalid_bundle")
    );
    assert_eq!(json["candidates"][0]["ready"].as_bool(), Some(false));
    assert!(json["candidates"][0]["validation_error"]
        .as_str()
        .is_some_and(|message| message.contains("benchmark manifest byte count mismatch")));
    assert_eq!(
        json["candidates"][0]["missing_artifacts"],
        serde_json::json!([])
    );
    assert_eq!(
        json["candidates"][1]["status"].as_str(),
        Some("missing_artifacts")
    );

    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_status_summarizes_passing_bundle_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-status-passing-baseline");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-passing-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;
    let artifact_dir = artifact_root.join("qwen-qwen2-5-0-5b-instruct");

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["summary"],
        serde_json::json!({
            "candidate_count": 2,
            "ready": false,
            "passed": 1,
            "failed": 0,
            "missing": 1,
            "status_counts": {
                "passed": 1,
                "missing_artifacts": 1,
                "invalid_bundle": 0,
                "benchmark_failed": 0,
                "validation_failed": 0
            },
            "passed_models": [
                "Qwen/Qwen2.5-0.5B-Instruct"
            ],
            "passing_report_summaries": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "passed",
                    "artifact_dir": artifact_dir.display().to_string(),
                    "benchmark_report": json["candidates"][0]["benchmark_report"].clone()
                }
            ],
            "failed_models": [],
            "failure_report_summaries": [],
            "invalid_bundle_summaries": [],
            "missing_artifact_summaries": [
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "missing_artifacts": [
                        "artifact_dir",
                        "benchmark_report",
                        "bundle_manifest"
                    ]
                }
            ],
            "readiness_blockers": [
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "recommended_action": "run_quality_gate_candidate"
                }
            ],
            "readiness_action_counts": {
                "run_quality_gate_candidate": 1,
                "regenerate_quality_gate_bundle": 0,
                "inspect_benchmark_failure_report": 0,
                "inspect_validation_failure_report": 0
            },
            "missing_models": [
                "Qwen/Qwen3-0.6B"
            ]
        })
    );
    assert_eq!(json["candidates"][0]["status"].as_str(), Some("passed"));
    assert_eq!(json["candidates"][0]["ready"].as_bool(), Some(true));
    assert_eq!(
        json["candidates"][0]["benchmark_report"]["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["benchmark_report"]["candidate_selection"],
        serde_json::json!({
            "source": "explicit",
            "model_id": "Qwen/Qwen2.5-0.5B-Instruct"
        })
    );
    assert_eq!(
        json["candidates"][0]["benchmark_report"]["dry_run"].as_bool(),
        Some(true)
    );
    assert!(
        json["candidates"][0]["benchmark_report"]["evaluation_suite_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert_eq!(
        json["candidates"][0]["benchmark_report"]["acceptance_coverage"]["complete"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["candidates"][0]["benchmark_report"]["acceptance_coverage"]["missing"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(
        json["candidates"][0]["benchmark_report"]["prompt_fingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
    );
    assert_eq!(
        json["candidates"][0]["benchmark_report"]["response_schema_version"].as_u64(),
        Some(1)
    );

    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_status_classifies_benchmark_failure_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-status-failure-baseline");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-failure-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;
    let artifact_dir = artifact_root.join("qwen-qwen2-5-0-5b-instruct");

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;
    let failure_report_path = artifact_dir.join("failure-report.json");
    fs::write(
        &failure_report_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "failure": {
                "stage": "benchmark",
                "message": "quality gate failed"
            }
        }))?,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["summary"],
        serde_json::json!({
            "candidate_count": 2,
            "ready": false,
            "passed": 0,
            "failed": 1,
            "missing": 1,
            "status_counts": {
                "passed": 0,
                "missing_artifacts": 1,
                "invalid_bundle": 0,
                "benchmark_failed": 1,
                "validation_failed": 0
            },
            "passed_models": [],
            "passing_report_summaries": [],
            "failed_models": [
                "Qwen/Qwen2.5-0.5B-Instruct"
            ],
            "failure_report_summaries": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "benchmark_failed",
                    "failure_report_path": failure_report_path.display().to_string(),
                    "failure": {
                        "stage": "benchmark",
                        "message": "quality gate failed"
                    }
                }
            ],
            "invalid_bundle_summaries": [],
            "missing_artifact_summaries": [
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "missing_artifacts": [
                        "artifact_dir",
                        "benchmark_report",
                        "bundle_manifest"
                    ]
                }
            ],
            "readiness_blockers": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "benchmark_failed",
                    "artifact_dir": artifact_dir.display().to_string(),
                    "recommended_action": "inspect_benchmark_failure_report"
                },
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "recommended_action": "run_quality_gate_candidate"
                }
            ],
            "readiness_action_counts": {
                "run_quality_gate_candidate": 1,
                "regenerate_quality_gate_bundle": 0,
                "inspect_benchmark_failure_report": 1,
                "inspect_validation_failure_report": 0
            },
            "missing_models": [
                "Qwen/Qwen3-0.6B"
            ]
        })
    );
    assert_eq!(
        json["candidates"][0]["status"].as_str(),
        Some("benchmark_failed")
    );
    assert_eq!(
        json["candidates"][0]["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["candidates"][0]["failure"],
        serde_json::json!({
            "stage": "benchmark",
            "message": "quality gate failed"
        })
    );
    assert_eq!(json["candidates"][0]["ready"].as_bool(), Some(false));
    assert!(json["candidates"][0]["validation_error"].is_null());

    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_quality_gate_status_classifies_validation_failure_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-local-model-status-validation-failure-baseline");
    let artifact_root = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-quality-gate-status-validation-failure-{}",
        std::process::id()
    ));
    if artifact_root.exists() {
        fs::remove_dir_all(&artifact_root)?;
    }
    fs::create_dir_all(&artifact_root)?;
    let artifact_dir = artifact_root.join("qwen-qwen2-5-0-5b-instruct");

    write_dry_run_local_model_bundle(&artifact_dir, &baseline_path)?;
    let failure_report_path = artifact_dir.join("validation-failure-report.json");
    fs::write(
        &failure_report_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "failure": {
                "stage": "validate_bundle",
                "message": "bundle validation failed"
            }
        }))?,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-quality-gate-status")
        .arg("--artifact-root")
        .arg(&artifact_root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["summary"],
        serde_json::json!({
            "candidate_count": 2,
            "ready": false,
            "passed": 0,
            "failed": 1,
            "missing": 1,
            "status_counts": {
                "passed": 0,
                "missing_artifacts": 1,
                "invalid_bundle": 0,
                "benchmark_failed": 0,
                "validation_failed": 1
            },
            "passed_models": [],
            "passing_report_summaries": [],
            "failed_models": [
                "Qwen/Qwen2.5-0.5B-Instruct"
            ],
            "failure_report_summaries": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "validation_failed",
                    "failure_report_path": failure_report_path.display().to_string(),
                    "failure": {
                        "stage": "validate_bundle",
                        "message": "bundle validation failed"
                    }
                }
            ],
            "invalid_bundle_summaries": [],
            "missing_artifact_summaries": [
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "missing_artifacts": [
                        "artifact_dir",
                        "benchmark_report",
                        "bundle_manifest"
                    ]
                }
            ],
            "readiness_blockers": [
                {
                    "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
                    "status": "validation_failed",
                    "artifact_dir": artifact_dir.display().to_string(),
                    "recommended_action": "inspect_validation_failure_report"
                },
                {
                    "model_id": "Qwen/Qwen3-0.6B",
                    "status": "missing_artifacts",
                    "artifact_dir": artifact_root
                        .join("qwen-qwen3-0-6b")
                        .display()
                        .to_string(),
                    "recommended_action": "run_quality_gate_candidate"
                }
            ],
            "readiness_action_counts": {
                "run_quality_gate_candidate": 1,
                "regenerate_quality_gate_bundle": 0,
                "inspect_benchmark_failure_report": 0,
                "inspect_validation_failure_report": 1
            },
            "missing_models": [
                "Qwen/Qwen3-0.6B"
            ]
        })
    );
    assert_eq!(
        json["candidates"][0]["status"].as_str(),
        Some("validation_failed")
    );
    assert_eq!(
        json["candidates"][0]["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["candidates"][0]["failure"],
        serde_json::json!({
            "stage": "validate_bundle",
            "message": "bundle validation failed"
        })
    );
    assert_eq!(json["candidates"][0]["ready"].as_bool(), Some(false));
    assert!(json["candidates"][0]["validation_error"].is_null());

    fs::remove_dir_all(artifact_root)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_contract_writes_schema_and_grammar() -> Result<(), Box<dyn std::error::Error>> {
    let schema_path = temp_store_path("continuitydb-cli-local-model-schema");
    let grammar_path = temp_store_path("continuitydb-cli-local-model-grammar");

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-contract")
        .arg("--schema-path")
        .arg(&schema_path)
        .arg("--grammar-path")
        .arg(&grammar_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let schema: Value = serde_json::from_str(&fs::read_to_string(&schema_path)?)?;
    let grammar = fs::read_to_string(&grammar_path)?;

    assert_eq!(json["schema_version"].as_u64(), Some(1));
    assert_eq!(
        json["schema_path"].as_str(),
        Some(schema_path.display().to_string().as_str())
    );
    assert_eq!(
        json["grammar_path"].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert_eq!(
        schema["$id"].as_str(),
        Some("https://continuitydb.dev/schemas/local-model-response.schema.json")
    );
    assert_eq!(schema["x-continuitydb-schema-version"].as_u64(), Some(1));
    assert!(json["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["schema_bytes"].as_u64(),
        Some(fs::read_to_string(&schema_path)?.len() as u64)
    );
    assert_eq!(json["grammar_bytes"].as_u64(), Some(grammar.len() as u64));
    assert!(grammar.contains("root ::= response"));
    assert!(grammar.contains("request-verification-action"));

    fs::remove_file(schema_path)?;
    fs::remove_file(grammar_path)?;
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_contract_writes_context_compiler_schema_and_grammar(
) -> Result<(), Box<dyn std::error::Error>> {
    let schema_path = temp_store_path("continuitydb-cli-local-model-context-compiler-schema");
    let grammar_path = temp_store_path("continuitydb-cli-local-model-context-compiler-grammar");

    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-contract")
        .arg("--context-compiler")
        .arg("--schema-path")
        .arg(&schema_path)
        .arg("--grammar-path")
        .arg(&grammar_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let schema: Value = serde_json::from_str(&fs::read_to_string(&schema_path)?)?;
    let grammar = fs::read_to_string(&grammar_path)?;

    assert_eq!(json["contract_kind"].as_str(), Some("context_compiler"));
    assert_eq!(json["schema_version"].as_u64(), Some(2));
    assert_eq!(
        schema["$id"].as_str(),
        Some("https://continuitydb.dev/schemas/local-model-context-compiler-response.schema.json")
    );
    assert_eq!(schema["x-continuitydb-schema-version"].as_u64(), Some(2));
    assert_eq!(
        schema["required"],
        serde_json::json!(["context_compiler_proposals"])
    );
    assert!(grammar.contains("context-compiler-proposals-field"));
    assert!(grammar.contains("falsification-brief"));
    assert!(!grammar.contains("request-verification-action"));

    fs::remove_file(schema_path)?;
    fs::remove_file(grammar_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_records_baseline_for_file_kernel() -> Result<(), Box<dyn std::error::Error>>
{
    let store_path = temp_store_path("continuitydb-cli-measure-workload-file-baseline-store");
    let baseline_path = temp_store_path("continuitydb-cli-measure-workload-file-baseline");
    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("file")
        .arg("--store-path")
        .arg(&store_path)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("file-small")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let baseline_text = fs::read_to_string(&baseline_path)?;
    let records: Vec<Value> = baseline_text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;

    assert_eq!(json["kernel"].as_str(), Some("file"));
    assert_eq!(json["baseline_label"].as_str(), Some("file-small"));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["label"].as_str(), Some("file-small"));
    assert_eq!(records[0]["kernel"].as_str(), Some("file"));
    assert_eq!(
        records[0]["snapshot"]["ingest"]["operation_count"].as_u64(),
        Some(8)
    );
    assert_eq!(
        records[0]["snapshot"]["lookup_plan"]["indexed_constraints"],
        serde_json::json!(["scope", "minimum_confidence"])
    );
    assert_eq!(
        records[0]["snapshot"]["lookup_plan"]["candidate_count"].as_u64(),
        Some(8)
    );

    fs::remove_file(store_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_reports_file_lookup_plan_baseline_regression(
) -> Result<(), Box<dyn std::error::Error>> {
    let first_store_path =
        temp_store_path("continuitydb-cli-measure-workload-file-plan-regression-store-first");
    let second_store_path =
        temp_store_path("continuitydb-cli-measure-workload-file-plan-regression-store-second");
    let baseline_path = temp_store_path("continuitydb-cli-measure-workload-file-plan-regression");
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("file")
        .arg("--store-path")
        .arg(&first_store_path)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("file-plan-regression")
        .assert()
        .success();

    let baseline_text = fs::read_to_string(&baseline_path)?;
    let mut record: Value = serde_json::from_str(
        baseline_text
            .lines()
            .next()
            .ok_or_else(|| std::io::Error::other("missing baseline record"))?,
    )?;
    record["snapshot"]["lookup_plan"]["candidate_count"] = Value::from(7);
    record["snapshot"]["lookup_plan"]["indexed_constraint_plans"][0]["candidate_count"] =
        Value::from(7);
    fs::write(
        &baseline_path,
        format!("{}\n", serde_json::to_string(&record)?),
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("file")
        .arg("--store-path")
        .arg(&second_store_path)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("file-plan-regression")
        .arg("--compare-baseline")
        .arg("--max-elapsed-growth-percent")
        .arg("1000000000000")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["baseline_comparison"]["passed"].as_bool(), Some(false));
    assert_eq!(
        json["baseline_comparison"]["regressions"][0]["LookupPlanCandidateCountChanged"]
            ["previous"]
            .as_u64(),
        Some(7)
    );
    assert_eq!(
        json["baseline_comparison"]["regressions"][0]["LookupPlanCandidateCountChanged"]["current"]
            .as_u64(),
        Some(8)
    );
    assert_eq!(
        json["baseline_comparison"]["regressions"][1]["LookupPlanConstraintCandidateCountChanged"]
            ["name"]
            .as_str(),
        Some("scope")
    );

    fs::remove_file(first_store_path)?;
    fs::remove_file(second_store_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_compares_baseline_and_reports_passed(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-measure-workload-compare-pass");
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .arg("--compare-baseline")
        .arg("--max-elapsed-growth-percent")
        .arg("1000000000000")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["baseline_comparison"]["passed"].as_bool(), Some(true));
    assert!(json["baseline_comparison"]["baseline_recorded_at"].is_string());
    assert_eq!(
        json["baseline_comparison"]["regressions"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    fs::remove_file(baseline_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_compares_baseline_and_fails_on_regression(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-measure-workload-compare-fail");
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .assert()
        .success();

    let baseline_text = fs::read_to_string(&baseline_path)?;
    let mut record: Value = serde_json::from_str(
        baseline_text
            .lines()
            .next()
            .ok_or_else(|| std::io::Error::other("missing baseline record"))?,
    )?;
    record["snapshot"]["workload"]["cell_count"] = Value::from(7);
    fs::write(
        &baseline_path,
        format!("{}\n", serde_json::to_string(&record)?),
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .arg("--compare-baseline")
        .arg("--fail-on-regression")
        .arg("--max-elapsed-growth-percent")
        .arg("1000000000000")
        .assert()
        .failure()
        .stderr(contains("workload baseline regression detected"));

    fs::remove_file(baseline_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_failure_report_path_records_regression(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-measure-workload-failure-report-baseline");
    let failure_report_path =
        temp_store_path("continuitydb-cli-measure-workload-failure-report").with_extension("json");
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .assert()
        .success();

    let baseline_text = fs::read_to_string(&baseline_path)?;
    let mut record: Value = serde_json::from_str(
        baseline_text
            .lines()
            .next()
            .ok_or_else(|| std::io::Error::other("missing baseline record"))?,
    )?;
    record["snapshot"]["workload"]["cell_count"] = Value::from(7);
    fs::write(
        &baseline_path,
        format!("{}\n", serde_json::to_string(&record)?),
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .arg("--compare-baseline")
        .arg("--fail-on-regression")
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .arg("--max-elapsed-growth-percent")
        .arg("1000000000000")
        .assert()
        .failure()
        .stderr(contains("workload baseline regression detected"));

    let report: Value = serde_json::from_str(&fs::read_to_string(&failure_report_path)?)?;
    assert_eq!(
        report["baseline_comparison"]["passed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["baseline_comparison"]["regressions"][0]["WorkloadCellCountChanged"]["previous"]
            .as_u64(),
        Some(7)
    );
    assert_eq!(
        report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(fs::read_to_string(&baseline_path)?.lines().count(), 1);

    fs::remove_file(baseline_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_measure_workload_artifact_dir_writes_regression_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path =
        temp_store_path("continuitydb-cli-measure-workload-artifact-regression-baseline");
    let artifact_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-measure-workload-artifact-regression-dir-{}",
        std::process::id()
    ));
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .assert()
        .success();

    let baseline_text = fs::read_to_string(&baseline_path)?;
    let mut record: Value = serde_json::from_str(
        baseline_text
            .lines()
            .next()
            .ok_or_else(|| std::io::Error::other("missing baseline record"))?,
    )?;
    record["snapshot"]["workload"]["cell_count"] = Value::from(7);
    fs::write(
        &baseline_path,
        format!("{}\n", serde_json::to_string(&record)?),
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .arg("--label")
        .arg("memory-compare")
        .arg("--compare-baseline")
        .arg("--fail-on-regression")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--max-elapsed-growth-percent")
        .arg("1000000000000")
        .assert()
        .failure()
        .stderr(contains("workload baseline regression detected"));

    let report_path = artifact_dir.join("workload-report.json");
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let bundle_manifest_path = artifact_dir.join("continuitydb-workload.manifest.json");
    let cells_path = artifact_dir.join("workload-cells.json");
    let checkout_request_path = artifact_dir.join("checkout-request.json");
    assert_eq!(
        report["baseline_comparison"]["passed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report["bundle_manifest"]["manifest_path"].as_str(),
        Some(bundle_manifest_path.display().to_string().as_str())
    );
    let bundle_manifest: Value = serde_json::from_str(&fs::read_to_string(&bundle_manifest_path)?)?;
    assert_eq!(
        bundle_manifest["baseline_comparison"]["passed"].as_bool(),
        Some(false)
    );
    assert_eq!(
        bundle_manifest["workload_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        report["workload_artifacts"]["cells_path"].as_str(),
        Some(cells_path.display().to_string().as_str())
    );
    assert_eq!(
        report["workload_artifacts"]["checkout_request_path"].as_str(),
        Some(checkout_request_path.display().to_string().as_str())
    );
    assert!(cells_path.exists());
    assert!(checkout_request_path.exists());
    assert_eq!(fs::read_to_string(&baseline_path)?.lines().count(), 1);

    fs::remove_file(baseline_path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_measure_workload_compares_baseline_requires_baseline_path(
) -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("continuitydb")?
        .arg("measure-workload")
        .arg("--kernel")
        .arg("memory")
        .arg("--compare-baseline")
        .assert()
        .failure()
        .stderr(contains("baseline path is required"));

    Ok(())
}

#[test]
fn cli_checkout_query_executes_serialized_typed_query() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query")?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?")).with_requirements(
            QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            },
        ),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["cells"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-query")
    );
    assert_eq!(json["audit_traces"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["audit_traces"][0]["evidence"].as_array().map(Vec::len),
        Some(1)
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_typed_model_assisted_compiler_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-model-assisted-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-model-assisted-query");
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let cell = test_cell("project:continuitydb:cli-query-model-assisted")?;
    let cell_id = cell.id;
    let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
    db.ingest_cells_at_with_commit_id(vec![cell], committed_at, CommitId::new())?;
    let proposal = ContextCompilerProposal::new(
        cell_id,
        ContextPacketStrategy::OperationalBrief,
        ContextAbstractionLevel::Brief,
        vec!["model-assisted-operational".to_string()],
        vec!["test://cli".to_string()],
    )?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_requirements(QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            })
            .with_compiler_policy(ContextCompilerPolicy::ModelAssisted)
            .with_compiler_proposals(vec![proposal])
            .with_return_shape(QueryReturnShape::ContextPacketsOnly),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["context_packets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["context_packets"][0]["compiler_policy"].as_str(),
        Some("ModelAssisted")
    );
    assert_eq!(
        json["context_packets"][0]["strategy"].as_str(),
        Some("OperationalBrief")
    );
    assert_eq!(
        json["context_packets"][0]["compiler_reason_tags"][0].as_str(),
        Some("model-assisted-operational")
    );
    assert_eq!(
        json["context_packets"][0]["compiler_evidence_locators"][0].as_str(),
        Some("test://cli")
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_versioned_query_envelope() -> Result<(), Box<dyn std::error::Error>>
{
    let store_path = temp_store_path("continuitydb-cli-checkout-query-envelope-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-envelope-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-envelope")?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?")).with_requirements(
            QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            },
        ),
    );
    fs::write(&query_path, encode_query_json(query)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["cells"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-query-envelope")
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_summary_only_typed_query() -> Result<(), Box<dyn std::error::Error>>
{
    let store_path = temp_store_path("continuitydb-cli-checkout-query-summary-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-summary-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-summary")?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_requirements(QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            })
            .with_return_shape(QueryReturnShape::SummaryOnly),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.summary")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["summary"]["selected_cell_count"].as_u64(), Some(1));
    assert_eq!(json["summary"]["total_tokens"].as_u64(), Some(12));
    assert!(json.get("cells").is_none());
    assert!(json.get("audit_traces").is_none());

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_text_query_file() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-text-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-text-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-text")?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["cells"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-query-text")
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_summary_only_text_query() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-text-summary-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-text-summary-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-text-summary")?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN summary_only"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.summary")
    );
    assert_eq!(json["summary"]["selected_cell_count"].as_u64(), Some(1));
    assert_eq!(json["summary"]["citation_count"].as_u64(), Some(1));
    assert!(json.get("cells").is_none());

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_cells_only_text_query() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-text-cells-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-text-cells-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-text-cells")?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN cells_only"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.cells")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["cells"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-query-text-cells")
    );
    assert!(json.get("summary").is_none());
    assert!(json.get("audit_traces").is_none());

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_executes_context_packets_only_text_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-text-context-packets-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-text-context-packets-query");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.context_packets")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["context_packets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["context_packets"][0]["lines"][0].as_str(),
        Some("Current belief: context packets are the agent-facing projection.")
    );
    assert!(json["context_packets"][0]["origin"]["cell_id"]
        .as_str()
        .is_some());
    assert_eq!(
        json["context_packets"][0]["origin"]["lifecycle_stage"].as_str(),
        Some("Observed")
    );
    assert_eq!(
        json["context_packets"][0]["origin"]["activation"].as_str(),
        Some("Active")
    );
    let packet_confidence = json["context_packets"][0]["selection"]["max_confidence"]
        .as_f64()
        .ok_or_else(|| std::io::Error::other("missing packet selection confidence"))?;
    assert!((packet_confidence - 0.91).abs() < 0.0001);
    assert_eq!(
        json["context_packets"][0]["selection"]["utility_score"].as_f64(),
        Some(0.5)
    );
    assert_eq!(
        json["context_packets"][0]["selection"]["reasons"][0].as_str(),
        Some("EvidenceConfidence")
    );
    assert_eq!(
        json["context_packets"][0]["entries"][0]["source"]["Projection"].as_str(),
        Some("Semantic")
    );
    assert!(json.get("cells").is_none());
    assert!(json.get("summary").is_none());
    assert!(json.get("audit_traces").is_none());

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_context_packets_only_preserves_revision_context(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-checkout-query-context-packets-revision-store");
    let query_path =
        temp_store_path("continuitydb-cli-checkout-query-context-packets-revision-query");
    let (selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let selected_id_text = selected_id.to_string();
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.context_packets")
    );
    assert_eq!(json["context_packets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["context_packets"][0]["origin"]["cell_id"].as_str(),
        Some(selected_id_text.as_str())
    );
    assert_eq!(
        json["context_packets"][0]["revision_context"][0]["related_cell_id"].as_str(),
        Some(related_id_text.as_str())
    );
    assert_eq!(
        json["context_packets"][0]["revision_context"][0]["kind"].as_str(),
        Some("ConflictsWith")
    );
    assert_eq!(
        json["context_packets"][0]["revision_context"][0]["citations"],
        serde_json::json!(["test://cli"])
    );
    assert!(json.get("audit_traces").is_none());
    assert!(json.get("cells").is_none());

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_result_envelope_wraps_cells_only_text_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-envelope-cells-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-envelope-cells-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-envelope-cells")?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN cells_only"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.result")
    );
    assert_eq!(json["return_shape"].as_str(), Some("cells_only"));
    assert_eq!(json["result"]["type"].as_str(), Some("cells"));
    assert_eq!(json["result"]["result"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["result"]["result"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-query-envelope-cells")
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_accepts_summary_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-validate-checkout-summary-store");
    let query_path = temp_store_path("continuitydb-cli-validate-checkout-summary-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-report").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-validation")
            .with_extension("json");
    write_committed_store(
        &store_path,
        "project:continuitydb:validate-checkout-summary",
    )?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN summary_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.summary_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["summary"]["selected_cell_count"].as_u64(), Some(1));
    assert_eq!(
        json["summary"]["bounded_by_token_budget"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_format_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-validate-checkout-summary-format-drift")
        .with_extension("json");
    let failure_report_path = temp_store_path("continuitydb-cli-validate-checkout-summary-failure")
        .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.result",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("checkout_query_summary_validation")
    );
    assert_eq!(
        failure_report["checkout_query_summary_report"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_result_envelope_wraps_summary_only_text_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-envelope-summary-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-envelope-summary-query");
    write_committed_store(
        &store_path,
        "project:continuitydb:cli-query-envelope-summary",
    )?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN summary_only"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.result")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["return_shape"].as_str(), Some("summary_only"));
    assert_eq!(json["result"]["type"].as_str(), Some("summary"));
    assert_eq!(
        json["result"]["result"]["selected_cell_count"].as_u64(),
        Some(1)
    );
    assert!(json["result"]["result"].get("cells").is_none());

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_epistemic_action_reason_counts(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-action-reasons")
            .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-summary-missing-action-reasons-failure",
    )
    .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_epistemic_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-pressure")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-pressure-failure")
            .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "epistemic_action_reason_counts": [],
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

fn assert_checkout_query_summary_rejects_mutation(
    label: &str,
    mutate: impl FnOnce(&mut Value),
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-summary-{label}-store"
    ));
    let query_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-summary-{label}-query"
    ));
    let report_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-summary-{label}-report"
    ))
    .with_extension("json");
    let failure_report_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-summary-{label}-failure"
    ))
    .with_extension("json");
    write_committed_store(&store_path, &format!("project:continuitydb:{label}"))?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN summary_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    mutate(&mut report);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_out_of_range_epistemic_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("pressure-range", |report| {
        report["summary"]["epistemic_pressure"]["maximum_revision_pressure"] =
            serde_json::json!(1.1);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_out_of_range_salience_score(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("salience-range", |report| {
        report["summary"]["maximum_salience_score"] = serde_json::json!(-0.1);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_out_of_range_context_gap_priority(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("context-gap-priority-range", |report| {
        report["summary"]["maximum_context_gap_priority"] = serde_json::json!(1.2);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_negative_total_tokens(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("negative-total-tokens", |report| {
        report["summary"]["total_tokens"] = serde_json::json!(-1);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_negative_token_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("negative-token-budget", |report| {
        report["summary"]["token_budget"] = serde_json::json!(-1);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_zero_epistemic_action_count(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("zero-action-count", |report| {
        report["summary"]["epistemic_action_counts"] = serde_json::json!([
            {
                "action": "Use",
                "count": 0
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_zero_selection_reason_count(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("zero-selection-reason-count", |report| {
        report["summary"]["selection_reason_counts"] = serde_json::json!([
            {
                "reason": "EvidenceConfidence",
                "count": 0
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_zero_context_gap_kind_count(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("zero-context-gap-kind-count", |report| {
        report["summary"]["context_gap_kind_counts"] = serde_json::json!([
            {
                "kind": "MissingEvidence",
                "count": 0
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_duplicate_epistemic_action_count(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("duplicate-action-count", |report| {
        report["summary"]["epistemic_action_counts"] = serde_json::json!([
            {
                "action": "Use",
                "count": 1
            },
            {
                "action": "Use",
                "count": 1
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_duplicate_selection_reason_count(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("duplicate-selection-reason-count", |report| {
        report["summary"]["selection_reason_counts"] = serde_json::json!([
            {
                "reason": "EvidenceConfidence",
                "count": 1
            },
            {
                "reason": "EvidenceConfidence",
                "count": 1
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_duplicate_context_gap_kind_count(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("duplicate-context-gap-kind-count", |report| {
        report["summary"]["context_gap_kind_counts"] = serde_json::json!([
            {
                "kind": "MissingEvidence",
                "count": 1
            },
            {
                "kind": "MissingEvidence",
                "count": 1
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_epistemic_action_total_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("action-total-drift", |report| {
        report["summary"]["context_packet_count"] = serde_json::json!(1);
        report["summary"]["epistemic_action_counts"] = serde_json::json!([
            {
                "action": "Use",
                "count": 2
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_context_gap_total_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("context-gap-total-drift", |report| {
        report["summary"]["context_gap_count"] = serde_json::json!(0);
        report["summary"]["context_gap_kind_counts"] = serde_json::json!([
            {
                "kind": "MissingEvidence",
                "count": 1
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_invalidation_condition_total_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("invalidation-total-drift", |report| {
        report["summary"]["invalidation_condition_count"] = serde_json::json!(0);
        report["summary"]["invalidation_condition_kind_counts"] = serde_json::json!([
            {
                "kind": "ContradictoryEvidence",
                "count": 1
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_minimum_confidence(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("missing-minimum-confidence", |report| {
        if let Some(summary) = report["summary"].as_object_mut() {
            summary.remove("minimum_selected_confidence");
        }
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_inverted_confidence_range(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("inverted-confidence-range", |report| {
        report["summary"]["minimum_selected_confidence"] = serde_json::json!(0.9);
        report["summary"]["maximum_selected_confidence"] = serde_json::json!(0.7);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_null_confidence_with_selected_cells(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("selected-null-confidence", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(1);
        report["summary"]["minimum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["maximum_selected_confidence"] = serde_json::Value::Null;
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_uncertainty_count_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("uncertainty-count-drift", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(1);
        report["summary"]["uncertainty_count"] = serde_json::json!(2);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_context_packet_count_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("context-packet-count-drift", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(1);
        report["summary"]["context_packet_count"] = serde_json::json!(2);
        report["summary"]["epistemic_action_counts"] = serde_json::json!([
            {
                "action": "Use",
                "count": 2
            }
        ]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_empty_selection_token_residue(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("empty-selection-token-residue", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(0);
        report["summary"]["uncertainty_count"] = serde_json::json!(0);
        report["summary"]["context_packet_count"] = serde_json::json!(0);
        report["summary"]["epistemic_action_counts"] = serde_json::json!([]);
        report["summary"]["minimum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["maximum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["total_tokens"] = serde_json::json!(12);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_empty_selection_citation_residue(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("empty-selection-citation-residue", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(0);
        report["summary"]["uncertainty_count"] = serde_json::json!(0);
        report["summary"]["context_packet_count"] = serde_json::json!(0);
        report["summary"]["epistemic_action_counts"] = serde_json::json!([]);
        report["summary"]["minimum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["maximum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["total_tokens"] = serde_json::json!(0);
        report["summary"]["citation_count"] = serde_json::json!(1);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_empty_selection_score_residue(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("empty-selection-score-residue", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(0);
        report["summary"]["uncertainty_count"] = serde_json::json!(0);
        report["summary"]["context_packet_count"] = serde_json::json!(0);
        report["summary"]["citation_count"] = serde_json::json!(0);
        report["summary"]["total_tokens"] = serde_json::json!(0);
        report["summary"]["epistemic_action_counts"] = serde_json::json!([]);
        report["summary"]["epistemic_action_reason_counts"] = serde_json::json!([]);
        report["summary"]["selection_reason_counts"] = serde_json::json!([]);
        report["summary"]["minimum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["maximum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["maximum_salience_score"] = serde_json::json!(0.75);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_empty_selection_pressure_residue(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("empty-selection-pressure-residue", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(0);
        report["summary"]["uncertainty_count"] = serde_json::json!(0);
        report["summary"]["context_packet_count"] = serde_json::json!(0);
        report["summary"]["citation_count"] = serde_json::json!(0);
        report["summary"]["total_tokens"] = serde_json::json!(0);
        report["summary"]["epistemic_action_counts"] = serde_json::json!([]);
        report["summary"]["epistemic_action_reason_counts"] = serde_json::json!([]);
        report["summary"]["selection_reason_counts"] = serde_json::json!([]);
        report["summary"]["minimum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["maximum_selected_confidence"] = serde_json::Value::Null;
        report["summary"]["epistemic_pressure"]["maximum_checkout_pressure"] =
            serde_json::json!(0.6);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_selected_cell_without_citation(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("selected-cell-without-citation", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(1);
        report["summary"]["citation_count"] = serde_json::json!(0);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_bounded_token_budget_drift(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("bounded-token-budget-drift", |report| {
        report["summary"]["total_tokens"] = serde_json::json!(12);
        report["summary"]["token_budget"] = serde_json::json!(1200);
        report["summary"]["bounded_by_token_budget"] = serde_json::json!(false);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_selected_summary_without_selection_reasons(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("selected-without-selection-reasons", |report| {
        report["summary"]["selected_cell_count"] = serde_json::json!(1);
        report["summary"]["context_packet_count"] = serde_json::json!(1);
        report["summary"]["selection_reason_counts"] = serde_json::json!([]);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_context_gap_priority_without_gaps(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation("context-gap-priority-without-gaps", |report| {
        report["summary"]["context_gap_count"] = serde_json::json!(0);
        report["summary"]["context_gap_kind_counts"] = serde_json::json!([]);
        report["summary"]["maximum_context_gap_priority"] = serde_json::json!(0.75);
    })
}

#[test]
fn cli_validate_checkout_query_summary_rejects_invalidation_priority_without_conditions(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_checkout_query_summary_rejects_mutation(
        "invalidation-priority-without-conditions",
        |report| {
            report["summary"]["invalidation_condition_count"] = serde_json::json!(0);
            report["summary"]["invalidation_condition_kind_counts"] = serde_json::json!([]);
            report["summary"]["maximum_invalidation_priority"] = serde_json::json!(0.75);
        },
    )
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_selection_reason_counts(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-selection-reasons")
            .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-summary-missing-selection-reasons-failure",
    )
    .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "epistemic_action_reason_counts": [],
            "epistemic_pressure": {
                "maximum_revision_pressure": 0.0,
                "maximum_scavenging_pressure": 0.0,
                "maximum_checkout_pressure": 0.0
            },
            "maximum_context_affordance_score": 0.0,
            "maximum_salience_score": 0.0,
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_context_affordance_score(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-affordance")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-affordance-failure")
            .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "epistemic_action_reason_counts": [],
            "epistemic_pressure": {
                "maximum_revision_pressure": 0.0,
                "maximum_scavenging_pressure": 0.0,
                "maximum_checkout_pressure": 0.0
            },
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_salience_score(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-salience")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-salience-failure")
            .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "epistemic_action_reason_counts": [],
            "epistemic_pressure": {
                "maximum_revision_pressure": 0.0,
                "maximum_scavenging_pressure": 0.0,
                "maximum_checkout_pressure": 0.0
            },
            "maximum_context_affordance_score": 0.0,
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_context_gap_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-context-gap")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-context-gap-failure")
            .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "epistemic_action_reason_counts": [],
            "selection_reason_counts": [],
            "epistemic_pressure": {
                "maximum_revision_pressure": 0.0,
                "maximum_scavenging_pressure": 0.0,
                "maximum_checkout_pressure": 0.0
            },
            "maximum_context_affordance_score": 0.0,
            "maximum_salience_score": 0.0,
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_summary_rejects_missing_invalidation_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-invalidation")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-summary-missing-invalidation-failure")
            .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.summary",
        "format_version": 1,
        "summary": {
            "selected_cell_count": 1,
            "alternative_count": 0,
            "total_tokens": 12,
            "token_budget": 1200,
            "citation_count": 1,
            "uncertainty_count": 1,
            "context_packet_count": 1,
            "frontier_recommendation_count": 0,
            "revision_link_count": 0,
            "revision_context_count": 0,
            "epistemic_action_counts": [],
            "epistemic_action_reason_counts": [],
            "selection_reason_counts": [],
            "epistemic_pressure": {
                "maximum_revision_pressure": 0.0,
                "maximum_scavenging_pressure": 0.0,
                "maximum_checkout_pressure": 0.0
            },
            "maximum_context_affordance_score": 0.0,
            "maximum_salience_score": 0.0,
            "context_gap_count": 0,
            "context_gap_kind_counts": [],
            "maximum_context_gap_priority": null,
            "minimum_selected_confidence": 0.7,
            "maximum_selected_confidence": 0.7,
            "bounded_by_token_budget": true
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-summary")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query summary artifact is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_accepts_summary_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-validate-checkout-result-store");
    let query_path = temp_store_path("continuitydb-cli-validate-checkout-result-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-report").with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-validation")
            .with_extension("json");
    write_committed_store(&store_path, "project:continuitydb:validate-checkout-result")?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN summary_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.result_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["return_shape"].as_str(), Some("summary_only"));
    assert_eq!(json["result_type"].as_str(), Some("summary"));
    assert_eq!(json["summary"]["selected_cell_count"].as_u64(), Some(1));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_shape_drift() -> Result<(), Box<dyn std::error::Error>>
{
    let report_path = temp_store_path("continuitydb-cli-validate-checkout-result-shape-drift")
        .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-failure").with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.result",
        "format_version": 1,
        "return_shape": "packed_context_with_metadata",
        "result": {
            "type": "summary",
            "result": {
                "selected_cell_count": 1,
                "alternative_count": 0,
                "total_tokens": 12,
                "token_budget": 1200,
                "bounded_by_token_budget": true,
                "citation_count": 1,
                "uncertainty_count": 1,
                "context_packet_count": 1,
                "frontier_recommendation_count": 0,
                "revision_link_count": 0,
                "revision_context_count": 0,
                "minimum_selected_confidence": 0.7,
                "maximum_selected_confidence": 0.7
            }
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("checkout_query_result_validation")
    );
    assert_eq!(
        failure_report["checkout_query_result_report"]["parseable"].as_bool(),
        Some(true)
    );
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_summary_missing_epistemic_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    let report_path = temp_store_path("continuitydb-cli-validate-checkout-result-missing-pressure")
        .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-missing-pressure-failure")
            .with_extension("json");
    let report = serde_json::json!({
        "format": "continuitydb.checkout_query.result",
        "format_version": 1,
        "return_shape": "summary_only",
        "result": {
            "type": "summary",
            "result": {
                "selected_cell_count": 1,
                "alternative_count": 0,
                "total_tokens": 12,
                "token_budget": 1200,
                "citation_count": 1,
                "uncertainty_count": 1,
                "context_packet_count": 1,
                "frontier_recommendation_count": 0,
                "revision_link_count": 0,
                "revision_context_count": 0,
                "epistemic_action_counts": [],
                "epistemic_action_reason_counts": [],
                "minimum_selected_confidence": 0.7,
                "maximum_selected_confidence": 0.7,
                "bounded_by_token_budget": true
            }
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_epistemic_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-pressure-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-pressure-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-pressure-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-pressure-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("epistemic_pressure");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_compiler_policy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-policy-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-policy-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-policy-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-policy-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet"))?
        .remove("compiler_policy");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_epistemic_action_reasons(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-reasons-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-reasons-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-reasons-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-reasons-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("epistemic_action_reasons");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_context_affordance(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-affordance-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-affordance-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-affordance-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-affordance-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("context_affordance");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_context_gaps(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-context-gaps-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-context-gaps-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-context-gaps-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-context-gaps-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("context_gaps");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_trajectory_memory(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-trajectory-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-trajectory-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-trajectory-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-trajectory-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("trajectory_memory");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_attention(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-attention-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-attention-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-attention-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-attention-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("attention");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_expectation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-expectation-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-expectation-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-expectation-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-expectation-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("expectation");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_origin(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-origin-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-origin-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-origin-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-origin-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("origin");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_missing_entries(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entries-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entries-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entries-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entries-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("entries");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_result_rejects_context_packet_entry_missing_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entry-source-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entry-source-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entry-source-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-result-packet-entry-source-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg("--result-envelope")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["result"]["result"][0]["entries"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet entry"))?
        .remove("source");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-result")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("checkout query result envelope is invalid"));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_accepts_context_packets_artifact(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-validate-checkout-context-packets-store");
    let query_path = temp_store_path("continuitydb-cli-validate-checkout-context-packets-query");
    let report_path = temp_store_path("continuitydb-cli-validate-checkout-context-packets-report")
        .with_extension("json");
    let validation_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-validation")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    fs::write(&report_path, output)?;

    let validation_output = Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&validation_output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.checkout_query.context_packets_validation")
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(json["context_packet_count"].as_u64(), Some(1));
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );

    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    assert_eq!(validation_report["valid"].as_bool(), Some(true));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_line_without_matching_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-line-entry-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-line-entry-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-line-entry-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-line-entry-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["lines"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet lines"))?
        .push(Value::String(
            "Bare packet text without structured entry provenance.".to_string(),
        ));
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_origin(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-origin-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-origin-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-origin-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-origin-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("origin");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_origin_empty_anchors(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-anchors-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-anchors-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-anchors-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-anchors-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["origin"]["anchors"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_origin_blank_scope_payload(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-scope-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-scope-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-scope-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-scope-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["origin"]["scope"] = serde_json::json!({
        "Project": "   "
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_origin_blank_lifecycle_stage(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-lifecycle-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-lifecycle-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-lifecycle-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-blank-lifecycle-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["origin"]["lifecycle_stage"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_origin_empty_valid_time(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-valid-time-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-valid-time-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-valid-time-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-origin-empty-valid-time-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["origin"]["valid_time"] = serde_json::json!({});
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_compiler_policy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("compiler_policy");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_abstraction_level(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-abstraction-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-abstraction-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-abstraction-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-abstraction-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("abstraction_level");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_packet_strategy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-strategy-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-strategy-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-strategy-report")
            .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-strategy-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["strategy"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_compiler_policy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-policy-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-policy-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-policy-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-policy-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["compiler_policy"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_abstraction_level(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-abstraction-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-abstraction-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-abstraction-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-abstraction-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["abstraction_level"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_compiler_reason_tags(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("compiler_reason_tags");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_compiler_reason_tags(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-reasons-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-reasons-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-reasons-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-compiler-reasons-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["compiler_reason_tags"] = serde_json::json!(["   "]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_model_assisted_without_compiler_reason_tags(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-model-reasons-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-model-reasons-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-model-reasons-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-model-reasons-failure")
            .with_extension("json");
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let cell = test_cell("project:continuitydb:cli-query-model-assisted-reason-validation")?;
    let cell_id = cell.id;
    let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
    db.ingest_cells_at_with_commit_id(vec![cell], committed_at, CommitId::new())?;
    let proposal = ContextCompilerProposal::new(
        cell_id,
        ContextPacketStrategy::OperationalBrief,
        ContextAbstractionLevel::Brief,
        vec!["model-assisted-operational".to_string()],
        vec!["test://cli".to_string()],
    )?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_requirements(QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            })
            .with_compiler_policy(ContextCompilerPolicy::ModelAssisted)
            .with_compiler_proposals(vec![proposal])
            .with_return_shape(QueryReturnShape::ContextPacketsOnly),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["compiler_reason_tags"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_compiler_evidence_locators(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-compiler-evidence-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-compiler-evidence-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-compiler-evidence-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-compiler-evidence-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("compiler_evidence_locators");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_unsupported_compiler_evidence_locator(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-unsupported-compiler-evidence-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-unsupported-compiler-evidence-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-unsupported-compiler-evidence-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-unsupported-compiler-evidence-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["compiler_evidence_locators"] =
        serde_json::json!(["artifact://not/source-cell-evidence"]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_model_assisted_without_compiler_evidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-model-assisted-evidence-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-model-assisted-evidence-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-model-assisted-evidence-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-model-assisted-evidence-failure",
    )
    .with_extension("json");
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let cell = test_cell("project:continuitydb:cli-query-model-assisted-validation")?;
    let cell_id = cell.id;
    let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
    db.ingest_cells_at_with_commit_id(vec![cell], committed_at, CommitId::new())?;
    let proposal = ContextCompilerProposal::new(
        cell_id,
        ContextPacketStrategy::OperationalBrief,
        ContextAbstractionLevel::Brief,
        vec!["model-assisted-operational".to_string()],
        vec!["test://cli".to_string()],
    )?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_requirements(QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            })
            .with_compiler_policy(ContextCompilerPolicy::ModelAssisted)
            .with_compiler_proposals(vec![proposal])
            .with_return_shape(QueryReturnShape::ContextPacketsOnly),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["compiler_evidence_locators"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_citations(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-citations-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-citations-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-citations-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-citations-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet"))?
        .remove("citations");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_citations(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-citations-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-citations-query");
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-citations-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-citations-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["citations"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("missing packet citations"))?
        .push(serde_json::json!("   "));
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_entry_missing_confidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["entries"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing context packet entry"))?
        .remove("confidence");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_entry_out_of_range_confidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-confidence-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["entries"][0]["confidence"] = serde_json::json!(1.2);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_entry_negative_token_count(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-negative-token-count-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-negative-token-count-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-negative-token-count-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-negative-token-count-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["entries"][0]["token_count"] = serde_json::json!(-1);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_entry_blank_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-source-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-source-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-source-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-source-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["entries"][0]["source"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_entry_blank_text(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-text-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-text-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-text-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-entry-blank-text-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["lines"][0] = serde_json::json!("   ");
    report["context_packets"][0]["entries"][0]["text"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_dependency_context_missing_kind(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-dependency-kind-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-dependency-kind-query");
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-kind-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-kind-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing dependency context"))?
        .remove("kind");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_dependency_context_missing_anchors(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-anchors-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-anchors-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-anchors-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-anchors-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing dependency context"))?
        .remove("anchors");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_dependency_context_empty_anchors(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-anchors-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-anchors-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-anchors-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-anchors-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]["anchors"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_dependency_context_missing_citations(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-citations-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-citations-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-citations-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-citations-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing dependency context"))?
        .remove("citations");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_dependency_context_empty_citations(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-citations-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-citations-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-citations-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-empty-citations-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]["citations"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_dependency_context_blank_rationale(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-blank-rationale-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-blank-rationale-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-blank-rationale-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-blank-rationale-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]["rationale"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_unsupported_dependency_context_citation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-unsupported-citation-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-unsupported-citation-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-unsupported-citation-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-dependency-unsupported-citation-failure",
    )
    .with_extension("json");
    write_dependency_context_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "dependency-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND dependency_kind = depends_on
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["dependency_context"][0]["citations"] =
        serde_json::json!(["test://not-promoted-into-packet-citations"]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_revision_context_missing_relation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-relation-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-relation-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-relation-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-relation-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing revision context"))?
        .remove("relation");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_revision_context_out_of_range_confidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-confidence-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-confidence-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-confidence-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-confidence-range-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]["max_confidence"] = serde_json::json!(-0.1);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_revision_context_missing_anchors(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-anchors-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-anchors-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-anchors-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-anchors-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing revision context"))?
        .remove("anchors");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_revision_context_empty_anchors(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-anchors-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-anchors-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-anchors-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-anchors-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]["anchors"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_revision_context_empty_citations(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-citations-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-citations-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-citations-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-empty-citations-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]["citations"] = serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_revision_context_blank_relation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-blank-relation-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-blank-relation-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-blank-relation-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-blank-relation-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]["relation"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_unsupported_revision_context_citation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-unsupported-citation-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-unsupported-citation-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-unsupported-citation-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-revision-unsupported-citation-failure",
    )
    .with_extension("json");
    let (_selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
RETURN context_packets_only"#
        ),
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["revision_context"][0]["citations"] =
        serde_json::json!(["test://not-promoted-into-packet-citations"]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_answerability_questions(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-answerability-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-answerability-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-answerability-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-answerability-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("answerability_questions");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_answerability_questions(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-answerability-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-answerability-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-answerability-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-answerability-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["answerability_questions"] =
        serde_json::json!(["   "]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_selection_reasons(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-selection-reasons-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-selection-reasons-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-selection-reasons-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-blank-selection-reasons-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["reasons"] = serde_json::json!(["   "]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_context_gaps(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-context-gaps-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-context-gaps-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-context-gaps-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-context-gaps-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("context_gaps");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_context_gap_blank_question(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-blank-question-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-blank-question-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-blank-question-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-blank-question-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["context_gaps"] = serde_json::json!([
        {
            "kind": "MissingEvidence",
            "question": "   ",
            "rationale": "Need source evidence before using this packet.",
            "priority": 0.7
        }
    ]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_context_gap_out_of_range_priority(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-priority-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-priority-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-priority-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-context-gap-priority-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["context_gaps"] = serde_json::json!([
        {
            "kind": "MissingEvidence",
            "question": "Which retained artifact proves this packet?",
            "rationale": "Need source evidence before using this packet.",
            "priority": 1.2
        }
    ]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_invalidation_conditions(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-invalidation-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-invalidation-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-invalidation-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-invalidation-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("invalidation_conditions");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_invalidation_blank_condition(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-blank-condition-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-blank-condition-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-blank-condition-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-blank-condition-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["invalidation_conditions"] = serde_json::json!([
        {
            "kind": "ContradictoryEvidence",
            "condition": "   ",
            "rationale": "Contradictory deployment evidence should trigger revision.",
            "priority": 0.8
        }
    ]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_invalidation_out_of_range_priority(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-priority-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-priority-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-priority-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-invalidation-priority-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["invalidation_conditions"] = serde_json::json!([
        {
            "kind": "ContradictoryEvidence",
            "condition": "A retained artifact contradicts this packet.",
            "rationale": "Contradictory deployment evidence should trigger revision.",
            "priority": -0.1
        }
    ]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_trajectory_memory(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-trajectory-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-trajectory-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-trajectory-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-trajectory-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("trajectory_memory");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_without_trace_citation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-trace-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-trace-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-trace-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-trace-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    let citations = report["context_packets"][0]["citations"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("missing citations"))?;
    citations.retain(|citation| {
        citation.as_str() != Some("artifact://rollout/context-packet-trajectory")
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_missing_applicability(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-applicability-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-applicability-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-applicability-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-applicability-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing trajectory memory"))?
        .remove("applicability_conditions");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_missing_invalidation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-invalidation-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-invalidation-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-invalidation-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-invalidation-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing trajectory memory"))?
        .remove("invalidation_conditions");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_empty_applicability(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-applicability-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-applicability-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-applicability-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-applicability-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]["applicability_conditions"] =
        serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_empty_invalidation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-invalidation-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-invalidation-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-invalidation-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-empty-invalidation-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]["invalidation_conditions"] =
        serde_json::json!([]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_blank_hypothesis(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-hypothesis-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-hypothesis-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-hypothesis-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-hypothesis-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]["hypothesis_tried"] =
        serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_blank_reusable_lesson(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-lesson-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-lesson-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-lesson-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-lesson-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]["reusable_lesson"] =
        serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_out_of_range_confidence(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-confidence-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-confidence-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-confidence-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-confidence-range-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]["confidence"] =
        serde_json::json!(1.2);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_trajectory_memory_blank_checkout_strategy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-strategy-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-strategy-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-strategy-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-trajectory-blank-strategy-failure",
    )
    .with_extension("json");
    write_trajectory_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["trajectory_memory"]["checkout_strategy"] =
        serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_epistemic_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-pressure-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-pressure-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-pressure-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-pressure-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("epistemic_pressure");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("checkout_query_context_packets_validation")
    );
    assert_eq!(
        failure_report["checkout_query_context_packets_report"]["parseable"].as_bool(),
        Some(true)
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

fn assert_context_packets_rejects_epistemic_pressure_value(
    field: &str,
    value: Value,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-pressure-{label}-store"
    ));
    let query_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-pressure-{label}-query"
    ));
    let report_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-pressure-{label}-report"
    ))
    .with_extension("json");
    let failure_report_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-pressure-{label}-failure"
    ))
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["epistemic_pressure"][field] = value;
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_revision_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_context_packets_rejects_epistemic_pressure_value(
        "revision_pressure",
        serde_json::json!(1.1),
        "revision-range",
    )
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_scavenging_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_context_packets_rejects_epistemic_pressure_value(
        "scavenging_pressure",
        serde_json::json!(-0.1),
        "scavenging-range",
    )
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_checkout_pressure(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_context_packets_rejects_epistemic_pressure_value(
        "checkout_pressure",
        serde_json::json!(1.01),
        "checkout-range",
    )
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_epistemic_action_reasons(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-reasons-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("epistemic_action_reasons");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_epistemic_action_reasons(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-reasons-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-reasons-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-reasons-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-reasons-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["epistemic_action_reasons"] =
        serde_json::json!(["   "]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_blank_epistemic_action(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-action-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-action-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-action-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-blank-action-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["epistemic_action"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_lifecycle_policy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-policy-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("lifecycle_policy");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_lifecycle_policy_blank_reasons(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-reasons-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-reasons-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-reasons-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-reasons-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["lifecycle_policy"]["reasons"] =
        serde_json::json!(["   "]);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_lifecycle_policy_blank_use_policy(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-use-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-use-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-use-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-use-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["lifecycle_policy"]["use_policy"] =
        serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_lifecycle_policy_blank_promotion(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-promotion-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-promotion-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-promotion-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-policy-blank-promotion-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["lifecycle_policy"]["promotion"] =
        serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_context_affordance_score(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-affordance-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-affordance-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-affordance-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-affordance-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("context_affordance_score");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_attention(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-attention-range-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-attention-range-query");
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-attention-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-attention-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["attention"]["novelty"] = serde_json::json!(1.2);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_context_affordance(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["context_affordance"]["risk_of_misuse"] =
        serde_json::json!(-0.1);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_context_affordance_score(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-score-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-score-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-score-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-affordance-score-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["context_affordance_score"] = serde_json::json!(1.2);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_salience_score(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-salience-score-range-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-salience-score-range-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-salience-score-range-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-salience-score-range-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["salience_score"] = serde_json::json!(-0.1);
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_salience_score(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-salience-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-salience-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-salience-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-salience-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("salience_score");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_missing_expectation(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-expectation-store");
    let query_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-expectation-query");
    let report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-expectation-report")
            .with_extension("json");
    let failure_report_path =
        temp_store_path("continuitydb-cli-validate-checkout-context-packets-expectation-failure")
            .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("missing packet selection"))?
        .remove("expectation");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_expectation_blank_label(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-expectation-blank-label-store",
    );
    let query_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-expectation-blank-label-query",
    );
    let report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-expectation-blank-label-report",
    )
    .with_extension("json");
    let failure_report_path = temp_store_path(
        "continuitydb-cli-validate-checkout-context-packets-expectation-blank-label-failure",
    )
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"]["expectation"]["label"] = serde_json::json!("   ");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

fn assert_context_packets_rejects_selection_value(
    field: &str,
    value: Value,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-selection-{label}-store"
    ));
    let query_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-selection-{label}-query"
    ));
    let report_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-selection-{label}-report"
    ))
    .with_extension("json");
    let failure_report_path = temp_store_path(&format!(
        "continuitydb-cli-validate-checkout-context-packets-selection-{label}-failure"
    ))
    .with_extension("json");
    write_context_packet_store(&store_path)?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
RETURN context_packets_only"#,
    )?;
    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut report: Value = serde_json::from_slice(&output)?;
    report["context_packets"][0]["selection"][field] = value;
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-checkout-query-context-packets")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "checkout query context packets artifact is invalid",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_out_of_range_utility_score(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_context_packets_rejects_selection_value(
        "utility_score",
        serde_json::json!(1.01),
        "utility-score-range",
    )
}

#[test]
fn cli_validate_checkout_query_context_packets_rejects_negative_surprise_bits(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_context_packets_rejects_selection_value(
        "surprise_bits",
        serde_json::json!(-0.1),
        "negative-surprise-bits",
    )
}

#[test]
fn cli_checkout_query_executes_revision_link_text_constraints(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-revision-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-revision-query");
    let (selected_id, related_id) = write_revision_filter_store(&store_path)?;
    let selected_id_text = selected_id.to_string();
    let related_id_text = related_id.to_string();
    fs::write(
        &query_path,
        format!(
            r#"CHECKOUT "revision-review" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND revision_related_cell = "{related_id_text}"
  AND revision_link_kind = conflicts_with
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#
        ),
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["cells"][0]["id"].as_str(),
        Some(selected_id_text.as_str())
    );
    assert_eq!(
        json["cells"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-revision-selected")
    );
    assert_eq!(
        json["audit_traces"][0]["revision_context"][0]["related_cell_id"].as_str(),
        Some(related_id_text.as_str())
    );
    assert_eq!(
        json["audit_traces"][0]["revision_context"][0]["kind"].as_str(),
        Some("ConflictsWith")
    );
    assert_eq!(
        json["audit_traces"][0]["revision_context"][0]["citations"],
        serde_json::json!(["test://cli"])
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_rejects_invalid_query_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-invalid-envelope-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-invalid-envelope-query");
    write_committed_store(
        &store_path,
        "project:continuitydb:cli-query-invalid-envelope",
    )?;
    let envelope = QueryEnvelope {
        format: QUERY_ENVELOPE_FORMAT.to_string(),
        version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
        query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        ))),
    };
    fs::write(&query_path, serde_json::to_vec(&envelope)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .failure()
        .stderr(contains("query envelope is invalid"));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_checkout_query_rejects_unsupported_query_optimization(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-unsupported-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-unsupported-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-unsupported")?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_return_shape(QueryReturnShape::CellsOnly)
            .with_optimization(QueryOptimization::TokenCostOnly),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .failure()
        .stderr(contains("unsupported optimization"));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}

#[test]
fn cli_compact_file_rewrites_legacy_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact");
    write_legacy_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let records = fs::read_to_string(&path)?
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(json["compacted"].as_bool(), Some(true));
    assert_eq!(json["path"].as_str(), path.to_str());
    assert_eq!(records.len(), 3);
    assert!(records[0].contains(r#""type":"header""#));
    assert!(records[1].contains(r#""type":"cell""#));
    assert!(records[1].contains(r#""checksum":"continuitydb-fnv1a64:"#));
    assert!(records[2].contains(r#""type":"commit""#));
    assert!(records[2].contains(r#""checksum":"continuitydb-fnv1a64:"#));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_compact_file_if_needed_skips_canonical_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact-if-needed-canonical");

    let output = Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .arg("--if-needed")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["compacted"].as_bool(), Some(false));
    assert_eq!(
        json["after"]["compaction_recommended"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_compact_file_if_needed_compacts_legacy_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact-if-needed-legacy");
    write_legacy_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .arg("--if-needed")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["compacted"].as_bool(), Some(true));
    assert_eq!(
        json["before"]["compaction_recommended"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["after"]["compaction_recommended"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_compact_file_fails_for_corrupt_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact-corrupt");
    fs::write(&path, "{not valid json}\n")?;

    Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .assert()
        .failure();

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_file_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect");
    write_revision_link_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.inspect_kernel.report")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(json["path"].as_str(), path.to_str());
    assert!(json["required"].is_null());
    assert_eq!(json["satisfies"].as_bool(), Some(true));
    assert_eq!(
        json["capabilities"]["durability"].as_str(),
        Some("append-log")
    );
    assert_eq!(json["capabilities"]["append_only"].as_bool(), Some(true));
    assert_eq!(
        json["capabilities"]["derived_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["capabilities"]["persistent_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["capabilities"]["explicit_commit_records"].as_bool(),
        Some(true)
    );
    assert_eq!(json["capabilities"]["durable_flush"].as_bool(), Some(true));
    assert_eq!(json["capabilities"]["compaction"].as_bool(), Some(true));
    assert_eq!(json["status"]["cell_count"].as_u64(), Some(2));
    assert_eq!(json["status"]["commit_count"].as_u64(), Some(1));
    assert_eq!(json["status"]["revision_link_count"].as_u64(), Some(1));
    assert!(
        json["status"]["file_size_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert_eq!(json["health"]["has_header"].as_bool(), Some(true));
    assert_eq!(json["health"]["legacy_raw_cells"].as_u64(), Some(0));
    assert_eq!(json["health"]["checksum_free_records"].as_u64(), Some(0));
    assert_eq!(json["health"]["canonical_records"].as_u64(), Some(4));
    assert_eq!(
        json["health"]["compaction_recommended"].as_bool(),
        Some(false)
    );
    assert_eq!(
        json["health"]["persistent_index_checkpoint_present_on_open"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["health"]["persistent_index_checkpoint_trusted_on_open"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["health"]["persistent_index_checkpoint_rebuilt_on_open"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-lookup-plan");
    write_revision_link_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-plan")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["indexed_constraint_count"].as_u64(),
        Some(0)
    );
    assert_eq!(
        json["lookup_plan"]["indexed_constraints"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(json["lookup_plan"]["candidate_count"].as_u64(), Some(2));
    assert_eq!(json["lookup_plan"]["full_scan"].as_bool(), Some(true));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_query_lookup_plan() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-query-lookup-plan");
    write_revision_link_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE scope = project("continuitydb")"#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["indexed_constraint_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        json["lookup_plan"]["indexed_constraints"],
        serde_json::json!(["scope", "answerability_question"])
    );
    assert_eq!(
        json["lookup_plan"]["indexed_constraint_plans"],
        serde_json::json!([
            {
                "name": "scope",
                "candidate_count": 2
            },
            {
                "name": "answerability_question",
                "candidate_count": 2
            }
        ])
    );
    assert_eq!(json["lookup_plan"]["candidate_count"].as_u64(), Some(2));
    assert_eq!(json["lookup_plan"]["full_scan"].as_bool(), Some(false));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan_exact_match_count(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-exact-lookup-plan");
    let first_valid = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_expired = Utc
        .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let as_of = Utc
        .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut expired = test_cell("project:continuitydb:cli-expired")?;
    expired.valid_time = ValidTimeRange::new(first_valid, Some(first_expired))?;
    let mut current = test_cell("project:continuitydb:cli-current")?;
    current.valid_time = ValidTimeRange::new(first_valid, None)?;
    let mut db = ContinuityDb::new(FileKernel::open(&path)?);
    db.ingest_cells_at_with_commit_id(vec![expired, current], as_of, CommitId::new())?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE valid_at = "2026-05-22T00:00:00Z""#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["indexed_constraints"],
        serde_json::json!(["answerability_question", "valid_at"])
    );
    assert_eq!(json["lookup_plan"]["candidate_count"].as_u64(), Some(2));
    assert_eq!(json["lookup_plan"]["exact_match_count"].as_u64(), Some(1));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan_filtered_candidate_count(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-filtered-lookup-plan");
    let first_valid = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_expired = Utc
        .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let as_of = Utc
        .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut expired = test_cell("project:continuitydb:cli-filtered-expired")?;
    expired.valid_time = ValidTimeRange::new(first_valid, Some(first_expired))?;
    let mut current = test_cell("project:continuitydb:cli-filtered-current")?;
    current.valid_time = ValidTimeRange::new(first_valid, None)?;
    let mut db = ContinuityDb::new(FileKernel::open(&path)?);
    db.ingest_cells_at_with_commit_id(vec![expired, current], as_of, CommitId::new())?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE valid_at = "2026-05-22T00:00:00Z""#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["lookup_plan"]["candidate_count"].as_u64(), Some(2));
    assert_eq!(json["lookup_plan"]["exact_match_count"].as_u64(), Some(1));
    assert_eq!(
        json["lookup_plan"]["filtered_candidate_count"].as_u64(),
        Some(1)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan_candidate_selectivity(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-selectivity-lookup-plan");
    let first_valid = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_expired = Utc
        .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let as_of = Utc
        .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut expired = test_cell("project:continuitydb:cli-selectivity-expired")?;
    expired.valid_time = ValidTimeRange::new(first_valid, Some(first_expired))?;
    let mut current = test_cell("project:continuitydb:cli-selectivity-current")?;
    current.valid_time = ValidTimeRange::new(first_valid, None)?;
    let mut db = ContinuityDb::new(FileKernel::open(&path)?);
    db.ingest_cells_at_with_commit_id(vec![expired, current], as_of, CommitId::new())?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE valid_at = "2026-05-22T00:00:00Z""#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["candidate_selectivity_basis_points"].as_u64(),
        Some(5000)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan_exact_constraint_labels(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-exact-constraint-lookup-plan");
    write_revision_link_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE scope = project("continuitydb")"#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["exact_constraint_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        json["lookup_plan"]["exact_constraints"],
        serde_json::json!(["scope", "answerability_question"])
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan_residual_exact_constraints(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-residual-exact-lookup-plan");
    let first_valid = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let as_of = Utc
        .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut current = test_cell("project:continuitydb:cli-residual-current")?;
    current.valid_time = ValidTimeRange::new(first_valid, None)?;
    let mut db = ContinuityDb::new(FileKernel::open(&path)?);
    db.ingest_cells_at_with_commit_id(vec![current], as_of, CommitId::new())?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE valid_at = "2026-05-22T00:00:00Z"
  AND scope = project("continuitydb")"#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["residual_exact_constraint_count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        json["lookup_plan"]["residual_exact_constraints"],
        serde_json::json!(["valid_at"])
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_lookup_plan_lossy_indexed_constraints(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-lossy-lookup-plan");
    let first_valid = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let as_of = Utc
        .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut current = test_cell("project:continuitydb:cli-lossy-current")?;
    current.valid_time = ValidTimeRange::new(first_valid, None)?;
    let mut db = ContinuityDb::new(FileKernel::open(&path)?);
    db.ingest_cells_at_with_commit_id(vec![current], as_of, CommitId::new())?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--lookup-query")
        .arg(
            r#"CHECKOUT "inspect" ANSWER "what is stored?"
WHERE system_at = "2026-05-22T00:00:00Z"
  AND valid_at = "2026-05-22T00:00:00Z"
  AND scope = project("continuitydb")"#,
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["lookup_plan"]["lossy_indexed_constraint_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        json["lookup_plan"]["lossy_indexed_constraints"],
        serde_json::json!(["system_at", "valid_at"])
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_legacy_health() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-legacy-health");
    write_legacy_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["health"]["has_header"].as_bool(), Some(false));
    assert_eq!(json["health"]["legacy_raw_cells"].as_u64(), Some(1));
    assert_eq!(
        json["health"]["compaction_recommended"].as_bool(),
        Some(true)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_reports_rebuilt_persistent_index_checkpoint(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-rebuilt-index-health");
    write_revision_link_store(&path)?;
    let index_path = persistent_index_path_for_test(&path);
    let mut checkpoint: Value = serde_json::from_str(&fs::read_to_string(&index_path)?)?;
    checkpoint["cells"][0]["checksum"] = Value::from("corrupt");
    fs::write(&index_path, serde_json::to_string_pretty(&checkpoint)?)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["health"]["persistent_index_checkpoint_present_on_open"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["health"]["persistent_index_checkpoint_trusted_on_open"].as_bool(),
        Some(false)
    );
    assert_eq!(
        json["health"]["persistent_index_checkpoint_rebuilt_on_open"].as_bool(),
        Some(true)
    );
    assert_eq!(json["status"]["cell_count"].as_u64(), Some(2));
    assert_eq!(json["status"]["revision_link_count"].as_u64(), Some(1));

    fs::remove_file(path)?;
    fs::remove_file(index_path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_accepts_canonical_requirement() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-canonical");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require-canonical")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["health"]["compaction_recommended"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_rejects_legacy_when_canonical_required(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-canonical-legacy");
    write_legacy_store(&path)?;

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require-canonical")
        .assert()
        .failure()
        .stderr(contains("file store compaction is recommended"));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_accepts_durable_append_log_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-durable");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("durable-append-log")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["required"].as_str(), Some("durable-append-log"));
    assert_eq!(json["satisfies"].as_bool(), Some(true));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_requirement_output_matches_native_gate(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-native-gate");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("durable-append-log")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let db = ContinuityDb::open_file_with_requirements(
        &path,
        continuitydb_kernel::KernelRequirements::durable_append_log(),
    )?;

    assert_eq!(json["required"].as_str(), Some("durable-append-log"));
    assert_eq!(json["satisfies"].as_bool(), Some(true));
    assert_eq!(db.kernel().path(), path.as_path());

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_accepts_persistent_indexed_append_log_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-persistent-indexed-append-log");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["required"].as_str(),
        Some("persistent-indexed-append-log")
    );
    assert_eq!(json["satisfies"].as_bool(), Some(true));
    assert_eq!(
        json["capabilities"]["durability"].as_str(),
        Some("append-log")
    );
    assert_eq!(
        json["capabilities"]["persistent_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["required_capabilities"]["minimum_durability"].as_str(),
        Some("append-log")
    );
    assert_eq!(
        json["required_capabilities"]["append_only"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["required_capabilities"]["derived_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["required_capabilities"]["persistent_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["required_capabilities"]["explicit_commit_records"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["required_capabilities"]["durable_flush"].as_bool(),
        Some(true)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_rejects_indexed_embedded_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-indexed");
    let report_path = temp_store_path("continuitydb-cli-inspect-require-indexed-report");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("indexed-embedded")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .failure()
        .stderr(contains("storage kernel requirements are not met"))
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;

    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert!(json["report_payload_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["report_payload_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(json["required"].as_str(), Some("indexed-embedded"));
    assert_eq!(json["satisfies"].as_bool(), Some(false));
    assert_eq!(
        json["capabilities"]["durability"].as_str(),
        Some("append-log")
    );
    assert_eq!(
        json["capabilities"]["persistent_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["required_capabilities"]["minimum_durability"].as_str(),
        Some("indexed-embedded")
    );
    assert_eq!(
        json["required_capabilities"]["persistent_indexes"].as_bool(),
        Some(true)
    );
    assert_eq!(report_json, json);

    fs::remove_file(path)?;
    fs::remove_file(report_path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_writes_artifact_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-bundle-source");
    let artifact_dir = temp_store_path("continuitydb-cli-inspect-bundle-artifacts");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let report_path = artifact_dir.join("inspect-kernel-report.json");
    let manifest_path = artifact_dir.join("continuitydb-inspect-kernel.manifest.json");
    let report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let manifest_json: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;

    assert_eq!(report_json, json);
    assert_eq!(
        json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["bundle_manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        manifest_json["format"].as_str(),
        Some("continuitydb.inspect_kernel.bundle")
    );
    assert_eq!(manifest_json["format_version"].as_u64(), Some(1));
    assert_eq!(
        manifest_json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        manifest_json["inspect_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        manifest_json["inspected_store_path"].as_str(),
        Some(path.display().to_string().as_str())
    );
    assert_eq!(
        manifest_json["required"].as_str(),
        Some("persistent-indexed-append-log")
    );
    assert_eq!(manifest_json["satisfies"].as_bool(), Some(true));

    let mut canonical_report = report_json.clone();
    canonical_report["bundle_manifest"] = Value::Null;
    canonical_report
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be an object"))?
        .remove("report_payload_fingerprint");
    canonical_report
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be an object"))?
        .remove("report_payload_bytes");
    let canonical_report_text = serde_json::to_string_pretty(&canonical_report)?;
    assert_eq!(
        manifest_json["inspect_report_fingerprint"].as_str(),
        Some(test_fnv1a64_fingerprint(&canonical_report_text).as_str())
    );
    assert_eq!(
        manifest_json["inspect_report_bytes"].as_u64(),
        Some(canonical_report_text.len() as u64)
    );

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-report")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    fs::remove_file(path)?;
    fs::remove_dir_all(artifact_dir)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_bundle_accepts_valid_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-bundle-validate-source");
    let artifact_dir = temp_store_path("continuitydb-cli-inspect-bundle-validate-artifacts");
    let validation_report_path = temp_store_path("continuitydb-cli-inspect-bundle-validate-report");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    let report_path = artifact_dir.join("inspect-kernel-report.json");
    let manifest_path = artifact_dir.join("continuitydb-inspect-kernel.manifest.json");

    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.inspect_kernel.bundle_validation")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["validation_report_path"].as_str(),
        Some(validation_report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["format"].as_str(),
        Some("continuitydb.inspect_kernel.bundle")
    );
    assert_eq!(json["manifest"]["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["manifest"]["self_described_artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["self_described_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["inspected_store_path"].as_str(),
        Some(path.display().to_string().as_str())
    );
    assert_eq!(
        json["manifest"]["required"].as_str(),
        Some("persistent-indexed-append-log")
    );
    assert_eq!(json["manifest"]["satisfies"].as_bool(), Some(true));
    assert_eq!(
        json["inspect_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["inspect_report"]["self_described_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["inspect_report"]["self_described_artifact_dir"].as_str(),
        Some(artifact_dir.display().to_string().as_str())
    );
    assert_eq!(
        json["inspect_report"]["inspected_store_path"].as_str(),
        Some(path.display().to_string().as_str())
    );
    assert_eq!(
        json["inspect_report"]["format"].as_str(),
        Some("continuitydb.inspect_kernel.report")
    );
    assert_eq!(json["inspect_report"]["format_version"].as_u64(), Some(1));
    assert_eq!(json["inspect_report"]["parseable"].as_bool(), Some(true));
    assert_eq!(json["inspect_report"]["parse_error"].as_str(), None);
    assert!(json["inspect_report"]["report_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["inspect_report"]["report_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert_eq!(json["inspect_report"]["valid"].as_bool(), Some(true));
    assert!(json["inspect_report"]["report_payload_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(validation_report, json);

    fs::remove_file(path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_bundle_rejects_report_artifact_dir_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-bundle-report-dir-source");
    let artifact_dir = temp_store_path("continuitydb-cli-inspect-bundle-report-dir-artifacts");
    let failure_report_path = temp_store_path("continuitydb-cli-inspect-bundle-report-dir-failure");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let report_path = artifact_dir.join("inspect-kernel-report.json");
    let manifest_path = artifact_dir.join("continuitydb-inspect-kernel.manifest.json");
    let mut report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    report_json["artifact_dir"] = Value::from("/tmp/wrong-inspect-artifact-dir");
    report_json["bundle_manifest"] = Value::Null;
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be an object"))?
        .remove("report_payload_fingerprint");
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be an object"))?
        .remove("report_payload_bytes");
    let report_payload = serde_json::to_string_pretty(&report_json)?;
    report_json["report_payload_fingerprint"] =
        Value::from(test_fnv1a64_fingerprint(&report_payload));
    report_json["report_payload_bytes"] = Value::from(report_payload.len());
    fs::write(&report_path, serde_json::to_string_pretty(&report_json)?)?;

    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest["inspect_report_fingerprint"] = Value::from(test_fnv1a64_fingerprint(&report_payload));
    manifest["inspect_report_bytes"] = Value::from(report_payload.len());
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "inspect kernel bundle report artifact directory mismatch",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["inspect_report"]["self_described_artifact_dir"].as_str(),
        Some("/tmp/wrong-inspect-artifact-dir")
    );
    assert_eq!(
        failure_report["failure"]["message"].as_str(),
        Some("inspect kernel bundle report artifact directory mismatch")
    );

    fs::remove_file(path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_bundle_rejects_manifest_content_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-bundle-invalid-source");
    let artifact_dir = temp_store_path("continuitydb-cli-inspect-bundle-invalid-artifacts");
    let failure_report_path = temp_store_path("continuitydb-cli-inspect-bundle-invalid-failure");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .assert()
        .success();

    let manifest_path = artifact_dir.join("continuitydb-inspect-kernel.manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest["inspected_store_path"] = Value::from("/tmp/wrong-inspected-store.jsonl");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-bundle")
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "inspect kernel bundle manifest report content mismatch: inspected_store_path",
        ));

    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    assert_eq!(
        failure_report["format"].as_str(),
        Some("continuitydb.inspect_kernel.bundle_validation")
    );
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["failure"]["stage"].as_str(),
        Some("inspect_kernel_bundle_validation")
    );
    assert_eq!(
        failure_report["manifest"]["manifest_path"].as_str(),
        Some(manifest_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["manifest"]["parseable"].as_bool(),
        Some(true)
    );
    assert_eq!(
        failure_report["inspect_report"]["inspected_store_path"].as_str(),
        Some(path.display().to_string().as_str())
    );

    fs::remove_file(path)?;
    fs::remove_dir_all(artifact_dir)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_report_accepts_valid_report(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-validate-source");
    let report_path = temp_store_path("continuitydb-cli-inspect-validate-report");
    let validation_report_path =
        temp_store_path("continuitydb-cli-inspect-validate-success-report");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--validation-report-path")
        .arg(&validation_report_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let validation_report: Value = serde_json::from_slice(&fs::read(&validation_report_path)?)?;
    let report_text = fs::read_to_string(&report_path)?;
    let report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;

    assert_eq!(
        json["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["format"].as_str(),
        Some("continuitydb.inspect_kernel.validation")
    );
    assert_eq!(json["format_version"].as_u64(), Some(1));
    assert_eq!(
        json["validation_report_path"].as_str(),
        Some(validation_report_path.display().to_string().as_str())
    );
    assert_eq!(json["valid"].as_bool(), Some(true));
    assert_eq!(
        json["report_payload_fingerprint"],
        report_json["report_payload_fingerprint"]
    );
    assert_eq!(
        json["report_payload_bytes"],
        report_json["report_payload_bytes"]
    );
    assert_eq!(
        json["inspected_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["inspected_report"]["self_described_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        json["inspected_report"]["inspected_store_path"].as_str(),
        Some(path.display().to_string().as_str())
    );
    assert_eq!(
        json["inspected_report"]["report_fingerprint"].as_str(),
        Some(test_fnv1a64_fingerprint(&report_text).as_str())
    );
    assert_eq!(
        json["inspected_report"]["report_bytes"].as_u64(),
        Some(report_text.len() as u64)
    );
    assert_eq!(json["inspected_report"]["parseable"].as_bool(), Some(true));
    assert_eq!(json["inspected_report"]["parse_error"].as_str(), None);
    assert_eq!(
        json["inspected_report"]["format"].as_str(),
        Some("continuitydb.inspect_kernel.report")
    );
    assert_eq!(json["inspected_report"]["format_version"].as_u64(), Some(1));
    assert_eq!(validation_report, json);

    fs::remove_file(path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(validation_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_report_rejects_wrong_report_format(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-validate-wrong-format-source");
    let report_path = temp_store_path("continuitydb-cli-inspect-validate-wrong-format-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-inspect-validate-wrong-format-failure-report");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let mut report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    report_json["format"] = Value::String("continuitydb.other.report".to_string());
    report_json["report_payload_fingerprint"] = Value::Null;
    report_json["report_payload_bytes"] = Value::Null;
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be object"))?
        .remove("report_payload_fingerprint");
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be object"))?
        .remove("report_payload_bytes");
    let report_payload = serde_json::to_string_pretty(&report_json)?;
    report_json["report_payload_fingerprint"] =
        Value::String(test_fnv1a64_fingerprint(&report_payload));
    report_json["report_payload_bytes"] = Value::from(report_payload.len());
    fs::write(&report_path, serde_json::to_string_pretty(&report_json)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("unsupported inspect kernel report format"));
    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;

    assert_eq!(
        failure_report["error"].as_str(),
        Some("unsupported inspect kernel report format")
    );

    fs::remove_file(path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_report_rejects_missing_persistent_index_health(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-validate-missing-index-health-source");
    let report_path =
        temp_store_path("continuitydb-cli-inspect-validate-missing-index-health-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-inspect-validate-missing-index-health-failure-report");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let mut report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    report_json["health"]
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report health must be object"))?
        .remove("persistent_index_checkpoint_trusted_on_open");
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be object"))?
        .remove("report_payload_fingerprint");
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be object"))?
        .remove("report_payload_bytes");
    let report_payload = serde_json::to_string_pretty(&report_json)?;
    report_json["report_payload_fingerprint"] =
        Value::String(test_fnv1a64_fingerprint(&report_payload));
    report_json["report_payload_bytes"] = Value::from(report_payload.len());
    fs::write(&report_path, serde_json::to_string_pretty(&report_json)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains(
            "inspect kernel report missing persistent-index checkpoint health",
        ));
    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;

    assert_eq!(
        failure_report["error"].as_str(),
        Some("inspect kernel report missing persistent-index checkpoint health")
    );

    fs::remove_file(path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_report_rejects_report_path_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-validate-path-mismatch-source");
    let report_path = temp_store_path("continuitydb-cli-inspect-validate-path-mismatch-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-inspect-validate-path-mismatch-failure-report");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let mut report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    report_json["report_path"] = Value::String("/tmp/continuitydb-wrong-report-path.json".into());
    report_json["report_payload_fingerprint"] = Value::Null;
    report_json["report_payload_bytes"] = Value::Null;
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be object"))?
        .remove("report_payload_fingerprint");
    report_json
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("inspect report must be object"))?
        .remove("report_payload_bytes");
    let report_payload = serde_json::to_string_pretty(&report_json)?;
    report_json["report_payload_fingerprint"] =
        Value::String(test_fnv1a64_fingerprint(&report_payload));
    report_json["report_payload_bytes"] = Value::from(report_payload.len());
    fs::write(&report_path, serde_json::to_string_pretty(&report_json)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("inspect kernel report path mismatch"));
    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;

    assert_eq!(
        failure_report["error"].as_str(),
        Some("inspect kernel report path mismatch")
    );

    fs::remove_file(path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_validate_inspect_kernel_report_rejects_tampered_payload(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-validate-tampered-source");
    let report_path = temp_store_path("continuitydb-cli-inspect-validate-tampered-report");
    let failure_report_path =
        temp_store_path("continuitydb-cli-inspect-validate-tampered-failure-report");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("persistent-indexed-append-log")
        .arg("--report-path")
        .arg(&report_path)
        .assert()
        .success();

    let mut report_json: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    report_json["satisfies"] = Value::Bool(false);
    fs::write(&report_path, serde_json::to_string_pretty(&report_json)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("validate-inspect-kernel-report")
        .arg("--report-path")
        .arg(&report_path)
        .arg("--failure-report-path")
        .arg(&failure_report_path)
        .assert()
        .failure()
        .stderr(contains("inspect kernel report fingerprint mismatch"));
    let failure_report: Value = serde_json::from_slice(&fs::read(&failure_report_path)?)?;
    let tampered_report_text = fs::read_to_string(&report_path)?;

    assert_eq!(
        failure_report["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["format"].as_str(),
        Some("continuitydb.inspect_kernel.validation")
    );
    assert_eq!(failure_report["format_version"].as_u64(), Some(1));
    assert_eq!(
        failure_report["failure_report_path"].as_str(),
        Some(failure_report_path.display().to_string().as_str())
    );
    assert_eq!(failure_report["valid"].as_bool(), Some(false));
    assert_eq!(
        failure_report["error"].as_str(),
        Some("inspect kernel report fingerprint mismatch")
    );
    assert_eq!(
        failure_report["inspected_report"]["report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
    assert_eq!(
        failure_report["inspected_report"]["report_fingerprint"].as_str(),
        Some(test_fnv1a64_fingerprint(&tampered_report_text).as_str())
    );
    assert_eq!(
        failure_report["inspected_report"]["report_bytes"].as_u64(),
        Some(tampered_report_text.len() as u64)
    );
    assert_eq!(
        failure_report["inspected_report"]["parseable"].as_bool(),
        Some(true)
    );
    assert_eq!(
        failure_report["inspected_report"]["parse_error"].as_str(),
        None
    );

    fs::remove_file(path)?;
    fs::remove_file(report_path)?;
    fs::remove_file(failure_report_path)?;
    Ok(())
}

#[test]
fn cli_exports_and_imports_commit_backup() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-source");
    let target_path = temp_store_path("continuitydb-cli-import-target");
    let backup_path = temp_store_path("continuitydb-cli-export-backup");
    write_revision_link_store(&source_path)?;

    let export_output = Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let export_json: Value = serde_json::from_slice(&export_output)?;
    let envelope_json: Value = serde_json::from_slice(&fs::read(&backup_path)?)?;

    assert_eq!(export_json["path"].as_str(), source_path.to_str());
    assert_eq!(export_json["output"].as_str(), backup_path.to_str());
    assert_eq!(export_json["exported_commits"].as_u64(), Some(1));
    assert!(export_json["next_after"].is_string());
    assert_eq!(
        envelope_json["format"].as_str(),
        Some("continuitydb.commit_export")
    );
    assert_eq!(envelope_json["version"].as_u64(), Some(1));

    let import_output = Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let import_json: Value = serde_json::from_slice(&import_output)?;
    let source_batch = ContinuityDb::new(FileKernel::open(&source_path)?)
        .export_commits(CommitManifestLookup::default())?;
    let source_db = ContinuityDb::new(FileKernel::open(&source_path)?);
    let target_db = ContinuityDb::new(FileKernel::open(&target_path)?);
    let target_batch = target_db.export_commits(CommitManifestLookup::default())?;

    assert_eq!(import_json["path"].as_str(), target_path.to_str());
    assert_eq!(import_json["input"].as_str(), backup_path.to_str());
    assert_eq!(import_json["imported_commits"].as_u64(), Some(1));
    assert_eq!(import_json["next_after"], export_json["next_after"]);
    assert_eq!(target_batch, source_batch);
    assert_eq!(
        target_db.list_revision_links(RevisionLinkLookup::default())?,
        source_db.list_revision_links(RevisionLinkLookup::default())?
    );

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_export_commits_limit_outputs_first_page_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-limit-source");
    let backup_path = temp_store_path("continuitydb-cli-export-limit-backup");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--limit")
        .arg("1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let batch = ContinuityDb::<FileKernel>::decode_commit_export_json(&fs::read(&backup_path)?)?;

    assert_eq!(json["exported_commits"].as_u64(), Some(1));
    assert_eq!(
        json["next_after"].as_str(),
        Some(commits[0].to_string().as_str())
    );
    assert_eq!(batch.slices.len(), 1);
    assert_eq!(batch.slices[0].manifest.commit_id, commits[0]);

    fs::remove_file(source_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_export_commits_after_cursor_outputs_next_page() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-after-source");
    let backup_path = temp_store_path("continuitydb-cli-export-after-backup");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--after")
        .arg(commits[0].to_string())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let batch = ContinuityDb::<FileKernel>::decode_commit_export_json(&fs::read(&backup_path)?)?;

    assert_eq!(json["exported_commits"].as_u64(), Some(1));
    assert_eq!(
        json["next_after"].as_str(),
        Some(commits[1].to_string().as_str())
    );
    assert_eq!(batch.slices.len(), 1);
    assert_eq!(batch.slices[0].manifest.commit_id, commits[1]);

    fs::remove_file(source_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_export_commits_fails_for_unknown_after_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-after-unknown-source");
    let backup_path = temp_store_path("continuitydb-cli-export-after-unknown-backup");
    write_committed_store(
        &source_path,
        "project:continuitydb:cli-export-after-unknown",
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--after")
        .arg(CommitId::new().to_string())
        .assert()
        .failure();

    fs::remove_file(source_path)?;
    let _ = fs::remove_file(backup_path);
    Ok(())
}

#[test]
fn cli_export_commits_rejects_invalid_after_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-after-invalid-source");
    let backup_path = temp_store_path("continuitydb-cli-export-after-invalid-backup");
    write_committed_store(
        &source_path,
        "project:continuitydb:cli-export-after-invalid",
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--after")
        .arg("not-a-uuid")
        .assert()
        .failure();

    fs::remove_file(source_path)?;
    let _ = fs::remove_file(backup_path);
    Ok(())
}

#[test]
fn cli_copy_commits_limit_copies_first_page() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-limit-source");
    let target_path = temp_store_path("continuitydb-cli-copy-limit-target");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .arg("--limit")
        .arg("1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["source"].as_str(), source_path.to_str());
    assert_eq!(json["target"].as_str(), target_path.to_str());
    assert_eq!(json["copied_commits"].as_u64(), Some(1));
    assert_eq!(
        json["next_after"].as_str(),
        Some(commits[0].to_string().as_str())
    );
    assert_eq!(target_batch.slices.len(), 1);
    assert_eq!(target_batch.slices[0].manifest.commit_id, commits[0]);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_after_cursor_copies_next_page() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-after-source");
    let target_path = temp_store_path("continuitydb-cli-copy-after-target");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .arg("--after")
        .arg(commits[0].to_string())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["copied_commits"].as_u64(), Some(1));
    assert_eq!(
        json["next_after"].as_str(),
        Some(commits[1].to_string().as_str())
    );
    assert_eq!(target_batch.slices.len(), 1);
    assert_eq!(target_batch.slices[0].manifest.commit_id, commits[1]);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_empty_source_reports_zero() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-empty-source");
    let target_path = temp_store_path("continuitydb-cli-copy-empty-target");

    let output = Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["copied_commits"].as_u64(), Some(0));
    assert!(json["next_after"].is_null());
    assert!(target_batch.slices.is_empty());

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_rejects_duplicate_target_commit() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-duplicate-source");
    let target_path = temp_store_path("continuitydb-cli-copy-duplicate-target");
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(FileKernel::open(&source_path)?);
    source.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-copy-duplicate-source")?],
        committed_at,
        commit_id,
    )?;
    let mut target = ContinuityDb::new(FileKernel::open(&target_path)?);
    target.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-copy-duplicate-target")?],
        committed_at,
        commit_id,
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .assert()
        .failure();
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(target_batch.slices.len(), 1);
    assert_eq!(target_batch.slices[0].manifest.commit_id, commit_id);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_rejects_invalid_after_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-invalid-source");
    let target_path = temp_store_path("continuitydb-cli-copy-invalid-target");

    Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .arg("--after")
        .arg("not-a-uuid")
        .assert()
        .failure();

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(target_path);
    Ok(())
}

#[test]
fn cli_import_commits_dry_run_validates_without_mutation() -> Result<(), Box<dyn std::error::Error>>
{
    let source_path = temp_store_path("continuitydb-cli-dry-run-source");
    let target_path = temp_store_path("continuitydb-cli-dry-run-target");
    let backup_path = temp_store_path("continuitydb-cli-dry-run-backup");
    write_committed_store(&source_path, "project:continuitydb:cli-dry-run")?;
    Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .arg("--dry-run")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["path"].as_str(), target_path.to_str());
    assert_eq!(json["input"].as_str(), backup_path.to_str());
    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["valid_commits"].as_u64(), Some(1));
    assert!(json["imported_commits"].is_null());
    assert_eq!(target_batch.slices.len(), 0);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_import_commits_dry_run_fails_for_invalid_envelope() -> Result<(), Box<dyn std::error::Error>>
{
    let target_path = temp_store_path("continuitydb-cli-dry-run-invalid-target");
    let backup_path = temp_store_path("continuitydb-cli-dry-run-invalid-backup");
    fs::write(&backup_path, "{not valid json}\n")?;

    Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .arg("--dry-run")
        .assert()
        .failure();

    let _ = fs::remove_file(target_path);
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_import_commits_fails_for_invalid_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let target_path = temp_store_path("continuitydb-cli-import-invalid-target");
    let backup_path = temp_store_path("continuitydb-cli-import-invalid-backup");
    fs::write(&backup_path, "{not valid json}\n")?;

    Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .assert()
        .failure();

    let _ = fs::remove_file(target_path);
    fs::remove_file(backup_path)?;
    Ok(())
}

fn temp_store_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
}

fn persistent_index_path_for_test(path: &std::path::Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "continuitydb.jsonl".to_string());
    path.with_file_name(format!("{file_name}.index.json"))
}

fn test_sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_sample_release_assets_manifest(
    artifact_dir: &std::path::Path,
    version: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let packages = [
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
    let mut assets = Vec::new();
    for package in packages {
        let name = format!("{package}-{version}.crate");
        let path = artifact_dir.join(&name);
        let bytes = format!("archive:{package}:{version}").into_bytes();
        fs::write(&path, &bytes)?;
        assets.push(serde_json::json!({
            "kind": "crate_archive",
            "package": package,
            "name": name,
            "path": path.display().to_string(),
            "bytes": bytes.len(),
            "sha256": test_sha256_hex(&bytes),
        }));
    }
    for name in [
        "proof-obligations.json",
        "proof-obligations-validation.json",
    ] {
        let path = artifact_dir.join(name);
        let bytes = format!("evidence:{name}").into_bytes();
        fs::write(&path, &bytes)?;
        assets.push(serde_json::json!({
            "kind": "release_preflight_evidence",
            "name": name,
            "path": path.display().to_string(),
            "bytes": bytes.len(),
            "sha256": test_sha256_hex(&bytes),
        }));
    }
    Ok(serde_json::json!({
        "format": "continuitydb.release_assets",
        "format_version": 1,
        "generated_by": "scripts/release_preflight.sh",
        "package_version": version,
        "asset_count": assets.len(),
        "assets": assets,
    }))
}

fn write_sample_release_upload_report(
    manifest_path: &std::path::Path,
    tag: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let manifest_text = fs::read_to_string(manifest_path)?;
    let manifest: Value = serde_json::from_str(&manifest_text)?;
    let assets = manifest["assets"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("sample manifest missing assets"))?
        .clone();
    Ok(serde_json::json!({
        "format": "continuitydb.release_upload",
        "format_version": 1,
        "generated_by": "scripts/upload_release_assets.sh",
        "valid": true,
        "release_tag": tag,
        "release_repository": "syndicat/continuitydb",
        "asset_count": assets.len(),
        "manifest": {
            "manifest_path": manifest_path.display().to_string(),
            "manifest_format": manifest["format"].clone(),
            "manifest_format_version": manifest["format_version"].clone(),
            "manifest_generated_by": manifest["generated_by"].clone(),
            "manifest_package_version": manifest["package_version"].clone(),
            "manifest_bytes": manifest_text.len(),
            "manifest_sha256": test_sha256_hex(manifest_text.as_bytes()),
            "manifest_asset_count": assets.len(),
        },
        "assets": assets,
    }))
}

fn sample_release_upload_test_report_json() -> Value {
    serde_json::json!({
        "format": "continuitydb.release_upload_test",
        "format_version": 1,
        "generated_by": "scripts/test_upload_release_assets.sh",
        "valid": true,
        "release_tag": "v0.1.0",
        "release_repository": "syndicat/continuitydb",
        "release_view_preflight_checked": true,
        "release_upload_repo_checked": true,
        "upload_failure_diagnostic_checked": true,
        "release_view_failure_diagnostic_checked": true,
        "asset_integrity_checked": true,
        "duplicate_manifest_checked": true,
        "success_report_checked": true,
        "success_report_validation_checked": true,
        "failure_report_checked": true,
        "failure_report_validation_checked": true,
        "release_view_failure_report_checked": true,
        "release_view_failure_report_validation_checked": true,
        "mocked_gh_invocation_count": 5,
    })
}

fn sample_ci_artifact_inventory_json() -> Value {
    let alpha_artifacts = vec![
        "target/alpha-workflow/proof-obligations.json",
        "target/alpha-workflow/proof-obligations-validation.json",
        "target/alpha-workflow/context-collapse-drill.json",
        "target/alpha-workflow/workload/workload-report.json",
        "target/alpha-workflow/workload/workload-validation.json",
        "target/alpha-workflow/workload/continuitydb-workload.manifest.json",
        "target/alpha-workflow/workload/workload-cells.json",
        "target/alpha-workflow/workload/checkout-request.json",
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
        "target/alpha-workflow/replay/replay-report.json",
        "target/alpha-workflow/replay/continuitydb-workload-replay.manifest.json",
        "target/alpha-workflow/inspect/inspect-kernel-report.json",
        "target/alpha-workflow/inspect/continuitydb-inspect-kernel.manifest.json",
        "target/alpha-workflow/store.jsonl",
        "target/alpha-workflow/store.jsonl.index.json",
        "target/alpha-workflow/replay-store.jsonl",
        "target/alpha-workflow/replay-store.jsonl.index.json",
    ];
    let smoke_artifacts = vec![
        "target/local-model-quality-gate-smoke/quality-gate-plan.json",
        "target/local-model-quality-gate-smoke/quality-gate-plan-validation.json",
        "target/local-model-quality-gate-smoke/candidates.json",
        "target/local-model-quality-gate-smoke/candidates-validation.json",
        "target/local-model-quality-gate-smoke/acceptance-criteria.json",
        "target/local-model-quality-gate-smoke/acceptance-criteria-validation.json",
        "target/local-model-quality-gate-smoke/evaluation-suite.json",
        "target/local-model-quality-gate-smoke/evaluation-suite-validation.json",
        "target/local-model-quality-gate-smoke/dry-run.json",
        "target/local-model-quality-gate-smoke/dry-run-validation.json",
        "target/local-model-quality-gate-smoke/quality-gate-run.json",
        "target/local-model-quality-gate-smoke/quality-gate-run-report.json",
        "target/local-model-quality-gate-smoke/quality-gate-run-validation.json",
        "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
        "target/local-model-quality-gate-smoke/operator-gate/gate-run.json",
        "target/local-model-quality-gate-smoke/operator-gate/gate-run-report.json",
        "target/local-model-quality-gate-smoke/operator-gate/gate-run-validation.json",
        "target/local-model-quality-gate-smoke/operator-gate/gate-run-output-validation.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.schema.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.gbnf",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.gbnf",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.schema.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.gbnf",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json",
        "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.gbnf",
        "target/local-model-quality-gate-smoke/quality-gate-status.json",
        "target/local-model-quality-gate-smoke/require-ready-status.json",
        "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
        "target/local-model-quality-gate-smoke/require-ready.stderr",
        "target/local-model-quality-gate-smoke/contract/local-model-response.schema.json",
        "target/local-model-quality-gate-smoke/contract/local-model-response.gbnf",
        "target/local-model-quality-gate-smoke/contract/local-model-context-compiler-response.schema.json",
        "target/local-model-quality-gate-smoke/contract/local-model-context-compiler-response.gbnf",
        "target/local-model-quality-gate-smoke/gate-artifacts/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
        "target/local-model-quality-gate-smoke/gate-artifacts/qwen-qwen2-5-0-5b-instruct/validation-report.json",
        "target/local-model-quality-gate-smoke/gate-artifacts/qwen-qwen3-0-6b/benchmark-report.json",
        "target/local-model-quality-gate-smoke/gate-artifacts/qwen-qwen3-0-6b/validation-report.json",
        "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
    ];
    let release_artifacts = vec![
        "target/release-preflight/proof-obligations.json",
        "target/release-preflight/proof-obligations-validation.json",
        "target/release-preflight/release-assets.json",
        "target/release-preflight/release-assets-validation.json",
        "target/release-preflight/release-upload-report.json",
        "target/release-preflight/release-upload-report-validation.json",
        "target/release-preflight/release-upload-failure-report.json",
        "target/release-preflight/release-upload-failure-report-validation.json",
        "target/release-preflight/release-view-failure-report.json",
        "target/release-preflight/release-view-failure-report-validation.json",
        "target/release-preflight/release-upload-test-report.json",
        "target/release-preflight/release-upload-test-report-validation.json",
    ];
    serde_json::json!({
        "format": "continuitydb.ci_artifact_inventory",
        "format_version": 1,
        "generated_by": "scripts/ci_artifact_inventory.sh",
        "valid": true,
        "artifact_roots": {
            "alpha_workflow": {"path": "target/alpha-workflow", "file_count": 27},
            "local_model_quality_gate_smoke": {
                "path": "target/local-model-quality-gate-smoke",
                "file_count": 79,
                "prompt_count": 9
            },
            "release_preflight": {"path": "target/release-preflight", "file_count": 4}
        },
        "verified_evidence": {
            "alpha_workflow": {
                "proof_obligation_count": 8,
                "proof_obligations_valid": true,
                "context_collapse_drill_valid": true,
                "summary_only_checkout_query_retained": true,
                "checkout_query_summary_validation_valid": true,
                "checkout_query_result_envelope_retained": true,
                "checkout_query_result_validation_valid": true,
                "cells_only_checkout_query_retained": true,
                "checkout_query_cells_result_validation_valid": true,
                "context_packets_only_checkout_query_retained": true,
                "checkout_query_context_packets_validation_valid": true,
                "workload_bundle_validation_valid": true,
                "workload_bundle_present": true,
                "replay_passed": true,
                "inspect_kernel_satisfies_required_profile": true,
                "inspect_kernel_persistent_index_checkpoint_trusted": true
            },
            "local_model_quality_gate_smoke": {
                "quality_gate_plan_validation_valid": true,
                "candidate_registry_validation_valid": true,
                "quality_gate_candidate_count": 2,
                "required_acceptance_criteria_count": 7,
                "acceptance_criteria_validation_valid": true,
                "evaluation_suite_validation_valid": true,
                "dry_run_validation_valid": true,
                "quality_gate_run_output_validation_valid": true,
                "complete_acceptance_coverage_candidate_count": 2,
                "prompt_count": 9,
                "dry_run_gate_ready": true,
                "quality_gate_status_validation_valid": true,
                "missing_artifact_status_ready": false,
                "missing_artifact_status_count": 2,
                "readiness_blocker_count": 2,
                "run_quality_gate_candidate_action_count": 2,
                "require_ready_status_retained": true,
                "require_ready_status_validation_valid": true,
                "operator_gate_run_report_retained": true,
                "operator_gate_run_output_validation_valid": true,
                "operator_gate_candidate_artifact_count": 2,
                "operator_gate_candidate_benchmark_validation_count": 2,
                "operator_gate_candidate_bundle_validation_count": 2,
                "operator_gate_candidate_validation_count": 2,
                "operator_gate_candidate_contract_artifact_count": 8,
                "operator_gate_validation_valid": true
            },
            "release_preflight": {
                "proof_obligation_count": 8,
                "proof_obligations_valid": true,
                "release_assets_valid": true,
                "release_asset_count": 12,
                "release_crate_archive_count": 10,
                "release_preflight_evidence_count": 2,
                "release_upload_preflight_valid": true,
                "release_upload_success_report_retained": true,
                "release_upload_success_report_validation_valid": true,
                "release_upload_failure_report_retained": true,
                "release_upload_failure_report_validation_valid": true,
                "release_view_failure_report_retained": true,
                "release_view_failure_report_validation_valid": true,
                "release_view_failure_diagnostic_checked": true,
                "release_upload_failure_diagnostic_checked": true,
                "release_upload_asset_integrity_checked": true,
                "release_upload_duplicate_manifest_checked": true,
                "release_upload_test_report_validation_valid": true
            }
        },
        "required_artifacts": {
            "alpha_workflow": alpha_artifacts,
            "local_model_quality_gate_smoke": smoke_artifacts,
            "release_preflight": release_artifacts
        },
        "required_checks": {
            "alpha_workflow": [
                {
                    "path": "target/alpha-workflow/proof-obligations-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.thesis_proof_obligations_validation\""
                },
                {
                    "path": "target/alpha-workflow/proof-obligations-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/context-collapse-drill.json",
                    "required_pattern": "\"format\": \"continuitydb.context_collapse_drill\""
                },
                {
                    "path": "target/alpha-workflow/context-collapse-drill.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/workload/workload-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.workload.bundle_validation\""
                },
                {
                    "path": "target/alpha-workflow/workload/workload-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/workload/workload-validation.json",
                    "required_pattern": "\"cells_parseable\": true"
                },
                {
                    "path": "target/alpha-workflow/workload/workload-validation.json",
                    "required_pattern": "\"checkout_request_parseable\": true"
                },
                {
                    "path": "target/alpha-workflow/workload/workload-report.json",
                    "required_pattern": "\"revision_link_count\": 14"
                },
                {
                    "path": "target/alpha-workflow/workload/continuitydb-workload.manifest.json",
                    "required_pattern": "\"revision_link_count\": 14"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.query",
                    "required_pattern": "RETURN summary_only"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.summary\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.json",
                    "required_pattern": "\"selected_cell_count\": 1"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.json",
                    "required_pattern": "\"selected_cell_count\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.json",
                    "required_pattern": "\"selection_reason_counts\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.json",
                    "required_pattern": "\"invalidation_condition_count\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary.json",
                    "required_pattern": "\"bounded_by_token_budget\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.summary_validation\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary-validation.json",
                    "required_pattern": "\"selected_cell_count\": 1"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary-validation.json",
                    "required_pattern": "\"invalidation_condition_count\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-summary-validation.json",
                    "required_pattern": "\"bounded_by_token_budget\": true"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.result\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result.json",
                    "required_pattern": "\"return_shape\": \"summary_only\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result.json",
                    "required_pattern": "\"type\": \"summary\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result.json",
                    "required_pattern": "\"selected_cell_count\": 1"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.result_validation\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result-validation.json",
                    "required_pattern": "\"return_shape\": \"summary_only\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-result-validation.json",
                    "required_pattern": "\"selected_cell_count\": 1"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells.query",
                    "required_pattern": "RETURN cells_only"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.cells\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells.json",
                    "required_pattern": "\"cells\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells.json",
                    "required_pattern": "\"payload\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.result\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result.json",
                    "required_pattern": "\"return_shape\": \"cells_only\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result.json",
                    "required_pattern": "\"type\": \"cells\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.result_validation\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result-validation.json",
                    "required_pattern": "\"return_shape\": \"cells_only\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result-validation.json",
                    "required_pattern": "\"result_type\": \"cells\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-cells-result-validation.json",
                    "required_pattern": "\"cell_count\": 1"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.query",
                    "required_pattern": "RETURN context_packets_only"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.context_packets\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"context_packets\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"strategy\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"compiler_policy\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"abstraction_level\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"compiler_reason_tags\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"compiler_evidence_locators\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"origin\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"cell_id\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"valid_time\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"commit_id\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"entries\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"source\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"text\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"confidence\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"token_count\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"citations\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"dependency_context\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"revision_context\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"target\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"kind\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"rationale\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"related_cell_id\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"relation\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"max_confidence\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"epistemic_action_reasons\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"answerability_questions\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"context_gaps\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"invalidation_conditions\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"trajectory_memory\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"epistemic_pressure\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"expectation\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"attention\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"salience_score\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"context_affordance\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"context_affordance_score\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets.json",
                    "required_pattern": "\"lifecycle_policy\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.checkout_query.context_packets_validation\""
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/alpha-workflow/query/checkout-context-packets-validation.json",
                    "required_pattern": "\"context_packet_count\": 1"
                },
                {
                    "path": "target/alpha-workflow/replay/replay-report.json",
                    "required_pattern": "\"passed\": true"
                },
                {
                    "path": "target/alpha-workflow/replay/replay-report.json",
                    "required_pattern": "\"revision_link_count\": 14"
                },
                {
                    "path": "target/alpha-workflow/replay/continuitydb-workload-replay.manifest.json",
                    "required_pattern": "\"revision_link_count\": 14"
                },
                {
                    "path": "target/alpha-workflow/inspect/inspect-kernel-report.json",
                    "required_pattern": "\"satisfies\": true"
                },
                {
                    "path": "target/alpha-workflow/inspect/inspect-kernel-report.json",
                    "required_pattern": "\"persistent_index_checkpoint_present_on_open\": true"
                },
                {
                    "path": "target/alpha-workflow/inspect/inspect-kernel-report.json",
                    "required_pattern": "\"persistent_index_checkpoint_trusted_on_open\": true"
                },
                {
                    "path": "target/alpha-workflow/inspect/inspect-kernel-report.json",
                    "required_pattern": "\"persistent_index_checkpoint_rebuilt_on_open\": false"
                }
            ],
            "local_model_quality_gate_smoke": [
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-plan.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_plan\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-plan-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_plan_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-plan-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-plan-validation.json",
                    "required_pattern": "\"candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-plan-validation.json",
                    "required_pattern": "\"total_steps\": 4"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-plan-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/quality-gate-plan.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/candidates.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_candidates\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/candidates-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_candidates_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/candidates-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/candidates-validation.json",
                    "required_pattern": "\"total_candidates\": 4"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/candidates-validation.json",
                    "required_pattern": "\"quality_gate_candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/candidates-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/candidates.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_acceptance_criteria\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"required_criteria_count\": 7"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"valid_json_schema_conformance\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"conflict_versus_supersession_classification\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"evidence_citation_preservation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"unsupported_claim_avoidance\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"stable_low_temperature_output\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"explicit_uncertainty_for_insufficient_evidence\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria.json",
                    "required_pattern": "\"deterministic_policy_rejection_avoidance\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_acceptance_criteria_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria-validation.json",
                    "required_pattern": "\"required_criteria_count\": 7"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/acceptance-criteria-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/acceptance-criteria.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/evaluation-suite.json",
                    "required_pattern": "\"complete\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/evaluation-suite-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_evaluation_suite_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/evaluation-suite-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/evaluation-suite-validation.json",
                    "required_pattern": "\"total_cases\": 9"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/evaluation-suite-validation.json",
                    "required_pattern": "\"complete\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/evaluation-suite-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/evaluation-suite.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/dry-run.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/dry-run-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_benchmark_report_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/dry-run-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/dry-run-validation.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/dry-run-validation.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/dry-run-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/dry-run.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_run_report_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-validation.json",
                    "required_pattern": "\"acceptance_coverage\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-validation.json",
                    "required_pattern": "\"candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-validation.json",
                    "required_pattern": "\"complete_candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-report.json",
                    "required_pattern": "\"ready\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_run_output_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
                    "required_pattern": "\"matches_report_file\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
                    "required_pattern": "\"candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
                    "required_pattern": "\"complete_candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-run-output-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/quality-gate-run.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status.json",
                    "required_pattern": "\"ready\": false"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status.json",
                    "required_pattern": "\"missing_artifacts\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status.json",
                    "required_pattern": "\"readiness_blockers\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status.json",
                    "required_pattern": "\"recommended_action\": \"run_quality_gate_candidate\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status.json",
                    "required_pattern": "\"run_quality_gate_candidate\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_status_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
                    "required_pattern": "\"candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
                    "required_pattern": "\"readiness_blocker_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
                    "required_pattern": "\"run_quality_gate_candidate_actions\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/quality-gate-status-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/quality-gate-status.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_status\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status.json",
                    "required_pattern": "\"ready\": false"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status.json",
                    "required_pattern": "\"readiness_blockers\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status.json",
                    "required_pattern": "\"run_quality_gate_candidate\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready.stderr",
                    "required_pattern": "local model quality gate is not ready"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_require_ready_status_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
                    "required_pattern": "\"ready\": false"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
                    "required_pattern": "\"readiness_blocker_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
                    "required_pattern": "\"contains_readiness_error\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/require-ready-status-validation.json",
                    "required_pattern": "\"stderr_path\": \"target/local-model-quality-gate-smoke/require-ready.stderr\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-report.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_run\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-report.json",
                    "required_pattern": "\"generated_by_command\": \"run-local-model-quality-gate\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-report.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-report.json",
                    "required_pattern": "\"candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-output-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_quality_gate_run_output_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-output-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-output-validation.json",
                    "required_pattern": "\"matches_report_file\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-output-validation.json",
                    "required_pattern": "\"candidate_count\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/gate-run-output-validation.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/operator-gate/gate-run.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
                    "required_pattern": "\"complete\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json",
                    "required_pattern": "\"context_compiler_schema_version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_benchmark_report_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_bundle_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"artifact_dir\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json",
                    "required_pattern": "\"context_compiler_schema_version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model.benchmark_bundle\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json",
                    "required_pattern": "\"context_compiler_schema_version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.schema.json",
                    "required_pattern": "\"title\": \"ContinuityDB Local Model Steward Response\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.gbnf",
                    "required_pattern": "root ::= response"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json",
                    "required_pattern": "\"title\": \"ContinuityDB Local Model Context Compiler Response\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json",
                    "required_pattern": "\"x-continuitydb-schema-version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.gbnf",
                    "required_pattern": "root ::= context-compiler-response"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen3-0.6B\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json",
                    "required_pattern": "\"complete\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json",
                    "required_pattern": "\"context_compiler_schema_version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_benchmark_report_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen3-0.6B\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model_bundle_validation\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"candidate_model_id\": \"Qwen/Qwen3-0.6B\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"dry_run\": true"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"artifact_dir\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"report_path\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json",
                    "required_pattern": "\"context_compiler_schema_version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json",
                    "required_pattern": "\"format\": \"continuitydb.local_model.benchmark_bundle\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json",
                    "required_pattern": "\"context_compiler_schema_version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.schema.json",
                    "required_pattern": "\"title\": \"ContinuityDB Local Model Steward Response\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.gbnf",
                    "required_pattern": "root ::= response"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json",
                    "required_pattern": "\"title\": \"ContinuityDB Local Model Context Compiler Response\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json",
                    "required_pattern": "\"x-continuitydb-schema-version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.gbnf",
                    "required_pattern": "root ::= context-compiler-response"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/contract/local-model-context-compiler-response.schema.json",
                    "required_pattern": "\"title\": \"ContinuityDB Local Model Context Compiler Response\""
                },
                {
                    "path": "target/local-model-quality-gate-smoke/contract/local-model-context-compiler-response.schema.json",
                    "required_pattern": "\"x-continuitydb-schema-version\": 2"
                },
                {
                    "path": "target/local-model-quality-gate-smoke/contract/local-model-context-compiler-response.gbnf",
                    "required_pattern": "root ::= context-compiler-response"
                }
            ],
            "release_preflight": [
                {
                    "path": "target/release-preflight/proof-obligations-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.thesis_proof_obligations_validation\""
                },
                {
                    "path": "target/release-preflight/proof-obligations-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-assets.json",
                    "required_pattern": "\"format\": \"continuitydb.release_assets\""
                },
                {
                    "path": "target/release-preflight/release-assets.json",
                    "required_pattern": "\"asset_count\": 12"
                },
                {
                    "path": "target/release-preflight/release-assets-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.release_assets_validation\""
                },
                {
                    "path": "target/release-preflight/release-assets-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-assets-validation.json",
                    "required_pattern": "\"asset_count\": 12"
                },
                {
                    "path": "target/release-preflight/release-assets-validation.json",
                    "required_pattern": "\"crate_archive_count\": 10"
                },
                {
                    "path": "target/release-preflight/release-assets-validation.json",
                    "required_pattern": "\"release_preflight_evidence_count\": 2"
                },
                {
                    "path": "target/release-preflight/release-upload-report.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload\""
                },
                {
                    "path": "target/release-preflight/release-upload-report.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-report.json",
                    "required_pattern": "\"release_tag\": \"v0.1.0\""
                },
                {
                    "path": "target/release-preflight/release-upload-report.json",
                    "required_pattern": "\"release_repository\": \"syndicat/continuitydb\""
                },
                {
                    "path": "target/release-preflight/release-upload-report.json",
                    "required_pattern": "\"asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-upload-report.json",
                    "required_pattern": "\"manifest_format\": \"continuitydb.release_assets\""
                },
                {
                    "path": "target/release-preflight/release-upload-report-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload_validation\""
                },
                {
                    "path": "target/release-preflight/release-upload-report-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-report-validation.json",
                    "required_pattern": "\"asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-upload-report-validation.json",
                    "required_pattern": "\"manifest_asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report.json",
                    "required_pattern": "\"valid\": false"
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report.json",
                    "required_pattern": "\"failure_stage\": \"upload\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report.json",
                    "required_pattern": "\"release_tag\": \"v0.1.0\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report.json",
                    "required_pattern": "\"release_repository\": \"syndicat/continuitydb\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report.json",
                    "required_pattern": "\"manifest_format\": \"continuitydb.release_assets\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload_validation\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report-validation.json",
                    "required_pattern": "\"release_upload_succeeded\": false"
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report-validation.json",
                    "required_pattern": "\"failure_stage\": \"upload\""
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report-validation.json",
                    "required_pattern": "\"asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-upload-failure-report-validation.json",
                    "required_pattern": "\"manifest_asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-view-failure-report.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report.json",
                    "required_pattern": "\"valid\": false"
                },
                {
                    "path": "target/release-preflight/release-view-failure-report.json",
                    "required_pattern": "\"failure_stage\": \"release_view\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report.json",
                    "required_pattern": "\"release_tag\": \"v0.1.0\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report.json",
                    "required_pattern": "\"release_repository\": \"syndicat/continuitydb\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report.json",
                    "required_pattern": "\"manifest_format\": \"continuitydb.release_assets\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload_validation\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-view-failure-report-validation.json",
                    "required_pattern": "\"release_upload_succeeded\": false"
                },
                {
                    "path": "target/release-preflight/release-view-failure-report-validation.json",
                    "required_pattern": "\"failure_stage\": \"release_view\""
                },
                {
                    "path": "target/release-preflight/release-view-failure-report-validation.json",
                    "required_pattern": "\"asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-view-failure-report-validation.json",
                    "required_pattern": "\"manifest_asset_count\": 1"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload_test\""
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"release_view_preflight_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"upload_failure_diagnostic_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"release_view_failure_diagnostic_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"asset_integrity_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"duplicate_manifest_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"success_report_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"success_report_validation_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"failure_report_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"failure_report_validation_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"release_view_failure_report_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report.json",
                    "required_pattern": "\"release_view_failure_report_validation_checked\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report-validation.json",
                    "required_pattern": "\"format\": \"continuitydb.release_upload_test_validation\""
                },
                {
                    "path": "target/release-preflight/release-upload-test-report-validation.json",
                    "required_pattern": "\"valid\": true"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report-validation.json",
                    "required_pattern": "\"checked_condition_count\": 12"
                },
                {
                    "path": "target/release-preflight/release-upload-test-report-validation.json",
                    "required_pattern": "\"mocked_gh_invocation_count\""
                }
            ]
        },
        "contract_summary": {
            "alpha_workflow": {
                "required_artifact_count": 28,
                "required_check_count": 90
            },
            "local_model_quality_gate_smoke": {
                "required_artifact_count": 47,
                "required_check_count": 126
            },
            "release_preflight": {
                "required_artifact_count": 12,
                "required_check_count": 60
            },
            "total_required_artifact_count": 87,
            "total_required_check_count": 276
        }
    })
}

#[cfg(all(feature = "local-model", unix))]
fn passing_local_model_runner_script() -> &'static str {
    r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000004","kind":"supersedes","target":"00000000-0000-0000-0000-000000000005"},"rationale":"The newer evidence supersedes the older status without contradicting it.","citations":["continuitydb://evaluation/supersession-evidence"]},{"action":{"type":"request_verification","cell_id":null,"request":"Verify deployment status before treating the release as shipped."},"rationale":"The evidence does not support deployment, so the shipped claim remains unsupported.","citations":["continuitydb://evaluation/unsupported-release-claim"]},{"action":{"type":"adjust_confidence","cell_id":"00000000-0000-0000-0000-000000000006","proposed_confidence":0.42},"rationale":"The cited evidence lowers confidence in the stale deployment status.","citations":["continuitydb://evaluation/confidence-evidence"]},{"action":{"type":"request_verification","cell_id":"00000000-0000-0000-0000-000000000007","request":"Refresh the stale high-impact frontier signal."},"rationale":"The stale high-impact frontier signal needs a refresh from current evidence.","citations":["continuitydb://evaluation/targeted-verification-evidence"]},{"action":{"type":"create_cell_draft","anchors":["project:continuitydb:benchmark-result"],"payload_text":"ContinuityDB local Steward benchmark produced a new result requiring review."},"rationale":"The new benchmark evidence supports drafting a StateCell for review.","citations":["continuitydb://evaluation/new-benchmark-evidence"]},{"action":{"type":"mark_frontier","cell_id":"00000000-0000-0000-0000-000000000003"},"rationale":"The release status changed between the build and incident sources, so this state should stay on the frontier.","citations":["continuitydb://evaluation/release-build-source","continuitydb://evaluation/release-incident-source"]},{"action":{"type":"request_verification","cell_id":null,"request":"Ask for a concrete answerability question before labeling the cell."},"rationale":"The answerability label input is invalid because it has no concrete question.","citations":["continuitydb://evaluation/invalid-answerability-label"]}]}'
"#
}

fn write_committed_store(path: &PathBuf, anchor: &str) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(vec![test_cell(anchor)?], committed_at, commit_id)?;
    Ok(())
}

fn write_context_packet_store(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut cell = test_cell("project:continuitydb:cli-query-text-context-packets")?;
    cell.add_projection(MemoryProjection::new(
        MemoryProjectionKind::Semantic,
        "Current belief: context packets are the agent-facing projection.",
        Confidence::new(0.88)?,
        CellCost::new(9, 0)?,
    )?);
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(vec![cell], committed_at, CommitId::new())?;
    Ok(())
}

fn write_trajectory_context_packet_store(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut cell = test_cell("project:continuitydb:cli-query-trajectory-context-packets")?;
    cell.set_trajectory_memory(TrajectoryMemory::new(
        "retry context packet validation without trace citation",
        "retained context packet artifact was generated",
        "validator accepted trajectory memory without trace citation",
        "artifact://rollout/context-packet-trajectory",
        0.87,
        "retain trajectory trace citations with reusable experience packets",
        vec!["validating retained context packet artifacts".to_string()],
        vec!["trace locator is missing from packet citations".to_string()],
        ContextPacketStrategy::FalsificationBrief,
    )?);
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(vec![cell], committed_at, CommitId::new())?;
    Ok(())
}

fn write_dependency_context_store(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let support = test_cell("project:continuitydb:cli-dependency-support")?;
    let support_id = support.id;
    let mut dependent = test_cell("project:continuitydb:cli-dependency-dependent")?;
    dependent.dependencies.push(CellDependency::new(
        support_id,
        CellDependencyKind::DependsOn,
        "dependent context needs the support cell",
    ));
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(vec![support, dependent], committed_at, CommitId::new())?;
    Ok(())
}

fn write_revision_link_store(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let source = test_cell("project:continuitydb:cli-status-source")?;
    let target = test_cell("project:continuitydb:cli-status-target")?;
    let source_id = source.id;
    let target_id = target.id;
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(vec![source, target], committed_at, CommitId::new())?;
    db.record_revision_link_at(
        source_id,
        RevisionLinkKind::Supersedes,
        target_id,
        committed_at,
    )?;
    Ok(())
}

fn write_revision_filter_store(
    path: &PathBuf,
) -> Result<(StateCellId, StateCellId), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let selected = test_cell("project:continuitydb:cli-revision-selected")?;
    let related = test_cell("project:continuitydb:cli-revision-related")?;
    let unrelated = test_cell("project:continuitydb:cli-revision-unrelated")?;
    let selected_id = selected.id;
    let related_id = related.id;
    let unrelated_id = unrelated.id;
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(
        vec![selected, related, unrelated],
        committed_at,
        CommitId::new(),
    )?;
    db.record_revision_link_at(
        selected_id,
        RevisionLinkKind::ConflictsWith,
        related_id,
        committed_at,
    )?;
    db.record_revision_link_at(
        unrelated_id,
        RevisionLinkKind::Supersedes,
        related_id,
        committed_at,
    )?;
    Ok((selected_id, related_id))
}

fn write_two_commit_store(path: &PathBuf) -> Result<Vec<CommitId>, Box<dyn std::error::Error>> {
    let first_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-export-page-first")?],
        first_time,
        first_commit,
    )?;
    db.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-export-page-second")?],
        second_time,
        second_commit,
    )?;
    Ok(vec![first_commit, second_commit])
}

fn write_legacy_store(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut cell = test_cell("project:continuitydb:cli-compact")?;
    cell.system_time = SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    fs::write(path, format!("{}\n", serde_json::to_string(&cell)?))?;
    Ok(())
}

fn test_cell(anchor: &str) -> Result<StateCell, Box<dyn std::error::Error>> {
    let valid_from = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    StateCell::new(
        StateCellId::new(),
        vec![SemanticAnchor::new(anchor)],
        ValidTimeRange::new(valid_from, None)?,
        Scope::Project("continuitydb".to_string()),
        Answerability::new(vec!["what is stored?".to_string()])?,
        vec![Evidence {
            source: SourceId::new("test"),
            citation: Citation {
                locator: "test://cli".to_string(),
            },
            confidence: Confidence::new(0.91)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text(anchor.to_string()),
        CellCost::new(12, 0)?,
    )
    .map_err(Into::into)
}
