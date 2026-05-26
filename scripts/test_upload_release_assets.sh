#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

report_path="${1:-target/release-preflight/release-upload-test-report.json}"
test_validation_path="$(dirname "$report_path")/release-upload-test-report-validation.json"
test_validation_failure_path="$(dirname "$report_path")/release-upload-test-report-validation-failure.json"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

mock_bin="$tmp_dir/bin"
mkdir -p "$mock_bin"

cat >"$mock_bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$*" >>"$GH_MOCK_ARGS_PATH"

expected_repo="${EXPECTED_RELEASE_REPOSITORY:-}"
if [[ -n "$expected_repo" ]]; then
  saw_repo=0
  previous=""
  for arg in "$@"; do
    if [[ "$previous" == "--repo" && "$arg" == "$expected_repo" ]]; then
      saw_repo=1
      break
    fi
    previous="$arg"
  done
  if [[ "$saw_repo" -ne 1 ]]; then
    printf 'missing expected --repo %s\n' "$expected_repo" >&2
    exit 64
  fi
fi

if [[ "${GH_FAIL_UPLOAD:-}" == "1" && "$1 $2" == "release upload" ]]; then
  printf 'HTTP 404: Not Found\n' >&2
  exit 72
fi

if [[ "${GH_FAIL_VIEW:-}" == "1" && "$1 $2" == "release view" ]]; then
  printf 'HTTP 404: Not Found\n' >&2
  exit 73
fi
SH
chmod +x "$mock_bin/gh"

gh_args_path="$tmp_dir/gh-args.txt"
report_dir="$(dirname "$report_path")"
mock_artifact_dir="$report_dir/mock-release-upload"
asset_path="$mock_artifact_dir/proof-obligations.json"
manifest_path="$mock_artifact_dir/release-assets.json"
upload_report_path="$report_dir/release-upload-report.json"
upload_validation_path="$report_dir/release-upload-report-validation.json"
upload_validation_failure_path="$report_dir/release-upload-report-validation-failure.json"
mkdir -p "$mock_artifact_dir"
printf '{"format":"continuitydb.thesis_proof_obligations"}\n' >"$asset_path"

python3 - "$manifest_path" "$asset_path" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path, asset_path = sys.argv[1:3]
asset_bytes = Path(asset_path).read_bytes()
with open(manifest_path, "w", encoding="utf-8") as handle:
    json.dump(
        {
            "format": "continuitydb.release_assets",
            "format_version": 1,
            "assets": [
                {
                    "kind": "release_preflight_evidence",
                    "name": "proof-obligations.json",
                    "path": asset_path,
                    "bytes": len(asset_bytes),
                    "sha256": hashlib.sha256(asset_bytes).hexdigest(),
                }
            ],
        },
        handle,
    )
PY

PATH="$mock_bin:$PATH" \
GH_MOCK_ARGS_PATH="$gh_args_path" \
GITHUB_REPOSITORY="syndicat/continuitydb" \
EXPECTED_RELEASE_REPOSITORY="syndicat/continuitydb" \
  scripts/upload_release_assets.sh v0.1.0 "$manifest_path" "$upload_report_path"

grep -q -- '--repo syndicat/continuitydb' "$gh_args_path"
grep -q -- 'release view v0.1.0 --repo syndicat/continuitydb' "$gh_args_path"
grep -q -- 'release upload v0.1.0' "$gh_args_path"
grep -q -- '"format": "continuitydb.release_upload"' "$upload_report_path"
grep -q -- '"valid": true' "$upload_report_path"
grep -q -- '"release_tag": "v0.1.0"' "$upload_report_path"
grep -q -- '"release_repository": "syndicat/continuitydb"' "$upload_report_path"
grep -q -- '"asset_count": 1' "$upload_report_path"
grep -q -- '"manifest_format": "continuitydb.release_assets"' "$upload_report_path"

cargo run -q -p continuitydb-cli -- validate-release-upload-report \
  --report-path "$upload_report_path" \
  --validation-report-path "$upload_validation_path" \
  --failure-report-path "$upload_validation_failure_path" >/dev/null
grep -q -- '"format": "continuitydb.release_upload_validation"' "$upload_validation_path"
grep -q -- '"valid": true' "$upload_validation_path"
grep -q -- '"asset_count": 1' "$upload_validation_path"
grep -q -- '"manifest_asset_count": 1' "$upload_validation_path"

asset_integrity_stderr="$tmp_dir/asset-integrity.stderr"
asset_integrity_report_path="$tmp_dir/asset-integrity-report.json"
printf 'tampered\n' >>"$asset_path"
if PATH="$mock_bin:$PATH" \
  GH_MOCK_ARGS_PATH="$gh_args_path" \
  GITHUB_REPOSITORY="syndicat/continuitydb" \
  EXPECTED_RELEASE_REPOSITORY="syndicat/continuitydb" \
  scripts/upload_release_assets.sh v0.1.0 "$manifest_path" "$asset_integrity_report_path" 2>"$asset_integrity_stderr"; then
  printf 'expected release asset integrity failure\n' >&2
  exit 1
fi
grep -q -- 'release asset manifest byte count mismatch' "$asset_integrity_stderr"

duplicate_manifest_stderr="$tmp_dir/duplicate-manifest.stderr"
duplicate_manifest_report_path="$tmp_dir/duplicate-manifest-report.json"
python3 - "$manifest_path" "$asset_path" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path, asset_path = sys.argv[1:3]
asset_bytes = Path(asset_path).read_bytes()
with open(manifest_path, "r", encoding="utf-8") as handle:
    manifest = json.load(handle)
manifest["assets"][0]["bytes"] = len(asset_bytes)
manifest["assets"][0]["sha256"] = hashlib.sha256(asset_bytes).hexdigest()
manifest["assets"].append(dict(manifest["assets"][0]))
with open(manifest_path, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle)
PY
if PATH="$mock_bin:$PATH" \
  GH_MOCK_ARGS_PATH="$gh_args_path" \
  GITHUB_REPOSITORY="syndicat/continuitydb" \
  EXPECTED_RELEASE_REPOSITORY="syndicat/continuitydb" \
  scripts/upload_release_assets.sh v0.1.0 "$manifest_path" "$duplicate_manifest_report_path" 2>"$duplicate_manifest_stderr"; then
  printf 'expected duplicate release asset manifest failure\n' >&2
  exit 1
fi
grep -q -- 'release asset manifest duplicate asset name' "$duplicate_manifest_stderr"

upload_failure_stderr="$tmp_dir/upload-failure.stderr"
upload_failure_report_path="$report_dir/release-upload-failure-report.json"
upload_failure_validation_path="$report_dir/release-upload-failure-report-validation.json"
upload_failure_validation_failure_path="$report_dir/release-upload-failure-report-validation-failure.json"
python3 - "$manifest_path" "$asset_path" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path, asset_path = sys.argv[1:3]
asset_bytes = Path(asset_path).read_bytes()
with open(manifest_path, "r", encoding="utf-8") as handle:
    manifest = json.load(handle)
manifest["assets"] = manifest["assets"][:1]
manifest["assets"][0]["bytes"] = len(asset_bytes)
manifest["assets"][0]["sha256"] = hashlib.sha256(asset_bytes).hexdigest()
with open(manifest_path, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle)
PY
if PATH="$mock_bin:$PATH" \
  GH_MOCK_ARGS_PATH="$gh_args_path" \
  GITHUB_REPOSITORY="syndicat/continuitydb" \
  EXPECTED_RELEASE_REPOSITORY="syndicat/continuitydb" \
  GH_FAIL_UPLOAD=1 \
  scripts/upload_release_assets.sh v0.1.0 "$manifest_path" "$upload_failure_report_path" 2>"$upload_failure_stderr"; then
  printf 'expected release asset upload failure\n' >&2
  exit 1
fi
grep -q -- 'gh release upload failed for v0.1.0 in repository syndicat/continuitydb' "$upload_failure_stderr"
grep -q -- '"format": "continuitydb.release_upload"' "$upload_failure_report_path"
grep -q -- '"valid": false' "$upload_failure_report_path"
grep -q -- '"failure_stage": "upload"' "$upload_failure_report_path"
grep -q -- '"release_tag": "v0.1.0"' "$upload_failure_report_path"
grep -q -- '"release_repository": "syndicat/continuitydb"' "$upload_failure_report_path"
grep -q -- '"manifest_format": "continuitydb.release_assets"' "$upload_failure_report_path"

cargo run -q -p continuitydb-cli -- validate-release-upload-report \
  --report-path "$upload_failure_report_path" \
  --validation-report-path "$upload_failure_validation_path" \
  --failure-report-path "$upload_failure_validation_failure_path" >/dev/null
grep -q -- '"format": "continuitydb.release_upload_validation"' "$upload_failure_validation_path"
grep -q -- '"valid": true' "$upload_failure_validation_path"
grep -q -- '"release_upload_succeeded": false' "$upload_failure_validation_path"
grep -q -- '"failure_stage": "upload"' "$upload_failure_validation_path"
grep -q -- '"asset_count": 1' "$upload_failure_validation_path"
grep -q -- '"manifest_asset_count": 1' "$upload_failure_validation_path"

release_view_failure_stderr="$tmp_dir/release-view-failure.stderr"
release_view_failure_report_path="$report_dir/release-view-failure-report.json"
release_view_failure_validation_path="$report_dir/release-view-failure-report-validation.json"
release_view_failure_validation_failure_path="$report_dir/release-view-failure-report-validation-failure.json"
if PATH="$mock_bin:$PATH" \
  GH_MOCK_ARGS_PATH="$gh_args_path" \
  GITHUB_REPOSITORY="syndicat/continuitydb" \
  EXPECTED_RELEASE_REPOSITORY="syndicat/continuitydb" \
  GH_FAIL_VIEW=1 \
  scripts/upload_release_assets.sh v0.1.0 "$manifest_path" "$release_view_failure_report_path" 2>"$release_view_failure_stderr"; then
  printf 'expected release view failure\n' >&2
  exit 1
fi
grep -q -- 'release v0.1.0 was not found in repository syndicat/continuitydb' "$release_view_failure_stderr"
grep -q -- '"format": "continuitydb.release_upload"' "$release_view_failure_report_path"
grep -q -- '"valid": false' "$release_view_failure_report_path"
grep -q -- '"failure_stage": "release_view"' "$release_view_failure_report_path"
grep -q -- '"release_tag": "v0.1.0"' "$release_view_failure_report_path"
grep -q -- '"release_repository": "syndicat/continuitydb"' "$release_view_failure_report_path"
grep -q -- '"manifest_format": "continuitydb.release_assets"' "$release_view_failure_report_path"

cargo run -q -p continuitydb-cli -- validate-release-upload-report \
  --report-path "$release_view_failure_report_path" \
  --validation-report-path "$release_view_failure_validation_path" \
  --failure-report-path "$release_view_failure_validation_failure_path" >/dev/null
grep -q -- '"format": "continuitydb.release_upload_validation"' "$release_view_failure_validation_path"
grep -q -- '"valid": true' "$release_view_failure_validation_path"
grep -q -- '"release_upload_succeeded": false' "$release_view_failure_validation_path"
grep -q -- '"failure_stage": "release_view"' "$release_view_failure_validation_path"
grep -q -- '"asset_count": 1' "$release_view_failure_validation_path"
grep -q -- '"manifest_asset_count": 1' "$release_view_failure_validation_path"

printf '{"format":"continuitydb.thesis_proof_obligations"}\n' >"$asset_path"
python3 - "$manifest_path" "$asset_path" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path, asset_path = sys.argv[1:3]
asset_bytes = Path(asset_path).read_bytes()
with open(manifest_path, "w", encoding="utf-8") as handle:
    json.dump(
        {
            "format": "continuitydb.release_assets",
            "format_version": 1,
            "assets": [
                {
                    "kind": "release_preflight_evidence",
                    "name": "proof-obligations.json",
                    "path": asset_path,
                    "bytes": len(asset_bytes),
                    "sha256": hashlib.sha256(asset_bytes).hexdigest(),
                }
            ],
        },
        handle,
    )
PY
PATH="$mock_bin:$PATH" \
GH_MOCK_ARGS_PATH="$gh_args_path" \
GITHUB_REPOSITORY="syndicat/continuitydb" \
EXPECTED_RELEASE_REPOSITORY="syndicat/continuitydb" \
  scripts/upload_release_assets.sh v0.1.0 "$manifest_path" "$upload_report_path"
cargo run -q -p continuitydb-cli -- validate-release-upload-report \
  --report-path "$upload_report_path" \
  --validation-report-path "$upload_validation_path" \
  --failure-report-path "$upload_validation_failure_path" >/dev/null
grep -q -- '"valid": true' "$upload_report_path"
grep -q -- '"valid": true' "$upload_validation_path"

mkdir -p "$report_dir"
python3 - "$report_path" "$gh_args_path" "$upload_failure_stderr" "$release_view_failure_stderr" "$asset_integrity_stderr" "$duplicate_manifest_stderr" "$upload_report_path" "$upload_failure_report_path" "$release_view_failure_report_path" "$upload_validation_path" "$upload_failure_validation_path" "$release_view_failure_validation_path" <<'PY'
import json
import sys
from pathlib import Path

report_path, gh_args_path, upload_failure_stderr, release_view_failure_stderr, asset_integrity_stderr, duplicate_manifest_stderr, upload_report_path, upload_failure_report_path, release_view_failure_report_path, upload_validation_path, upload_failure_validation_path, release_view_failure_validation_path = sys.argv[1:13]
gh_invocations = Path(gh_args_path).read_text(encoding="utf-8").splitlines()
failure_stderr = Path(upload_failure_stderr).read_text(encoding="utf-8")
release_view_stderr = Path(release_view_failure_stderr).read_text(encoding="utf-8")
integrity_stderr = Path(asset_integrity_stderr).read_text(encoding="utf-8")
duplicate_stderr = Path(duplicate_manifest_stderr).read_text(encoding="utf-8")
upload_report = json.loads(Path(upload_report_path).read_text(encoding="utf-8"))
upload_failure_report = json.loads(
    Path(upload_failure_report_path).read_text(encoding="utf-8")
)
release_view_failure_report = json.loads(
    Path(release_view_failure_report_path).read_text(encoding="utf-8")
)
upload_validation = json.loads(Path(upload_validation_path).read_text(encoding="utf-8"))
upload_failure_validation = json.loads(
    Path(upload_failure_validation_path).read_text(encoding="utf-8")
)
release_view_failure_validation = json.loads(
    Path(release_view_failure_validation_path).read_text(encoding="utf-8")
)
with open(report_path, "w", encoding="utf-8") as handle:
    json.dump(
        {
            "format": "continuitydb.release_upload_test",
            "format_version": 1,
            "generated_by": "scripts/test_upload_release_assets.sh",
            "valid": True,
            "release_tag": "v0.1.0",
            "release_repository": "syndicat/continuitydb",
            "release_view_preflight_checked": any(
                invocation
                == "release view v0.1.0 --repo syndicat/continuitydb"
                for invocation in gh_invocations
            ),
            "release_upload_repo_checked": any(
                invocation.startswith("release upload v0.1.0 ")
                and "--repo syndicat/continuitydb" in invocation
                for invocation in gh_invocations
            ),
            "upload_failure_diagnostic_checked": (
                "gh release upload failed for v0.1.0 in repository syndicat/continuitydb"
                in failure_stderr
            ),
            "release_view_failure_diagnostic_checked": (
                "release v0.1.0 was not found in repository syndicat/continuitydb"
                in release_view_stderr
            ),
            "asset_integrity_checked": (
                "release asset manifest byte count mismatch" in integrity_stderr
            ),
            "duplicate_manifest_checked": (
                "release asset manifest duplicate asset name" in duplicate_stderr
            ),
            "success_report_checked": (
                upload_report.get("format") == "continuitydb.release_upload"
                and upload_report.get("valid") is True
                and upload_report.get("release_tag") == "v0.1.0"
                and upload_report.get("release_repository") == "syndicat/continuitydb"
                and upload_report.get("asset_count") == 1
                and upload_report.get("manifest", {}).get("manifest_format")
                == "continuitydb.release_assets"
            ),
            "success_report_validation_checked": (
                upload_validation.get("format")
                == "continuitydb.release_upload_validation"
                and upload_validation.get("valid") is True
                and upload_validation.get("release_tag") == "v0.1.0"
                and upload_validation.get("release_repository")
                == "syndicat/continuitydb"
                and upload_validation.get("asset_count") == 1
                and upload_validation.get("manifest_asset_count") == 1
            ),
            "failure_report_checked": (
                upload_failure_report.get("format")
                == "continuitydb.release_upload"
                and upload_failure_report.get("valid") is False
                and upload_failure_report.get("failure_stage") == "upload"
                and upload_failure_report.get("release_tag") == "v0.1.0"
                and upload_failure_report.get("release_repository")
                == "syndicat/continuitydb"
                and upload_failure_report.get("manifest", {}).get(
                    "manifest_format"
                )
                == "continuitydb.release_assets"
            ),
            "failure_report_validation_checked": (
                upload_failure_validation.get("format")
                == "continuitydb.release_upload_validation"
                and upload_failure_validation.get("valid") is True
                and upload_failure_validation.get("release_upload_succeeded") is False
                and upload_failure_validation.get("failure_stage") == "upload"
                and upload_failure_validation.get("asset_count") == 1
                and upload_failure_validation.get("manifest_asset_count") == 1
            ),
            "release_view_failure_report_checked": (
                release_view_failure_report.get("format")
                == "continuitydb.release_upload"
                and release_view_failure_report.get("valid") is False
                and release_view_failure_report.get("failure_stage")
                == "release_view"
                and release_view_failure_report.get("release_tag") == "v0.1.0"
                and release_view_failure_report.get("release_repository")
                == "syndicat/continuitydb"
                and release_view_failure_report.get("manifest", {}).get(
                    "manifest_format"
                )
                == "continuitydb.release_assets"
            ),
            "release_view_failure_report_validation_checked": (
                release_view_failure_validation.get("format")
                == "continuitydb.release_upload_validation"
                and release_view_failure_validation.get("valid") is True
                and release_view_failure_validation.get("release_upload_succeeded")
                is False
                and release_view_failure_validation.get("failure_stage")
                == "release_view"
                and release_view_failure_validation.get("asset_count") == 1
                and release_view_failure_validation.get("manifest_asset_count") == 1
            ),
            "mocked_gh_invocation_count": len(gh_invocations),
        },
        handle,
        indent=2,
        sort_keys=True,
    )
    handle.write("\n")
PY

cargo run -q -p continuitydb-cli -- validate-release-upload-test-report \
  --report-path "$report_path" \
  --validation-report-path "$test_validation_path" \
  --failure-report-path "$test_validation_failure_path" >/dev/null
grep -q -- '"format": "continuitydb.release_upload_test_validation"' "$test_validation_path"
grep -q -- '"valid": true' "$test_validation_path"
grep -q -- '"checked_condition_count": 12' "$test_validation_path"
