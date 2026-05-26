#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

report_path="${1:-target/ci-artifact-inventory.json}"
validation_report_path="${2:-target/ci-artifact-inventory-validation.json}"
failure_report_path="${3:-target/ci-artifact-inventory-validation-failure.json}"

rm -f "$validation_report_path" "$failure_report_path"

cargo run -q -p continuitydb-cli -- validate-ci-artifact-inventory \
  --report-path "$report_path" \
  --validation-report-path "$validation_report_path" \
  --failure-report-path "$failure_report_path"
