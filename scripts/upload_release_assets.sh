#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

fail() {
  printf 'release asset upload failed: %s\n' "$1" >&2
  exit 1
}

tag="${1:-}"
manifest_path="${2:-target/release-preflight/release-assets.json}"
upload_report_path="${3:-target/release-preflight/release-upload-report.json}"
release_repository="${RELEASE_REPOSITORY:-${GITHUB_REPOSITORY:-}}"

[[ -n "$tag" ]] || fail "missing release tag argument"
[[ -f "$manifest_path" ]] || fail "missing release asset manifest at $manifest_path"
command -v gh >/dev/null 2>&1 || fail "gh CLI is required"
command -v python3 >/dev/null 2>&1 || fail "python3 is required"
if [[ "${GITHUB_ACTIONS:-}" == "true" && -z "$release_repository" ]]; then
  fail "missing release repository in GitHub Actions"
fi

write_release_upload_report() {
  local valid="$1"
  local failure_stage="$2"
  local error_message="$3"

  mkdir -p "$(dirname "$upload_report_path")"
  python3 - "$manifest_path" "$upload_report_path" "$tag" "$release_repository" "$valid" "$failure_stage" "$error_message" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

(
    manifest_path,
    upload_report_path,
    release_tag,
    release_repository,
    valid_text,
    failure_stage,
    error_message,
) = sys.argv[1:8]

manifest_bytes = b""
manifest = {}
manifest_parseable = False
manifest_parse_error = None
manifest_file = Path(manifest_path)
if manifest_file.is_file():
    manifest_bytes = manifest_file.read_bytes()
    try:
        manifest = json.loads(manifest_bytes.decode("utf-8"))
        manifest_parseable = isinstance(manifest, dict)
        if not manifest_parseable:
            manifest_parse_error = "manifest root is not an object"
            manifest = {}
    except Exception as exc:  # best-effort failure evidence
        manifest_parse_error = str(exc)

assets = manifest.get("assets", [])
if not isinstance(assets, list):
    assets = []
reported_assets = []
for asset in assets:
    if not isinstance(asset, dict):
        continue
    reported_asset = {
        "kind": asset.get("kind"),
        "name": asset.get("name"),
        "path": asset.get("path"),
        "bytes": asset.get("bytes"),
        "sha256": asset.get("sha256"),
    }
    if asset.get("package") is not None:
        reported_asset["package"] = asset.get("package")
    reported_assets.append(reported_asset)

valid = valid_text == "true"
report = {
    "format": "continuitydb.release_upload",
    "format_version": 1,
    "generated_by": "scripts/upload_release_assets.sh",
    "valid": valid,
    "release_tag": release_tag,
    "release_repository": release_repository or None,
    "asset_count": len(assets),
    "manifest": {
        "manifest_path": manifest_path,
        "manifest_parseable": manifest_parseable,
        "manifest_parse_error": manifest_parse_error,
        "manifest_format": manifest.get("format"),
        "manifest_format_version": manifest.get("format_version"),
        "manifest_generated_by": manifest.get("generated_by"),
        "manifest_package_version": manifest.get("package_version"),
        "manifest_bytes": len(manifest_bytes),
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest()
        if manifest_bytes
        else None,
        "manifest_asset_count": manifest.get("asset_count"),
    },
    "assets": reported_assets,
}
if not valid:
    report["failure_stage"] = failure_stage
    report["error"] = error_message

with open(upload_report_path, "w", encoding="utf-8") as handle:
    json.dump(report, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
}

upload_paths_file="$(mktemp)"
cleanup() {
  rm -f "$upload_paths_file"
}
trap cleanup EXIT

if ! python3 - "$manifest_path" >"$upload_paths_file" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path = sys.argv[1]
with open(manifest_path, "r", encoding="utf-8") as handle:
    manifest = json.load(handle)

if manifest.get("format") != "continuitydb.release_assets":
    raise SystemExit("manifest format is not continuitydb.release_assets")

assets = manifest.get("assets")
if not isinstance(assets, list) or not assets:
    raise SystemExit("manifest does not contain a non-empty assets list")

seen_names = set()
seen_paths = set()
for index, asset in enumerate(assets):
    if not isinstance(asset, dict):
        raise SystemExit(f"asset {index} is not an object")
    path = asset.get("path")
    name = asset.get("name")
    expected_bytes = asset.get("bytes")
    expected_sha256 = asset.get("sha256")
    if not isinstance(path, str) or not path:
        raise SystemExit(f"asset {index} has an empty path")
    if not isinstance(name, str) or not name:
        raise SystemExit(f"asset {index} has an empty name")
    if name in seen_names:
        raise SystemExit(f"release asset manifest duplicate asset name: {name}")
    seen_names.add(name)
    if path in seen_paths:
        raise SystemExit(f"release asset manifest duplicate asset path: {path}")
    seen_paths.add(path)
    if not isinstance(expected_bytes, int) or expected_bytes < 0:
        raise SystemExit(f"asset {index} has invalid byte count")
    if not isinstance(expected_sha256, str) or not expected_sha256:
        raise SystemExit(f"asset {index} has an empty sha256")
    asset_path = Path(path)
    if not asset_path.is_file():
        raise SystemExit(f"asset {index} path does not exist: {path}")
    asset_bytes = asset_path.read_bytes()
    if len(asset_bytes) != expected_bytes:
        raise SystemExit(
            f"release asset manifest byte count mismatch for {path}: "
            f"expected {expected_bytes}, actual {len(asset_bytes)}"
        )
    actual_sha256 = hashlib.sha256(asset_bytes).hexdigest()
    if actual_sha256 != expected_sha256:
        raise SystemExit(
            f"release asset manifest sha256 mismatch for {path}: "
            f"expected {expected_sha256}, actual {actual_sha256}"
        )
    print(path)
PY
then
  write_release_upload_report false manifest_validation "release asset manifest validation failed"
  fail "release asset manifest validation failed"
fi

mapfile -t upload_paths <"$upload_paths_file"

((${#upload_paths[@]} > 0)) || fail "release asset manifest produced no upload paths"

for path in "${upload_paths[@]}"; do
  [[ -n "$path" ]] || fail "release asset manifest produced an empty upload path"
  [[ -f "$path" ]] || fail "release asset path does not exist: $path"
done

gh_args=(release upload "$tag")
if [[ -n "$release_repository" ]]; then
  gh_args+=(--repo "$release_repository")
fi
gh_args+=("${upload_paths[@]}" --clobber)

release_view_args=(release view "$tag")
if [[ -n "$release_repository" ]]; then
  release_view_args+=(--repo "$release_repository")
fi
if ! gh "${release_view_args[@]}" >/dev/null; then
  if [[ -n "$release_repository" ]]; then
    write_release_upload_report false release_view "release $tag was not found in repository $release_repository"
    fail "release $tag was not found in repository $release_repository"
  fi
  write_release_upload_report false release_view "release $tag was not found"
  fail "release $tag was not found"
fi

if ! gh "${gh_args[@]}"; then
  if [[ -n "$release_repository" ]]; then
    write_release_upload_report false upload "gh release upload failed for $tag in repository $release_repository"
    fail "gh release upload failed for $tag in repository $release_repository"
  fi
  write_release_upload_report false upload "gh release upload failed for $tag"
  fail "gh release upload failed for $tag"
fi

write_release_upload_report true "" ""
