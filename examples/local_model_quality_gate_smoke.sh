#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKDIR="${1:-$(mktemp -d "${TMPDIR:-/tmp}/continuitydb-local-model-gate.XXXXXX")}"

mkdir -p "$WORKDIR"

PLAN="$WORKDIR/quality-gate-plan.json"
PLAN_VALIDATION="$WORKDIR/quality-gate-plan-validation.json"
CANDIDATES="$WORKDIR/candidates.json"
CANDIDATES_VALIDATION="$WORKDIR/candidates-validation.json"
ACCEPTANCE_CRITERIA="$WORKDIR/acceptance-criteria.json"
ACCEPTANCE_CRITERIA_VALIDATION="$WORKDIR/acceptance-criteria-validation.json"
SUITE="$WORKDIR/evaluation-suite.json"
SUITE_VALIDATION="$WORKDIR/evaluation-suite-validation.json"
STATUS="$WORKDIR/quality-gate-status.json"
STATUS_VALIDATION="$WORKDIR/quality-gate-status-validation.json"
READY_STATUS="$WORKDIR/require-ready-status.json"
READY_STATUS_VALIDATION="$WORKDIR/require-ready-status-validation.json"
READY_STDERR="$WORKDIR/require-ready.stderr"
DRY_RUN="$WORKDIR/dry-run.json"
DRY_RUN_VALIDATION="$WORKDIR/dry-run-validation.json"
GATE_RUN="$WORKDIR/quality-gate-run.json"
GATE_RUN_REPORT="$WORKDIR/quality-gate-run-report.json"
GATE_RUN_VALIDATION="$WORKDIR/quality-gate-run-validation.json"
GATE_RUN_OUTPUT_VALIDATION="$WORKDIR/quality-gate-run-output-validation.json"
OPERATOR_GATE_ROOT="$WORKDIR/operator-gate"
BASELINE="$WORKDIR/baselines.jsonl"
GATE_BASELINE="$WORKDIR/gate-baselines.jsonl"
CONTRACT_DIR="$WORKDIR/contract"
PROMPT_DIR="$WORKDIR/prompts"
ARTIFACT_ROOT="$WORKDIR/artifacts"
GATE_ARTIFACT_ROOT="$WORKDIR/gate-artifacts"
MODEL_PATH="$WORKDIR/model.gguf"
QWEN25_PATH="$WORKDIR/qwen2.5.gguf"
QWEN3_PATH="$WORKDIR/qwen3.gguf"

rm -f "$PLAN" "$PLAN_VALIDATION" "$CANDIDATES" "$CANDIDATES_VALIDATION" "$ACCEPTANCE_CRITERIA" "$ACCEPTANCE_CRITERIA_VALIDATION" "$SUITE" "$SUITE_VALIDATION" "$STATUS" "$STATUS_VALIDATION" "$READY_STATUS" "$READY_STATUS_VALIDATION" "$READY_STDERR" "$DRY_RUN" "$DRY_RUN_VALIDATION" "$GATE_RUN" "$GATE_RUN_REPORT" "$GATE_RUN_VALIDATION" "$GATE_RUN_OUTPUT_VALIDATION" "$BASELINE" "$GATE_BASELINE" "$MODEL_PATH" "$QWEN25_PATH" "$QWEN3_PATH"
rm -rf "$CONTRACT_DIR" "$PROMPT_DIR" "$ARTIFACT_ROOT" "$GATE_ARTIFACT_ROOT" "$OPERATOR_GATE_ROOT"

cd "$ROOT"

cargo run -q -p continuitydb-cli --features local-model -- local-model-quality-gate-plan >"$PLAN"
grep -q '"format": "continuitydb.local_model_quality_gate_plan"' "$PLAN"
grep -q '"candidate_set": "quality_gate"' "$PLAN"
grep -q '"job_id": "local-model-qwen-qwen2-5-0-5b-instruct"' "$PLAN"
grep -q '"substitution_contract"' "$PLAN"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-quality-gate-plan \
  --report-path "$PLAN" \
  --validation-report-path "$PLAN_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.local_model_quality_gate_plan_validation"' "$PLAN_VALIDATION"
grep -q '"valid": true' "$PLAN_VALIDATION"
grep -q '"candidate_count": 2' "$PLAN_VALIDATION"
grep -q '"total_steps": 4' "$PLAN_VALIDATION"

cargo run -q -p continuitydb-cli --features local-model -- local-model-candidates >"$CANDIDATES"
grep -q '"format": "continuitydb.local_model_candidates"' "$CANDIDATES"
grep -q '"default_quality_gate_candidate": true' "$CANDIDATES"
grep -q '"quality_gate_eligible": true' "$CANDIDATES"
grep -q '"ci_suitable": true' "$CANDIDATES"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-candidates \
  --report-path "$CANDIDATES" \
  --validation-report-path "$CANDIDATES_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.local_model_candidates_validation"' "$CANDIDATES_VALIDATION"
grep -q '"valid": true' "$CANDIDATES_VALIDATION"
grep -q '"total_candidates": 4' "$CANDIDATES_VALIDATION"
grep -q '"quality_gate_candidate_count": 2' "$CANDIDATES_VALIDATION"

cargo run -q -p continuitydb-cli --features local-model -- local-model-acceptance-criteria >"$ACCEPTANCE_CRITERIA"
grep -q '"format": "continuitydb.local_model_acceptance_criteria"' "$ACCEPTANCE_CRITERIA"
grep -q '"required_criteria_count": 7' "$ACCEPTANCE_CRITERIA"
grep -q '"valid_json_schema_conformance"' "$ACCEPTANCE_CRITERIA"
grep -q '"conflict_versus_supersession_classification"' "$ACCEPTANCE_CRITERIA"
grep -q '"evidence_citation_preservation"' "$ACCEPTANCE_CRITERIA"
grep -q '"unsupported_claim_avoidance"' "$ACCEPTANCE_CRITERIA"
grep -q '"stable_low_temperature_output"' "$ACCEPTANCE_CRITERIA"
grep -q '"explicit_uncertainty_for_insufficient_evidence"' "$ACCEPTANCE_CRITERIA"
grep -q '"deterministic_policy_rejection_avoidance"' "$ACCEPTANCE_CRITERIA"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-acceptance-criteria \
  --report-path "$ACCEPTANCE_CRITERIA" \
  --validation-report-path "$ACCEPTANCE_CRITERIA_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.local_model_acceptance_criteria_validation"' "$ACCEPTANCE_CRITERIA_VALIDATION"
grep -q '"valid": true' "$ACCEPTANCE_CRITERIA_VALIDATION"
grep -q '"required_criteria_count": 7' "$ACCEPTANCE_CRITERIA_VALIDATION"

cargo run -q -p continuitydb-cli --features local-model -- local-model-evaluation-suite >"$SUITE"
grep -q '"format": "continuitydb.local_model_evaluation_suite"' "$SUITE"
grep -q '"evaluation_suite_fingerprint"' "$SUITE"
grep -q '"total_cases": 9' "$SUITE"
grep -q '"complete": true' "$SUITE"
grep -q '"explicit_uncertainty_for_insufficient_evidence"' "$SUITE"
grep -q '"policy rejection avoidance"' "$SUITE"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-evaluation-suite \
  --report-path "$SUITE" \
  --validation-report-path "$SUITE_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.local_model_evaluation_suite_validation"' "$SUITE_VALIDATION"
grep -q '"valid": true' "$SUITE_VALIDATION"
grep -q '"total_cases": 9' "$SUITE_VALIDATION"
grep -q '"complete": true' "$SUITE_VALIDATION"

cargo run -q -p continuitydb-cli --features local-model -- benchmark-local-model \
  --dry-run \
  --candidate-defaults \
  --enforce-candidate-requirements \
  --executable /bin/echo \
  --model-path "$MODEL_PATH" \
  --baseline-path "$BASELINE" \
  --contract-dir "$CONTRACT_DIR" \
  --prompt-dir "$PROMPT_DIR" \
  --report-path "$DRY_RUN" >/dev/null

test -s "$DRY_RUN"
test -s "$CONTRACT_DIR/local-model-response.schema.json"
test -s "$CONTRACT_DIR/local-model-response.gbnf"
test -s "$CONTRACT_DIR/local-model-context-compiler-response.schema.json"
test -s "$CONTRACT_DIR/local-model-context-compiler-response.gbnf"
grep -q '"dry_run": true' "$DRY_RUN"
grep -q '"source": "default_ci_candidate"' "$DRY_RUN"
grep -q '"will_record_baseline": false' "$DRY_RUN"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-benchmark-report \
  --report-path "$DRY_RUN" \
  --validation-report-path "$DRY_RUN_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.local_model_benchmark_report_validation"' "$DRY_RUN_VALIDATION"
grep -q '"valid": true' "$DRY_RUN_VALIDATION"
grep -q '"dry_run": true' "$DRY_RUN_VALIDATION"
grep -q '"candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct"' "$DRY_RUN_VALIDATION"

prompt_count="$(find "$PROMPT_DIR" -type f -name '*.prompt.txt' | wc -l | tr -d ' ')"
test "$prompt_count" = "9"

cargo run -q -p continuitydb-cli --features local-model -- run-local-model-quality-gate \
  --dry-run \
  --executable /bin/echo \
  --model-path "Qwen/Qwen2.5-0.5B-Instruct=$QWEN25_PATH" \
  --model-path "Qwen/Qwen3-0.6B=$QWEN3_PATH" \
  --baseline-path "$GATE_BASELINE" \
  --artifact-root "$GATE_ARTIFACT_ROOT" \
  --report-path "$GATE_RUN_REPORT" \
  --fail-on-not-ready >"$GATE_RUN"
grep -q '"format": "continuitydb.local_model_quality_gate_run"' "$GATE_RUN"
grep -q '"generated_by_command": "run-local-model-quality-gate"' "$GATE_RUN"
grep -q '"candidate_count": 2' "$GATE_RUN"
grep -q '"ready": true' "$GATE_RUN"
test -s "$GATE_RUN_REPORT"
grep -q '"runtime_preflight"' "$GATE_RUN_REPORT"
grep -q '"acceptance_coverage"' "$GATE_ARTIFACT_ROOT/qwen-qwen2-5-0-5b-instruct/benchmark-report.json"
grep -q '"complete": true' "$GATE_ARTIFACT_ROOT/qwen-qwen2-5-0-5b-instruct/benchmark-report.json"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-quality-gate-run-report \
  --report-path "$GATE_RUN_REPORT" \
  --validation-report-path "$GATE_RUN_VALIDATION" >/dev/null
test -s "$GATE_RUN_VALIDATION"
grep -q '"format": "continuitydb.local_model_quality_gate_run_report_validation"' "$GATE_RUN_VALIDATION"
grep -q '"valid": true' "$GATE_RUN_VALIDATION"
grep -q '"acceptance_coverage"' "$GATE_RUN_VALIDATION"
grep -q '"candidate_count": 2' "$GATE_RUN_VALIDATION"
grep -q '"complete_candidate_count": 2' "$GATE_RUN_VALIDATION"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-quality-gate-run-output \
  --report-path "$GATE_RUN" \
  --validation-report-path "$GATE_RUN_OUTPUT_VALIDATION" >/dev/null
test -s "$GATE_RUN_OUTPUT_VALIDATION"
grep -q '"format": "continuitydb.local_model_quality_gate_run_output_validation"' "$GATE_RUN_OUTPUT_VALIDATION"
grep -q '"valid": true' "$GATE_RUN_OUTPUT_VALIDATION"
grep -q '"matches_report_file": true' "$GATE_RUN_OUTPUT_VALIDATION"
grep -q '"candidate_count": 2' "$GATE_RUN_OUTPUT_VALIDATION"
grep -q '"complete_candidate_count": 2' "$GATE_RUN_OUTPUT_VALIDATION"
test -s "$GATE_ARTIFACT_ROOT/qwen-qwen2-5-0-5b-instruct/benchmark-report.json"
test -s "$GATE_ARTIFACT_ROOT/qwen-qwen2-5-0-5b-instruct/validation-report.json"
test -s "$GATE_ARTIFACT_ROOT/qwen-qwen3-0-6b/benchmark-report.json"
test -s "$GATE_ARTIFACT_ROOT/qwen-qwen3-0-6b/validation-report.json"

CONTINUITYDB_LOCAL_MODEL_DRY_RUN=1 \
CONTINUITYDB_LOCAL_MODEL_ARTIFACT_ROOT="$OPERATOR_GATE_ROOT" \
CONTINUITYDB_LOCAL_MODEL_BASELINE_PATH="$WORKDIR/operator-gate-baselines.jsonl" \
  scripts/local_model_quality_gate.sh >/dev/null
test -s "$OPERATOR_GATE_ROOT/gate-run-report.json"
test -s "$OPERATOR_GATE_ROOT/gate-run-validation.json"
test -s "$OPERATOR_GATE_ROOT/gate-run-output-validation.json"
grep -q '"valid": true' "$OPERATOR_GATE_ROOT/gate-run-validation.json"
grep -q '"format": "continuitydb.local_model_quality_gate_run_output_validation"' "$OPERATOR_GATE_ROOT/gate-run-output-validation.json"
grep -q '"valid": true' "$OPERATOR_GATE_ROOT/gate-run-output-validation.json"
grep -q '"matches_report_file": true' "$OPERATOR_GATE_ROOT/gate-run-output-validation.json"
for candidate_slug in qwen-qwen2-5-0-5b-instruct qwen-qwen3-0-6b; do
  case "$candidate_slug" in
    qwen-qwen2-5-0-5b-instruct) candidate_model_id="Qwen/Qwen2.5-0.5B-Instruct" ;;
    qwen-qwen3-0-6b) candidate_model_id="Qwen/Qwen3-0.6B" ;;
    *) printf 'unexpected local-model candidate slug: %s\n' "$candidate_slug" >&2; exit 1 ;;
  esac
  cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-benchmark-report \
    --report-path "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/benchmark-report.json" \
    --validation-report-path "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/benchmark-report-validation.json" >/dev/null
  test -s "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/benchmark-report-validation.json"
  grep -q '"format": "continuitydb.local_model_benchmark_report_validation"' "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/benchmark-report-validation.json"
  grep -q '"valid": true' "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/benchmark-report-validation.json"
  grep -q '"dry_run": true' "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/benchmark-report-validation.json"
  grep -q '"format": "continuitydb.local_model_bundle_validation"' "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/validation-report.json"
  grep -q '"valid": true' "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/validation-report.json"
  grep -q "\"candidate_model_id\": \"$candidate_model_id\"" "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/validation-report.json"
  grep -q "\"artifact_dir\": \"$OPERATOR_GATE_ROOT/candidates/$candidate_slug\"" "$OPERATOR_GATE_ROOT/candidates/$candidate_slug/validation-report.json"
done

cargo run -q -p continuitydb-cli --features local-model -- local-model-quality-gate-status \
  --artifact-root "$ARTIFACT_ROOT" >"$STATUS"
grep -q '"format": "continuitydb.local_model_quality_gate_status"' "$STATUS"
grep -q '"ready": false' "$STATUS"
grep -q '"missing_artifacts": 2' "$STATUS"
grep -q '"readiness_blockers"' "$STATUS"
grep -q '"recommended_action": "run_quality_gate_candidate"' "$STATUS"
grep -q '"run_quality_gate_candidate": 2' "$STATUS"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-quality-gate-status \
  --report-path "$STATUS" \
  --validation-report-path "$STATUS_VALIDATION" >/dev/null
test -s "$STATUS_VALIDATION"
grep -q '"format": "continuitydb.local_model_quality_gate_status_validation"' "$STATUS_VALIDATION"
grep -q '"valid": true' "$STATUS_VALIDATION"
grep -q '"candidate_count": 2' "$STATUS_VALIDATION"
grep -q '"readiness_blocker_count": 2' "$STATUS_VALIDATION"
grep -q '"run_quality_gate_candidate_actions": 2' "$STATUS_VALIDATION"

if cargo run -q -p continuitydb-cli --features local-model -- local-model-quality-gate-status \
  --artifact-root "$ARTIFACT_ROOT" \
  --require-ready >"$READY_STATUS" 2>"$READY_STDERR"; then
  printf 'expected local-model-quality-gate-status --require-ready to fail for missing artifacts\n' >&2
  exit 1
fi
test -s "$READY_STATUS"
grep -q '"format": "continuitydb.local_model_quality_gate_status"' "$READY_STATUS"
grep -q '"ready": false' "$READY_STATUS"
grep -q '"readiness_blockers"' "$READY_STATUS"
grep -q '"run_quality_gate_candidate": 2' "$READY_STATUS"
grep -q 'local model quality gate is not ready' "$READY_STDERR"
cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-require-ready-status \
  --report-path "$READY_STATUS" \
  --stderr-path "$READY_STDERR" \
  --validation-report-path "$READY_STATUS_VALIDATION" >/dev/null
test -s "$READY_STATUS_VALIDATION"
grep -q '"format": "continuitydb.local_model_require_ready_status_validation"' "$READY_STATUS_VALIDATION"
grep -q '"valid": true' "$READY_STATUS_VALIDATION"
grep -q '"ready": false' "$READY_STATUS_VALIDATION"
grep -q '"readiness_blocker_count": 2' "$READY_STATUS_VALIDATION"
grep -q '"contains_readiness_error": true' "$READY_STATUS_VALIDATION"

printf 'ContinuityDB local model quality gate smoke artifacts: %s\n' "$WORKDIR"
