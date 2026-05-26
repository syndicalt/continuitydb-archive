#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

workflow_path=".github/workflows/release.yml"

python3 - "$workflow_path" <<'PY'
import sys
from pathlib import Path

workflow_path = Path(sys.argv[1])
workflow = workflow_path.read_text(encoding="utf-8")

upload_step = "      - name: Upload GitHub Release Assets\n"
validation_step = "      - name: Validate GitHub Release Upload Report\n"
if upload_step not in workflow:
    raise SystemExit("release workflow is missing the asset upload step")
if validation_step not in workflow:
    raise SystemExit("release workflow is missing upload-report validation")
if workflow.index(validation_step) < workflow.index(upload_step):
    raise SystemExit("upload-report validation must run after asset upload")

validation_block = workflow[workflow.index(validation_step) :]
next_step = validation_block.find("\n      - name: ", len(validation_step))
if next_step != -1:
    validation_block = validation_block[:next_step]

required_snippets = [
    "        if: always()\n",
    "validate-release-upload-report",
    "--report-path target/release-preflight/release-upload-report.json",
    "--validation-report-path target/release-preflight/release-upload-report-validation.json",
    "--failure-report-path target/release-preflight/release-upload-report-validation-failure.json",
]
for snippet in required_snippets:
    if snippet not in validation_block:
        raise SystemExit(f"release workflow validation step missing: {snippet.strip()}")
PY
