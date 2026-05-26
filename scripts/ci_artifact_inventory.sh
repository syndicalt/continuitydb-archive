#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

alpha_dir="${1:-target/alpha-workflow}"
smoke_dir="${2:-target/local-model-quality-gate-smoke}"
release_dir="${3:-target/release-preflight}"
inventory_path="${4:-target/ci-artifact-inventory.json}"
failure_report_path="${5:-target/ci-artifact-inventory-failure.json}"

json_string() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  printf '"%s"' "$value"
}

write_failure_report() {
  local message="$1"
  mkdir -p "$(dirname "$failure_report_path")"
  {
    printf '{\n'
    printf '  "format": "continuitydb.ci_artifact_inventory_failure",\n'
    printf '  "format_version": 1,\n'
    printf '  "generated_by": "scripts/ci_artifact_inventory.sh",\n'
    printf '  "valid": false,\n'
    printf '  "failure": {\n'
    printf '    "stage": "ci_artifact_inventory",\n'
    printf '    "message": '
    json_string "$message"
    printf '\n'
    printf '  },\n'
    printf '  "artifact_roots": {\n'
    printf '    "alpha_workflow": '
    json_string "$alpha_dir"
    printf ',\n'
    printf '    "local_model_quality_gate_smoke": '
    json_string "$smoke_dir"
    printf ',\n'
    printf '    "release_preflight": '
    json_string "$release_dir"
    printf '\n'
    printf '  }\n'
    printf '}\n'
  } >"$failure_report_path"
}

fail() {
  local message="$1"
  rm -f "$inventory_path"
  write_failure_report "$message"
  printf 'ci artifact inventory failed: %s\n' "$message" >&2
  printf 'CI artifact inventory failure report: %s\n' "$failure_report_path" >&2
  exit 1
}

require_file() {
  local path="$1"
  [[ -s "$path" ]] || fail "missing or empty artifact: $path"
}

require_grep() {
  local pattern="$1"
  local path="$2"
  grep -Fq "$pattern" "$path" || fail "artifact $path does not contain required pattern: $pattern"
}

file_count() {
  local dir="$1"
  [[ -d "$dir" ]] || fail "missing artifact directory: $dir"
  find "$dir" -type f | wc -l | tr -d ' '
}

write_json_string_array() {
  local indent="$1"
  shift
  local item

  printf '[\n'
  while (($# > 0)); do
    item="$1"
    shift
    printf '%s' "$indent"
    json_string "$item"
    if (($# > 0)); then
      printf ','
    fi
    printf '\n'
  done
  printf '    ]'
}

require_grep_checks() {
  local check path pattern

  for check in "$@"; do
    path="${check%%|*}"
    pattern="${check#*|}"
    require_grep "$pattern" "$path"
  done
}

write_json_check_array() {
  local check path pattern

  printf '[\n'
  while (($# > 0)); do
    check="$1"
    shift
    path="${check%%|*}"
    pattern="${check#*|}"
    printf '      {\n'
    printf '        "path": '
    json_string "$path"
    printf ',\n'
    printf '        "required_pattern": '
    json_string "$pattern"
    printf '\n'
    printf '      }'
    if (($# > 0)); then
      printf ','
    fi
    printf '\n'
  done
  printf '    ]'
}

rm -f "$inventory_path" "$failure_report_path"

alpha_required_artifacts=(
  "$alpha_dir/proof-obligations.json"
  "$alpha_dir/proof-obligations-validation.json"
  "$alpha_dir/context-collapse-drill.json"
  "$alpha_dir/workload/workload-report.json"
  "$alpha_dir/workload/workload-validation.json"
  "$alpha_dir/workload/continuitydb-workload.manifest.json"
  "$alpha_dir/workload/workload-cells.json"
  "$alpha_dir/workload/checkout-request.json"
  "$alpha_dir/query/checkout-summary.query"
  "$alpha_dir/query/checkout-summary.json"
  "$alpha_dir/query/checkout-summary-validation.json"
  "$alpha_dir/query/checkout-result.json"
  "$alpha_dir/query/checkout-result-validation.json"
  "$alpha_dir/query/checkout-cells.query"
  "$alpha_dir/query/checkout-cells.json"
  "$alpha_dir/query/checkout-cells-result.json"
  "$alpha_dir/query/checkout-cells-result-validation.json"
  "$alpha_dir/query/checkout-context-packets.query"
  "$alpha_dir/query/checkout-context-packets.json"
  "$alpha_dir/query/checkout-context-packets-validation.json"
  "$alpha_dir/replay/replay-report.json"
  "$alpha_dir/replay/continuitydb-workload-replay.manifest.json"
  "$alpha_dir/inspect/inspect-kernel-report.json"
  "$alpha_dir/inspect/continuitydb-inspect-kernel.manifest.json"
  "$alpha_dir/store.jsonl"
  "$alpha_dir/store.jsonl.index.json"
  "$alpha_dir/replay-store.jsonl"
  "$alpha_dir/replay-store.jsonl.index.json"
)

for artifact in "${alpha_required_artifacts[@]}"; do
  require_file "$artifact"
done
alpha_required_checks=(
  "$alpha_dir/proof-obligations-validation.json|\"format\": \"continuitydb.thesis_proof_obligations_validation\""
  "$alpha_dir/proof-obligations-validation.json|\"valid\": true"
  "$alpha_dir/context-collapse-drill.json|\"format\": \"continuitydb.context_collapse_drill\""
  "$alpha_dir/context-collapse-drill.json|\"valid\": true"
  "$alpha_dir/workload/workload-validation.json|\"format\": \"continuitydb.workload.bundle_validation\""
  "$alpha_dir/workload/workload-validation.json|\"valid\": true"
  "$alpha_dir/workload/workload-validation.json|\"cells_parseable\": true"
  "$alpha_dir/workload/workload-validation.json|\"checkout_request_parseable\": true"
  "$alpha_dir/workload/workload-report.json|\"revision_link_count\": 14"
  "$alpha_dir/workload/continuitydb-workload.manifest.json|\"revision_link_count\": 14"
  "$alpha_dir/query/checkout-summary.query|RETURN summary_only"
  "$alpha_dir/query/checkout-summary.json|\"format\": \"continuitydb.checkout_query.summary\""
  "$alpha_dir/query/checkout-summary.json|\"selected_cell_count\": 1"
  "$alpha_dir/query/checkout-summary.json|\"selected_cell_count\""
  "$alpha_dir/query/checkout-summary.json|\"selection_reason_counts\""
  "$alpha_dir/query/checkout-summary.json|\"invalidation_condition_count\""
  "$alpha_dir/query/checkout-summary.json|\"bounded_by_token_budget\""
  "$alpha_dir/query/checkout-summary-validation.json|\"format\": \"continuitydb.checkout_query.summary_validation\""
  "$alpha_dir/query/checkout-summary-validation.json|\"valid\": true"
  "$alpha_dir/query/checkout-summary-validation.json|\"selected_cell_count\": 1"
  "$alpha_dir/query/checkout-summary-validation.json|\"invalidation_condition_count\""
  "$alpha_dir/query/checkout-summary-validation.json|\"bounded_by_token_budget\": true"
  "$alpha_dir/query/checkout-result.json|\"format\": \"continuitydb.checkout_query.result\""
  "$alpha_dir/query/checkout-result.json|\"return_shape\": \"summary_only\""
  "$alpha_dir/query/checkout-result.json|\"type\": \"summary\""
  "$alpha_dir/query/checkout-result.json|\"selected_cell_count\": 1"
  "$alpha_dir/query/checkout-result-validation.json|\"format\": \"continuitydb.checkout_query.result_validation\""
  "$alpha_dir/query/checkout-result-validation.json|\"valid\": true"
  "$alpha_dir/query/checkout-result-validation.json|\"return_shape\": \"summary_only\""
  "$alpha_dir/query/checkout-result-validation.json|\"selected_cell_count\": 1"
  "$alpha_dir/query/checkout-cells.query|RETURN cells_only"
  "$alpha_dir/query/checkout-cells.json|\"format\": \"continuitydb.checkout_query.cells\""
  "$alpha_dir/query/checkout-cells.json|\"cells\""
  "$alpha_dir/query/checkout-cells.json|\"payload\""
  "$alpha_dir/query/checkout-cells-result.json|\"format\": \"continuitydb.checkout_query.result\""
  "$alpha_dir/query/checkout-cells-result.json|\"return_shape\": \"cells_only\""
  "$alpha_dir/query/checkout-cells-result.json|\"type\": \"cells\""
  "$alpha_dir/query/checkout-cells-result-validation.json|\"format\": \"continuitydb.checkout_query.result_validation\""
  "$alpha_dir/query/checkout-cells-result-validation.json|\"valid\": true"
  "$alpha_dir/query/checkout-cells-result-validation.json|\"return_shape\": \"cells_only\""
  "$alpha_dir/query/checkout-cells-result-validation.json|\"result_type\": \"cells\""
  "$alpha_dir/query/checkout-cells-result-validation.json|\"cell_count\": 1"
  "$alpha_dir/query/checkout-context-packets.query|RETURN context_packets_only"
  "$alpha_dir/query/checkout-context-packets.json|\"format\": \"continuitydb.checkout_query.context_packets\""
  "$alpha_dir/query/checkout-context-packets.json|\"context_packets\""
  "$alpha_dir/query/checkout-context-packets.json|\"strategy\""
  "$alpha_dir/query/checkout-context-packets.json|\"compiler_policy\""
  "$alpha_dir/query/checkout-context-packets.json|\"abstraction_level\""
  "$alpha_dir/query/checkout-context-packets.json|\"compiler_reason_tags\""
  "$alpha_dir/query/checkout-context-packets.json|\"compiler_evidence_locators\""
  "$alpha_dir/query/checkout-context-packets.json|\"origin\""
  "$alpha_dir/query/checkout-context-packets.json|\"cell_id\""
  "$alpha_dir/query/checkout-context-packets.json|\"valid_time\""
  "$alpha_dir/query/checkout-context-packets.json|\"commit_id\""
  "$alpha_dir/query/checkout-context-packets.json|\"entries\""
  "$alpha_dir/query/checkout-context-packets.json|\"source\""
  "$alpha_dir/query/checkout-context-packets.json|\"text\""
  "$alpha_dir/query/checkout-context-packets.json|\"confidence\""
  "$alpha_dir/query/checkout-context-packets.json|\"token_count\""
  "$alpha_dir/query/checkout-context-packets.json|\"citations\""
  "$alpha_dir/query/checkout-context-packets.json|\"dependency_context\""
  "$alpha_dir/query/checkout-context-packets.json|\"revision_context\""
  "$alpha_dir/query/checkout-context-packets.json|\"target\""
  "$alpha_dir/query/checkout-context-packets.json|\"kind\""
  "$alpha_dir/query/checkout-context-packets.json|\"rationale\""
  "$alpha_dir/query/checkout-context-packets.json|\"related_cell_id\""
  "$alpha_dir/query/checkout-context-packets.json|\"relation\""
  "$alpha_dir/query/checkout-context-packets.json|\"max_confidence\""
  "$alpha_dir/query/checkout-context-packets.json|\"epistemic_action_reasons\""
  "$alpha_dir/query/checkout-context-packets.json|\"answerability_questions\""
  "$alpha_dir/query/checkout-context-packets.json|\"context_gaps\""
  "$alpha_dir/query/checkout-context-packets.json|\"invalidation_conditions\""
  "$alpha_dir/query/checkout-context-packets.json|\"trajectory_memory\""
  "$alpha_dir/query/checkout-context-packets.json|\"epistemic_pressure\""
  "$alpha_dir/query/checkout-context-packets.json|\"expectation\""
  "$alpha_dir/query/checkout-context-packets.json|\"attention\""
  "$alpha_dir/query/checkout-context-packets.json|\"salience_score\""
  "$alpha_dir/query/checkout-context-packets.json|\"context_affordance\""
  "$alpha_dir/query/checkout-context-packets.json|\"context_affordance_score\""
  "$alpha_dir/query/checkout-context-packets.json|\"lifecycle_policy\""
  "$alpha_dir/query/checkout-context-packets-validation.json|\"format\": \"continuitydb.checkout_query.context_packets_validation\""
  "$alpha_dir/query/checkout-context-packets-validation.json|\"valid\": true"
  "$alpha_dir/query/checkout-context-packets-validation.json|\"context_packet_count\": 1"
  "$alpha_dir/replay/replay-report.json|\"passed\": true"
  "$alpha_dir/replay/replay-report.json|\"revision_link_count\": 14"
  "$alpha_dir/replay/continuitydb-workload-replay.manifest.json|\"revision_link_count\": 14"
  "$alpha_dir/inspect/inspect-kernel-report.json|\"satisfies\": true"
  "$alpha_dir/inspect/inspect-kernel-report.json|\"persistent_index_checkpoint_present_on_open\": true"
  "$alpha_dir/inspect/inspect-kernel-report.json|\"persistent_index_checkpoint_trusted_on_open\": true"
  "$alpha_dir/inspect/inspect-kernel-report.json|\"persistent_index_checkpoint_rebuilt_on_open\": false"
)
require_grep_checks "${alpha_required_checks[@]}"

smoke_required_artifacts=(
  "$smoke_dir/quality-gate-plan.json"
  "$smoke_dir/quality-gate-plan-validation.json"
  "$smoke_dir/candidates.json"
  "$smoke_dir/candidates-validation.json"
  "$smoke_dir/acceptance-criteria.json"
  "$smoke_dir/acceptance-criteria-validation.json"
  "$smoke_dir/evaluation-suite.json"
  "$smoke_dir/evaluation-suite-validation.json"
  "$smoke_dir/dry-run.json"
  "$smoke_dir/dry-run-validation.json"
  "$smoke_dir/quality-gate-run.json"
  "$smoke_dir/quality-gate-run-report.json"
  "$smoke_dir/quality-gate-run-validation.json"
  "$smoke_dir/quality-gate-run-output-validation.json"
  "$smoke_dir/operator-gate/gate-run.json"
  "$smoke_dir/operator-gate/gate-run-report.json"
  "$smoke_dir/operator-gate/gate-run-validation.json"
  "$smoke_dir/operator-gate/gate-run-output-validation.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.schema.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.gbnf"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.gbnf"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.schema.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.gbnf"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.gbnf"
  "$smoke_dir/quality-gate-status.json"
  "$smoke_dir/quality-gate-status-validation.json"
  "$smoke_dir/require-ready-status.json"
  "$smoke_dir/require-ready-status-validation.json"
  "$smoke_dir/require-ready.stderr"
  "$smoke_dir/contract/local-model-response.schema.json"
  "$smoke_dir/contract/local-model-response.gbnf"
  "$smoke_dir/contract/local-model-context-compiler-response.schema.json"
  "$smoke_dir/contract/local-model-context-compiler-response.gbnf"
  "$smoke_dir/gate-artifacts/qwen-qwen2-5-0-5b-instruct/benchmark-report.json"
  "$smoke_dir/gate-artifacts/qwen-qwen2-5-0-5b-instruct/validation-report.json"
  "$smoke_dir/gate-artifacts/qwen-qwen3-0-6b/benchmark-report.json"
  "$smoke_dir/gate-artifacts/qwen-qwen3-0-6b/validation-report.json"
)

for artifact in "${smoke_required_artifacts[@]}"; do
  require_file "$artifact"
done
prompt_count="$(find "$smoke_dir/prompts" -type f -name '*.prompt.txt' | wc -l | tr -d ' ')"
[[ "$prompt_count" == "9" ]] || fail "expected 9 local-model smoke prompts, found $prompt_count"
smoke_required_checks=(
  "$smoke_dir/quality-gate-plan.json|\"format\": \"continuitydb.local_model_quality_gate_plan\""
  "$smoke_dir/quality-gate-plan-validation.json|\"format\": \"continuitydb.local_model_quality_gate_plan_validation\""
  "$smoke_dir/quality-gate-plan-validation.json|\"valid\": true"
  "$smoke_dir/quality-gate-plan-validation.json|\"candidate_count\": 2"
  "$smoke_dir/quality-gate-plan-validation.json|\"total_steps\": 4"
  "$smoke_dir/quality-gate-plan-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/quality-gate-plan.json\""
  "$smoke_dir/candidates.json|\"format\": \"continuitydb.local_model_candidates\""
  "$smoke_dir/candidates-validation.json|\"format\": \"continuitydb.local_model_candidates_validation\""
  "$smoke_dir/candidates-validation.json|\"valid\": true"
  "$smoke_dir/candidates-validation.json|\"total_candidates\": 4"
  "$smoke_dir/candidates-validation.json|\"quality_gate_candidate_count\": 2"
  "$smoke_dir/candidates-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/candidates.json\""
  "$smoke_dir/acceptance-criteria.json|\"format\": \"continuitydb.local_model_acceptance_criteria\""
  "$smoke_dir/acceptance-criteria.json|\"required_criteria_count\": 7"
  "$smoke_dir/acceptance-criteria.json|\"valid_json_schema_conformance\""
  "$smoke_dir/acceptance-criteria.json|\"conflict_versus_supersession_classification\""
  "$smoke_dir/acceptance-criteria.json|\"evidence_citation_preservation\""
  "$smoke_dir/acceptance-criteria.json|\"unsupported_claim_avoidance\""
  "$smoke_dir/acceptance-criteria.json|\"stable_low_temperature_output\""
  "$smoke_dir/acceptance-criteria.json|\"explicit_uncertainty_for_insufficient_evidence\""
  "$smoke_dir/acceptance-criteria.json|\"deterministic_policy_rejection_avoidance\""
  "$smoke_dir/acceptance-criteria-validation.json|\"format\": \"continuitydb.local_model_acceptance_criteria_validation\""
  "$smoke_dir/acceptance-criteria-validation.json|\"valid\": true"
  "$smoke_dir/acceptance-criteria-validation.json|\"required_criteria_count\": 7"
  "$smoke_dir/acceptance-criteria-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/acceptance-criteria.json\""
  "$smoke_dir/evaluation-suite.json|\"complete\": true"
  "$smoke_dir/evaluation-suite-validation.json|\"format\": \"continuitydb.local_model_evaluation_suite_validation\""
  "$smoke_dir/evaluation-suite-validation.json|\"valid\": true"
  "$smoke_dir/evaluation-suite-validation.json|\"total_cases\": 9"
  "$smoke_dir/evaluation-suite-validation.json|\"complete\": true"
  "$smoke_dir/evaluation-suite-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/evaluation-suite.json\""
  "$smoke_dir/dry-run.json|\"dry_run\": true"
  "$smoke_dir/dry-run-validation.json|\"format\": \"continuitydb.local_model_benchmark_report_validation\""
  "$smoke_dir/dry-run-validation.json|\"valid\": true"
  "$smoke_dir/dry-run-validation.json|\"dry_run\": true"
  "$smoke_dir/dry-run-validation.json|\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
  "$smoke_dir/dry-run-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/dry-run.json\""
  "$smoke_dir/quality-gate-run-validation.json|\"format\": \"continuitydb.local_model_quality_gate_run_report_validation\""
  "$smoke_dir/quality-gate-run-validation.json|\"valid\": true"
  "$smoke_dir/quality-gate-run-validation.json|\"acceptance_coverage\""
  "$smoke_dir/quality-gate-run-validation.json|\"candidate_count\": 2"
  "$smoke_dir/quality-gate-run-validation.json|\"complete_candidate_count\": 2"
  "$smoke_dir/quality-gate-run-report.json|\"ready\": true"
  "$smoke_dir/quality-gate-run-output-validation.json|\"format\": \"continuitydb.local_model_quality_gate_run_output_validation\""
  "$smoke_dir/quality-gate-run-output-validation.json|\"valid\": true"
  "$smoke_dir/quality-gate-run-output-validation.json|\"matches_report_file\": true"
  "$smoke_dir/quality-gate-run-output-validation.json|\"candidate_count\": 2"
  "$smoke_dir/quality-gate-run-output-validation.json|\"complete_candidate_count\": 2"
  "$smoke_dir/quality-gate-run-output-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/quality-gate-run.json\""
  "$smoke_dir/quality-gate-status.json|\"ready\": false"
  "$smoke_dir/quality-gate-status.json|\"missing_artifacts\": 2"
  "$smoke_dir/quality-gate-status.json|\"readiness_blockers\""
  "$smoke_dir/quality-gate-status.json|\"recommended_action\": \"run_quality_gate_candidate\""
  "$smoke_dir/quality-gate-status.json|\"run_quality_gate_candidate\": 2"
  "$smoke_dir/quality-gate-status-validation.json|\"format\": \"continuitydb.local_model_quality_gate_status_validation\""
  "$smoke_dir/quality-gate-status-validation.json|\"valid\": true"
  "$smoke_dir/quality-gate-status-validation.json|\"candidate_count\": 2"
  "$smoke_dir/quality-gate-status-validation.json|\"readiness_blocker_count\": 2"
  "$smoke_dir/quality-gate-status-validation.json|\"run_quality_gate_candidate_actions\": 2"
  "$smoke_dir/quality-gate-status-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/quality-gate-status.json\""
  "$smoke_dir/require-ready-status.json|\"format\": \"continuitydb.local_model_quality_gate_status\""
  "$smoke_dir/require-ready-status.json|\"ready\": false"
  "$smoke_dir/require-ready-status.json|\"readiness_blockers\""
  "$smoke_dir/require-ready-status.json|\"run_quality_gate_candidate\": 2"
  "$smoke_dir/require-ready.stderr|local model quality gate is not ready"
  "$smoke_dir/require-ready-status-validation.json|\"format\": \"continuitydb.local_model_require_ready_status_validation\""
  "$smoke_dir/require-ready-status-validation.json|\"valid\": true"
  "$smoke_dir/require-ready-status-validation.json|\"ready\": false"
  "$smoke_dir/require-ready-status-validation.json|\"readiness_blocker_count\": 2"
  "$smoke_dir/require-ready-status-validation.json|\"contains_readiness_error\": true"
  "$smoke_dir/require-ready-status-validation.json|\"stderr_path\": \"target/local-model-quality-gate-smoke/require-ready.stderr\""
  "$smoke_dir/operator-gate/gate-run-report.json|\"format\": \"continuitydb.local_model_quality_gate_run\""
  "$smoke_dir/operator-gate/gate-run-report.json|\"generated_by_command\": \"run-local-model-quality-gate\""
  "$smoke_dir/operator-gate/gate-run-report.json|\"dry_run\": true"
  "$smoke_dir/operator-gate/gate-run-report.json|\"candidate_count\": 2"
  "$smoke_dir/operator-gate/gate-run-validation.json|\"valid\": true"
  "$smoke_dir/operator-gate/gate-run-output-validation.json|\"format\": \"continuitydb.local_model_quality_gate_run_output_validation\""
  "$smoke_dir/operator-gate/gate-run-output-validation.json|\"valid\": true"
  "$smoke_dir/operator-gate/gate-run-output-validation.json|\"matches_report_file\": true"
  "$smoke_dir/operator-gate/gate-run-output-validation.json|\"candidate_count\": 2"
  "$smoke_dir/operator-gate/gate-run-output-validation.json|\"report_path\": \"target/local-model-quality-gate-smoke/operator-gate/gate-run.json\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json|\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json|\"complete\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report.json|\"context_compiler_schema_version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json|\"format\": \"continuitydb.local_model_benchmark_report_validation\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json|\"valid\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json|\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/benchmark-report-validation.json|\"dry_run\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"format\": \"continuitydb.local_model_bundle_validation\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"valid\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"candidate_model_id\": \"Qwen/Qwen2.5-0.5B-Instruct\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"dry_run\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"artifact_dir\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"report_path\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/validation-report.json|\"context_compiler_schema_version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json|\"format\": \"continuitydb.local_model.benchmark_bundle\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/local-model-benchmark.manifest.json|\"context_compiler_schema_version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.schema.json|\"title\": \"ContinuityDB Local Model Steward Response\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-response.gbnf|root ::= response"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json|\"title\": \"ContinuityDB Local Model Context Compiler Response\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.schema.json|\"x-continuitydb-schema-version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen2-5-0-5b-instruct/contracts/local-model-context-compiler-response.gbnf|root ::= context-compiler-response"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json|\"candidate_model_id\": \"Qwen/Qwen3-0.6B\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json|\"complete\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report.json|\"context_compiler_schema_version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json|\"format\": \"continuitydb.local_model_benchmark_report_validation\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json|\"valid\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json|\"candidate_model_id\": \"Qwen/Qwen3-0.6B\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/benchmark-report-validation.json|\"dry_run\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"format\": \"continuitydb.local_model_bundle_validation\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"valid\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"candidate_model_id\": \"Qwen/Qwen3-0.6B\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"dry_run\": true"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"artifact_dir\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"report_path\": \"target/local-model-quality-gate-smoke/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/validation-report.json|\"context_compiler_schema_version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json|\"format\": \"continuitydb.local_model.benchmark_bundle\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/local-model-benchmark.manifest.json|\"context_compiler_schema_version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.schema.json|\"title\": \"ContinuityDB Local Model Steward Response\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-response.gbnf|root ::= response"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json|\"title\": \"ContinuityDB Local Model Context Compiler Response\""
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.schema.json|\"x-continuitydb-schema-version\": 2"
  "$smoke_dir/operator-gate/candidates/qwen-qwen3-0-6b/contracts/local-model-context-compiler-response.gbnf|root ::= context-compiler-response"
  "$smoke_dir/contract/local-model-context-compiler-response.schema.json|\"title\": \"ContinuityDB Local Model Context Compiler Response\""
  "$smoke_dir/contract/local-model-context-compiler-response.schema.json|\"x-continuitydb-schema-version\": 2"
  "$smoke_dir/contract/local-model-context-compiler-response.gbnf|root ::= context-compiler-response"
)
require_grep_checks "${smoke_required_checks[@]}"

release_required_artifacts=(
  "$release_dir/proof-obligations.json"
  "$release_dir/proof-obligations-validation.json"
  "$release_dir/release-assets.json"
  "$release_dir/release-assets-validation.json"
  "$release_dir/release-upload-report.json"
  "$release_dir/release-upload-report-validation.json"
  "$release_dir/release-upload-failure-report.json"
  "$release_dir/release-upload-failure-report-validation.json"
  "$release_dir/release-view-failure-report.json"
  "$release_dir/release-view-failure-report-validation.json"
  "$release_dir/release-upload-test-report.json"
  "$release_dir/release-upload-test-report-validation.json"
)

for artifact in "${release_required_artifacts[@]}"; do
  require_file "$artifact"
done
release_required_checks=(
  "$release_dir/proof-obligations-validation.json|\"format\": \"continuitydb.thesis_proof_obligations_validation\""
  "$release_dir/proof-obligations-validation.json|\"valid\": true"
  "$release_dir/release-assets.json|\"format\": \"continuitydb.release_assets\""
  "$release_dir/release-assets.json|\"asset_count\": 12"
  "$release_dir/release-assets-validation.json|\"format\": \"continuitydb.release_assets_validation\""
  "$release_dir/release-assets-validation.json|\"valid\": true"
  "$release_dir/release-assets-validation.json|\"asset_count\": 12"
  "$release_dir/release-assets-validation.json|\"crate_archive_count\": 10"
  "$release_dir/release-assets-validation.json|\"release_preflight_evidence_count\": 2"
  "$release_dir/release-upload-report.json|\"format\": \"continuitydb.release_upload\""
  "$release_dir/release-upload-report.json|\"valid\": true"
  "$release_dir/release-upload-report.json|\"release_tag\": \"v0.1.0\""
  "$release_dir/release-upload-report.json|\"release_repository\": \"syndicat/continuitydb\""
  "$release_dir/release-upload-report.json|\"asset_count\": 1"
  "$release_dir/release-upload-report.json|\"manifest_format\": \"continuitydb.release_assets\""
  "$release_dir/release-upload-report-validation.json|\"format\": \"continuitydb.release_upload_validation\""
  "$release_dir/release-upload-report-validation.json|\"valid\": true"
  "$release_dir/release-upload-report-validation.json|\"asset_count\": 1"
  "$release_dir/release-upload-report-validation.json|\"manifest_asset_count\": 1"
  "$release_dir/release-upload-failure-report.json|\"format\": \"continuitydb.release_upload\""
  "$release_dir/release-upload-failure-report.json|\"valid\": false"
  "$release_dir/release-upload-failure-report.json|\"failure_stage\": \"upload\""
  "$release_dir/release-upload-failure-report.json|\"release_tag\": \"v0.1.0\""
  "$release_dir/release-upload-failure-report.json|\"release_repository\": \"syndicat/continuitydb\""
  "$release_dir/release-upload-failure-report.json|\"manifest_format\": \"continuitydb.release_assets\""
  "$release_dir/release-upload-failure-report-validation.json|\"format\": \"continuitydb.release_upload_validation\""
  "$release_dir/release-upload-failure-report-validation.json|\"valid\": true"
  "$release_dir/release-upload-failure-report-validation.json|\"release_upload_succeeded\": false"
  "$release_dir/release-upload-failure-report-validation.json|\"failure_stage\": \"upload\""
  "$release_dir/release-upload-failure-report-validation.json|\"asset_count\": 1"
  "$release_dir/release-upload-failure-report-validation.json|\"manifest_asset_count\": 1"
  "$release_dir/release-view-failure-report.json|\"format\": \"continuitydb.release_upload\""
  "$release_dir/release-view-failure-report.json|\"valid\": false"
  "$release_dir/release-view-failure-report.json|\"failure_stage\": \"release_view\""
  "$release_dir/release-view-failure-report.json|\"release_tag\": \"v0.1.0\""
  "$release_dir/release-view-failure-report.json|\"release_repository\": \"syndicat/continuitydb\""
  "$release_dir/release-view-failure-report.json|\"manifest_format\": \"continuitydb.release_assets\""
  "$release_dir/release-view-failure-report-validation.json|\"format\": \"continuitydb.release_upload_validation\""
  "$release_dir/release-view-failure-report-validation.json|\"valid\": true"
  "$release_dir/release-view-failure-report-validation.json|\"release_upload_succeeded\": false"
  "$release_dir/release-view-failure-report-validation.json|\"failure_stage\": \"release_view\""
  "$release_dir/release-view-failure-report-validation.json|\"asset_count\": 1"
  "$release_dir/release-view-failure-report-validation.json|\"manifest_asset_count\": 1"
  "$release_dir/release-upload-test-report.json|\"format\": \"continuitydb.release_upload_test\""
  "$release_dir/release-upload-test-report.json|\"valid\": true"
  "$release_dir/release-upload-test-report.json|\"release_view_preflight_checked\": true"
  "$release_dir/release-upload-test-report.json|\"upload_failure_diagnostic_checked\": true"
  "$release_dir/release-upload-test-report.json|\"release_view_failure_diagnostic_checked\": true"
  "$release_dir/release-upload-test-report.json|\"asset_integrity_checked\": true"
  "$release_dir/release-upload-test-report.json|\"duplicate_manifest_checked\": true"
  "$release_dir/release-upload-test-report.json|\"success_report_checked\": true"
  "$release_dir/release-upload-test-report.json|\"success_report_validation_checked\": true"
  "$release_dir/release-upload-test-report.json|\"failure_report_checked\": true"
  "$release_dir/release-upload-test-report.json|\"failure_report_validation_checked\": true"
  "$release_dir/release-upload-test-report.json|\"release_view_failure_report_checked\": true"
  "$release_dir/release-upload-test-report.json|\"release_view_failure_report_validation_checked\": true"
  "$release_dir/release-upload-test-report-validation.json|\"format\": \"continuitydb.release_upload_test_validation\""
  "$release_dir/release-upload-test-report-validation.json|\"valid\": true"
  "$release_dir/release-upload-test-report-validation.json|\"checked_condition_count\": 12"
  "$release_dir/release-upload-test-report-validation.json|\"mocked_gh_invocation_count\""
)
require_grep_checks "${release_required_checks[@]}"

alpha_count="$(file_count "$alpha_dir")"
smoke_count="$(file_count "$smoke_dir")"
release_count="$(file_count "$release_dir")"
alpha_required_artifact_count="${#alpha_required_artifacts[@]}"
smoke_required_artifact_count="${#smoke_required_artifacts[@]}"
release_required_artifact_count="${#release_required_artifacts[@]}"
alpha_required_check_count="${#alpha_required_checks[@]}"
smoke_required_check_count="${#smoke_required_checks[@]}"
release_required_check_count="${#release_required_checks[@]}"
total_required_artifact_count=$((alpha_required_artifact_count + smoke_required_artifact_count + release_required_artifact_count))
total_required_check_count=$((alpha_required_check_count + smoke_required_check_count + release_required_check_count))

mkdir -p "$(dirname "$inventory_path")"
{
  printf '{\n'
  printf '  "format": "continuitydb.ci_artifact_inventory",\n'
  printf '  "format_version": 1,\n'
  printf '  "generated_by": "scripts/ci_artifact_inventory.sh",\n'
  printf '  "valid": true,\n'
  printf '  "artifact_roots": {\n'
  printf '    "alpha_workflow": {\n'
  printf '      "path": '
  json_string "$alpha_dir"
  printf ',\n'
  printf '      "file_count": %s\n' "$alpha_count"
  printf '    },\n'
  printf '    "local_model_quality_gate_smoke": {\n'
  printf '      "path": '
  json_string "$smoke_dir"
  printf ',\n'
  printf '      "file_count": %s,\n' "$smoke_count"
  printf '      "prompt_count": %s\n' "$prompt_count"
  printf '    },\n'
  printf '    "release_preflight": {\n'
  printf '      "path": '
  json_string "$release_dir"
  printf ',\n'
  printf '      "file_count": %s\n' "$release_count"
  printf '    }\n'
  printf '  },\n'
  printf '  "verified_evidence": {\n'
  printf '    "alpha_workflow": {\n'
  printf '      "proof_obligation_count": 8,\n'
  printf '      "proof_obligations_valid": true,\n'
  printf '      "context_collapse_drill_valid": true,\n'
  printf '      "summary_only_checkout_query_retained": true,\n'
  printf '      "checkout_query_summary_validation_valid": true,\n'
  printf '      "checkout_query_result_envelope_retained": true,\n'
  printf '      "checkout_query_result_validation_valid": true,\n'
  printf '      "cells_only_checkout_query_retained": true,\n'
  printf '      "checkout_query_cells_result_validation_valid": true,\n'
  printf '      "context_packets_only_checkout_query_retained": true,\n'
  printf '      "checkout_query_context_packets_validation_valid": true,\n'
  printf '      "workload_bundle_validation_valid": true,\n'
  printf '      "workload_bundle_present": true,\n'
  printf '      "replay_passed": true,\n'
  printf '      "inspect_kernel_satisfies_required_profile": true,\n'
  printf '      "inspect_kernel_persistent_index_checkpoint_trusted": true\n'
  printf '    },\n'
  printf '    "local_model_quality_gate_smoke": {\n'
  printf '      "quality_gate_plan_validation_valid": true,\n'
  printf '      "candidate_registry_validation_valid": true,\n'
  printf '      "quality_gate_candidate_count": 2,\n'
  printf '      "required_acceptance_criteria_count": 7,\n'
  printf '      "acceptance_criteria_validation_valid": true,\n'
  printf '      "evaluation_suite_validation_valid": true,\n'
  printf '      "dry_run_validation_valid": true,\n'
  printf '      "quality_gate_run_output_validation_valid": true,\n'
  printf '      "complete_acceptance_coverage_candidate_count": 2,\n'
  printf '      "prompt_count": %s,\n' "$prompt_count"
  printf '      "dry_run_gate_ready": true,\n'
  printf '      "quality_gate_status_validation_valid": true,\n'
  printf '      "missing_artifact_status_ready": false,\n'
  printf '      "missing_artifact_status_count": 2,\n'
  printf '      "readiness_blocker_count": 2,\n'
  printf '      "run_quality_gate_candidate_action_count": 2,\n'
  printf '      "require_ready_status_retained": true,\n'
  printf '      "require_ready_status_validation_valid": true,\n'
  printf '      "operator_gate_run_report_retained": true,\n'
  printf '      "operator_gate_run_output_validation_valid": true,\n'
  printf '      "operator_gate_candidate_artifact_count": 2,\n'
  printf '      "operator_gate_candidate_benchmark_validation_count": 2,\n'
  printf '      "operator_gate_candidate_bundle_validation_count": 2,\n'
  printf '      "operator_gate_candidate_validation_count": 2,\n'
  printf '      "operator_gate_candidate_contract_artifact_count": 8,\n'
  printf '      "operator_gate_validation_valid": true\n'
  printf '    },\n'
  printf '    "release_preflight": {\n'
  printf '      "proof_obligation_count": 8,\n'
  printf '      "proof_obligations_valid": true,\n'
  printf '      "release_assets_valid": true,\n'
  printf '      "release_asset_count": 12,\n'
  printf '      "release_crate_archive_count": 10,\n'
  printf '      "release_preflight_evidence_count": 2,\n'
  printf '      "release_upload_preflight_valid": true,\n'
  printf '      "release_upload_success_report_retained": true,\n'
  printf '      "release_upload_success_report_validation_valid": true,\n'
  printf '      "release_upload_failure_report_retained": true,\n'
  printf '      "release_upload_failure_report_validation_valid": true,\n'
  printf '      "release_view_failure_report_retained": true,\n'
  printf '      "release_view_failure_report_validation_valid": true,\n'
  printf '      "release_view_failure_diagnostic_checked": true,\n'
  printf '      "release_upload_failure_diagnostic_checked": true,\n'
  printf '      "release_upload_asset_integrity_checked": true,\n'
  printf '      "release_upload_duplicate_manifest_checked": true,\n'
  printf '      "release_upload_test_report_validation_valid": true\n'
  printf '    }\n'
  printf '  },\n'
  printf '  "required_artifacts": {\n'
  printf '    "alpha_workflow": '
  write_json_string_array '      ' "${alpha_required_artifacts[@]}"
  printf ',\n'
  printf '    "local_model_quality_gate_smoke": '
  write_json_string_array '      ' "${smoke_required_artifacts[@]}"
  printf ',\n'
  printf '    "release_preflight": '
  write_json_string_array '      ' "${release_required_artifacts[@]}"
  printf '\n'
  printf '  },\n'
  printf '  "required_checks": {\n'
  printf '    "alpha_workflow": '
  write_json_check_array "${alpha_required_checks[@]}"
  printf ',\n'
  printf '    "local_model_quality_gate_smoke": '
  write_json_check_array "${smoke_required_checks[@]}"
  printf ',\n'
  printf '    "release_preflight": '
  write_json_check_array "${release_required_checks[@]}"
  printf '\n'
  printf '  },\n'
  printf '  "contract_summary": {\n'
  printf '    "alpha_workflow": {\n'
  printf '      "required_artifact_count": %s,\n' "$alpha_required_artifact_count"
  printf '      "required_check_count": %s\n' "$alpha_required_check_count"
  printf '    },\n'
  printf '    "local_model_quality_gate_smoke": {\n'
  printf '      "required_artifact_count": %s,\n' "$smoke_required_artifact_count"
  printf '      "required_check_count": %s\n' "$smoke_required_check_count"
  printf '    },\n'
  printf '    "release_preflight": {\n'
  printf '      "required_artifact_count": %s,\n' "$release_required_artifact_count"
  printf '      "required_check_count": %s\n' "$release_required_check_count"
  printf '    },\n'
  printf '    "total_required_artifact_count": %s,\n' "$total_required_artifact_count"
  printf '    "total_required_check_count": %s\n' "$total_required_check_count"
  printf '  }\n'
  printf '}\n'
} >"$inventory_path"

printf 'ContinuityDB CI artifact inventory passed: %s\n' "$inventory_path"
