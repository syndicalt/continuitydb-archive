#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PUBLISH_ORDER=(
  continuitydb-core
  continuitydb-kernel
  continuitydb-memory
  continuitydb-checkout
  continuitydb-query
  continuitydb-revision
  continuitydb-steward
  continuitydb-api
  continuitydb-workload
  continuitydb-cli
)

fail() {
  printf 'release preflight failed: %s\n' "$1" >&2
  exit 1
}

json_string() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  printf '"%s"' "$value"
}

sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}

package_version() {
  sed -n 's/^version = "\([^"]*\)"/\1/p' "$1" | head -n 1
}

internal_path_dependencies() {
  grep -Eoh 'continuitydb-[[:alnum:]-]+[[:space:]]*=[[:space:]]*\{[^}]*path[[:space:]]*=[^}]*\}' "$1" || true
}

version="$(package_version "crates/continuitydb-core/Cargo.toml")"
[[ -n "$version" ]] || fail "could not derive package version from continuitydb-core"

required_workspace_metadata=(
  'license.workspace = true'
  'repository.workspace = true'
  'rust-version.workspace = true'
  'description.workspace = true'
  'readme.workspace = true'
  'keywords.workspace = true'
  'categories.workspace = true'
)

for field in \
  '^license = ' \
  '^repository = ' \
  '^rust-version = ' \
  '^description = ' \
  '^readme = ' \
  '^keywords = ' \
  '^categories = '
do
  grep -q "$field" Cargo.toml || fail "workspace package metadata is missing ${field#^}"
done

declare -A publish_index=()
for index in "${!PUBLISH_ORDER[@]}"; do
  package="${PUBLISH_ORDER[$index]}"
  manifest="crates/$package/Cargo.toml"
  [[ -f "$manifest" ]] || fail "missing manifest for $package at $manifest"
  publish_index["$package"]="$index"
done

for package in "${PUBLISH_ORDER[@]}"; do
  manifest="crates/$package/Cargo.toml"
  actual_version="$(package_version "$manifest")"
  [[ "$actual_version" == "$version" ]] || fail "$package version $actual_version does not match $version"

  for metadata in "${required_workspace_metadata[@]}"; do
    grep -Fq "$metadata" "$manifest" || fail "$package does not inherit $metadata"
  done

  while IFS= read -r dependency; do
    dep_name="${dependency%%=*}"
    dep_name="${dep_name//[[:space:]]/}"
    [[ -n "${publish_index[$dep_name]+x}" ]] || fail "$package depends on unpublished workspace crate $dep_name"
    [[ "$dependency" == *"version = \"$version\""* ]] || fail "$package dependency $dep_name is missing version = \"$version\""

    dep_index="${publish_index[$dep_name]}"
    package_index="${publish_index[$package]}"
    if (( dep_index >= package_index )); then
      fail "$package appears before its internal dependency $dep_name in publish order"
    fi
  done < <(internal_path_dependencies "$manifest")
done

package_args=(--workspace --all-features --no-verify)
for package in "${PUBLISH_ORDER[@]}"; do
  package_args+=(--config "patch.crates-io.$package.path=\"crates/$package\"")
done
if [[ "${ALLOW_DIRTY:-0}" == "1" ]]; then
  package_args+=(--allow-dirty)
fi

rm -rf target/package
cargo package "${package_args[@]}"

for package in "${PUBLISH_ORDER[@]}"; do
  crate_archive="target/package/$package-$version.crate"
  [[ -f "$crate_archive" ]] || fail "missing package archive $crate_archive"
  tar -xzf "$crate_archive" -C target/package
done

verify_manifest="target/package/Cargo.toml"
{
  printf '[workspace]\n'
  printf 'members = [\n'
  for package in "${PUBLISH_ORDER[@]}"; do
    printf '  "%s-%s",\n' "$package" "$version"
  done
  printf ']\n'
  printf 'resolver = "2"\n\n'
  printf '[patch.crates-io]\n'
  for package in "${PUBLISH_ORDER[@]}"; do
    printf '%s = { path = "%s-%s" }\n' "$package" "$package" "$version"
  done
} >"$verify_manifest"

cargo check --manifest-path "$verify_manifest" --workspace --all-features

proof_dir="target/release-preflight"
proof_report="$proof_dir/proof-obligations.json"
proof_validation_report="$proof_dir/proof-obligations-validation.json"
proof_validation_failure_report="$proof_dir/proof-obligations-validation-failure.json"
release_asset_manifest="$proof_dir/release-assets.json"
release_asset_validation_report="$proof_dir/release-assets-validation.json"
release_asset_validation_failure_report="$proof_dir/release-assets-validation-failure.json"

rm -rf "$proof_dir"
mkdir -p "$proof_dir"

cargo run -q -p continuitydb-cli -- proof-obligations >"$proof_report"
cargo run -q -p continuitydb-cli -- validate-proof-obligations \
  --report-path "$proof_report" \
  --validation-report-path "$proof_validation_report" \
  --failure-report-path "$proof_validation_failure_report" >/dev/null

{
  printf '{\n'
  printf '  "format": "continuitydb.release_assets",\n'
  printf '  "format_version": 1,\n'
  printf '  "generated_by": "scripts/release_preflight.sh",\n'
  printf '  "package_version": '
  json_string "$version"
  printf ',\n'
  printf '  "asset_count": %s,\n' "$((${#PUBLISH_ORDER[@]} + 2))"
  printf '  "assets": [\n'
  for index in "${!PUBLISH_ORDER[@]}"; do
    package="${PUBLISH_ORDER[$index]}"
    crate_archive="target/package/$package-$version.crate"
    printf '    {\n'
    printf '      "kind": "crate_archive",\n'
    printf '      "package": '
    json_string "$package"
    printf ',\n'
    printf '      "name": '
    json_string "$package-$version.crate"
    printf ',\n'
    printf '      "path": '
    json_string "$crate_archive"
    printf ',\n'
    printf '      "bytes": %s,\n' "$(wc -c <"$crate_archive" | tr -d ' ')"
    printf '      "sha256": '
    json_string "$(sha256_file "$crate_archive")"
    printf '\n'
    printf '    },\n'
  done
  for asset_path in "$proof_report" "$proof_validation_report"; do
    asset_name="$(basename "$asset_path")"
    printf '    {\n'
    printf '      "kind": "release_preflight_evidence",\n'
    printf '      "name": '
    json_string "$asset_name"
    printf ',\n'
    printf '      "path": '
    json_string "$asset_path"
    printf ',\n'
    printf '      "bytes": %s,\n' "$(wc -c <"$asset_path" | tr -d ' ')"
    printf '      "sha256": '
    json_string "$(sha256_file "$asset_path")"
    printf '\n'
    if [[ "$asset_path" == "$proof_validation_report" ]]; then
      printf '    }\n'
    else
      printf '    },\n'
    fi
  done
  printf '  ]\n'
  printf '}\n'
} >"$release_asset_manifest"

cargo run -q -p continuitydb-cli -- validate-release-assets \
  --manifest-path "$release_asset_manifest" \
  --validation-report-path "$release_asset_validation_report" \
  --failure-report-path "$release_asset_validation_failure_report" >/dev/null

printf 'ContinuityDB release preflight passed for version %s\n' "$version"
printf 'Publish order:\n'
for package in "${PUBLISH_ORDER[@]}"; do
  printf '  %s\n' "$package"
done
printf 'Proof-obligation validation report: %s\n' "$proof_validation_report"
printf 'Release asset manifest: %s\n' "$release_asset_manifest"
printf 'Release asset validation report: %s\n' "$release_asset_validation_report"
