//! CLI smoke tests.

use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use continuitydb_api::ContinuityDb;
use continuitydb_core::{
    Answerability, CellCost, CellPayload, Citation, CommitId, Confidence, Evidence,
    RevisionLinkKind, Scope, SemanticAnchor, SourceId, StateCell, StateCellId, SystemTimeRange,
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
use std::{fs, path::PathBuf};

#[cfg(all(feature = "local-model", unix))]
use std::os::unix::fs::PermissionsExt;

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
    assert_eq!(json["checkout"]["matched_count"].as_u64(), Some(8));
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
    assert!(json["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
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
    assert!(json["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
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
    assert_eq!(
        bundle_manifest["format"].as_str(),
        Some("continuitydb.local_model.benchmark_bundle")
    );
    assert_eq!(
        bundle_manifest["benchmark_report_path"].as_str(),
        Some(report_path.display().to_string().as_str())
    );
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
    assert!(
        bundle_manifest["response_artifact_manifest"]["manifest_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("responses/local-model-responses.manifest.json"))
    );
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

    assert_eq!(json["runtime"]["arguments"][0].as_str(), Some("--model"));
    assert_eq!(
        json["runtime"]["arguments"][1].as_str(),
        Some("/models/qwen.gguf")
    );
    assert_eq!(json["runtime"]["arguments"][2].as_str(), Some("--ctx-size"));
    assert_eq!(json["runtime"]["arguments"][3].as_str(), Some("4096"));
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

    assert!(schema_path.exists());
    assert!(grammar_path.exists());
    assert_eq!(
        json["contract_artifacts"]["schema_path"].as_str(),
        Some(schema_path.display().to_string().as_str())
    );
    assert_eq!(
        json["contract_artifacts"]["grammar_path"].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert!(json["contract_artifacts"]["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["contract_artifacts"]["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
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
        json["candidates"][0]["recommended_runtime"].as_str(),
        Some("llama.cpp")
    );
    assert_eq!(
        json["candidates"][0]["artifact_format"].as_str(),
        Some("GGUF")
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
        json["candidates"][0]["recommended_runner_arguments"][5].as_str(),
        Some("0")
    );
    assert!(json["candidates"].as_array().is_some_and(|candidates| {
        candidates.iter().any(|candidate| {
            candidate["model_id"].as_str() == Some("HuggingFaceTB/SmolLM2-360M-Instruct")
                && candidate["role"].as_str() == Some("ultra-small-experimental")
        })
    }));
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
    assert!(grammar.contains("root ::= response"));
    assert!(grammar.contains("request-verification-action"));

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

    fs::remove_file(store_path)?;
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
fn cli_checkout_query_rejects_unsupported_query_semantics() -> Result<(), Box<dyn std::error::Error>>
{
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
        .stderr(contains("unsupported return shape"));

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
        Some(false)
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
fn cli_inspect_kernel_rejects_indexed_embedded_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-indexed");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("indexed-embedded")
        .assert()
        .failure()
        .stderr(contains("storage kernel requirements are not met"));

    fs::remove_file(path)?;
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
