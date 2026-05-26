#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

fail() {
  printf 'local model quality gate failed: %s\n' "$1" >&2
  exit 1
}

is_enabled() {
  case "${1:-0}" in
    1|true|TRUE|yes|YES|on|ON) return 0 ;;
    *) return 1 ;;
  esac
}

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
dry_run="${CONTINUITYDB_LOCAL_MODEL_DRY_RUN:-0}"
runner="${CONTINUITYDB_LOCAL_MODEL_RUNNER:-}"
artifact_root="${CONTINUITYDB_LOCAL_MODEL_ARTIFACT_ROOT:-local-model-artifacts/quality-gate-$timestamp}"
baseline_path="${CONTINUITYDB_LOCAL_MODEL_BASELINE_PATH:-local-model-artifacts/local-model-baselines.jsonl}"
qwen25_path="${CONTINUITYDB_QWEN25_GGUF:-}"
qwen3_path="${CONTINUITYDB_QWEN3_GGUF:-}"
fail_on_not_ready="${CONTINUITYDB_LOCAL_MODEL_FAIL_ON_NOT_READY:-1}"
stability_trials="${CONTINUITYDB_LOCAL_MODEL_STABILITY_TRIALS:-}"
fail_on_unstable="${CONTINUITYDB_LOCAL_MODEL_FAIL_ON_UNSTABLE:-0}"

if is_enabled "$dry_run"; then
  runner="${runner:-/bin/echo}"
  qwen25_path="${qwen25_path:-$artifact_root/dry-run-qwen2.5.gguf}"
  qwen3_path="${qwen3_path:-$artifact_root/dry-run-qwen3.gguf}"
else
  [[ -n "$runner" ]] || fail "CONTINUITYDB_LOCAL_MODEL_RUNNER must point to the local model executable"
  [[ -n "$qwen25_path" ]] || fail "CONTINUITYDB_QWEN25_GGUF must point to the Qwen2.5 GGUF artifact"
  [[ -n "$qwen3_path" ]] || fail "CONTINUITYDB_QWEN3_GGUF must point to the Qwen3 GGUF artifact"
  [[ -f "$qwen25_path" ]] || fail "Qwen2.5 GGUF artifact does not exist: $qwen25_path"
  [[ -f "$qwen3_path" ]] || fail "Qwen3 GGUF artifact does not exist: $qwen3_path"
fi

mkdir -p "$artifact_root" "$(dirname "$baseline_path")"

run_stdout="$artifact_root/gate-run.json"
run_report="$artifact_root/gate-run-report.json"
failure_report="$artifact_root/gate-run-failure.json"
validation_report="$artifact_root/gate-run-validation.json"
output_validation_report="$artifact_root/gate-run-output-validation.json"

cmd=(
  cargo run -q -p continuitydb-cli --features local-model --
  run-local-model-quality-gate
  --executable "$runner"
  --model-path "Qwen/Qwen2.5-0.5B-Instruct=$qwen25_path"
  --model-path "Qwen/Qwen3-0.6B=$qwen3_path"
  --baseline-path "$baseline_path"
  --artifact-root "$artifact_root/candidates"
  --report-path "$run_report"
  --failure-report-path "$failure_report"
)

if is_enabled "$dry_run"; then
  cmd+=(--dry-run)
fi

if is_enabled "$fail_on_not_ready"; then
  cmd+=(--fail-on-not-ready)
fi

if [[ -n "$stability_trials" ]]; then
  cmd+=(--stability-trials "$stability_trials")
fi

if is_enabled "$fail_on_unstable"; then
  cmd+=(--fail-on-unstable)
fi

set +e
"${cmd[@]}" >"$run_stdout"
status=$?
set -e

report_to_validate="$run_report"
if (( status != 0 )) && [[ -s "$failure_report" ]]; then
  report_to_validate="$failure_report"
fi

if [[ ! -s "$report_to_validate" ]]; then
  fail "quality gate exited with status $status without a structured report"
fi

cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-quality-gate-run-report \
  --report-path "$report_to_validate" \
  --validation-report-path "$validation_report" >/dev/null

if (( status == 0 )); then
  cargo run -q -p continuitydb-cli --features local-model -- validate-local-model-quality-gate-run-output \
    --report-path "$run_stdout" \
    --validation-report-path "$output_validation_report" >/dev/null
fi

printf 'ContinuityDB local model quality gate artifacts: %s\n' "$artifact_root"
printf 'Run stdout: %s\n' "$run_stdout"
printf 'Validated report: %s\n' "$report_to_validate"
printf 'Validation report: %s\n' "$validation_report"
if (( status == 0 )); then
  printf 'Output validation report: %s\n' "$output_validation_report"
fi

if (( status != 0 )); then
  printf 'Quality gate command exited with status %s\n' "$status" >&2
  exit "$status"
fi
