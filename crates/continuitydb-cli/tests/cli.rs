//! CLI smoke tests.

use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use continuitydb_api::ContinuityDb;
use continuitydb_core::{
    Answerability, CellCost, CellPayload, Citation, CommitId, Confidence, Evidence, Scope,
    SemanticAnchor, SourceId, StateCell, StateCellId, SystemTimeRange, TrustSignal, ValidTimeRange,
};
use continuitydb_kernel::{CommitManifestLookup, FileKernel};
use continuitydb_query::{
    encode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelope, QueryOptimization,
    QueryRequirements, QueryReturnShape, QueryTask, QUERY_ENVELOPE_FORMAT,
    QUERY_ENVELOPE_FORMAT_VERSION,
};
use predicates::str::contains;
use serde_json::Value;
use std::{fs, path::PathBuf};

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
    assert_eq!(json["status"]["cell_count"].as_u64(), Some(0));
    assert_eq!(json["status"]["commit_count"].as_u64(), Some(0));
    assert!(
        json["status"]["file_size_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert_eq!(json["health"]["has_header"].as_bool(), Some(true));
    assert_eq!(json["health"]["legacy_raw_cells"].as_u64(), Some(0));
    assert_eq!(json["health"]["checksum_free_records"].as_u64(), Some(0));
    assert_eq!(json["health"]["canonical_records"].as_u64(), Some(0));
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
    write_committed_store(&source_path, "project:continuitydb:cli-backup")?;

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
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(import_json["path"].as_str(), target_path.to_str());
    assert_eq!(import_json["input"].as_str(), backup_path.to_str());
    assert_eq!(import_json["imported_commits"].as_u64(), Some(1));
    assert_eq!(import_json["next_after"], export_json["next_after"]);
    assert_eq!(target_batch, source_batch);

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
