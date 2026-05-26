#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKDIR="${1:-$(mktemp -d "${TMPDIR:-/tmp}/continuitydb-alpha.XXXXXX")}"

mkdir -p "$WORKDIR"

STORE="$WORKDIR/store.jsonl"
WORKLOAD_DIR="$WORKDIR/workload"
QUERY_DIR="$WORKDIR/query"
REPLAY_STORE="$WORKDIR/replay-store.jsonl"
REPLAY_DIR="$WORKDIR/replay"
INSPECT_DIR="$WORKDIR/inspect"
PROOF="$WORKDIR/proof-obligations.json"
PROOF_VALIDATION="$WORKDIR/proof-obligations-validation.json"
CONTEXT_COLLAPSE="$WORKDIR/context-collapse-drill.json"
WORKLOAD_VALIDATION="$WORKLOAD_DIR/workload-validation.json"

rm -f "$STORE" "$REPLAY_STORE" "$PROOF" "$PROOF_VALIDATION" "$CONTEXT_COLLAPSE"
rm -rf "$WORKLOAD_DIR" "$QUERY_DIR" "$REPLAY_DIR" "$INSPECT_DIR"

cd "$ROOT"

cargo run -q -p continuitydb-cli -- proof-obligations >"$PROOF"
grep -q '"format": "continuitydb.thesis_proof_obligations"' "$PROOF"
grep -q '"obligation_count": 8' "$PROOF"
grep -q '"deterministic_continuity_checkout"' "$PROOF"
grep -q '"context_collapse_prevention_drill"' "$PROOF"
cargo run -q -p continuitydb-cli -- validate-proof-obligations \
  --report-path "$PROOF" \
  --validation-report-path "$PROOF_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.thesis_proof_obligations_validation"' "$PROOF_VALIDATION"
grep -q '"valid": true' "$PROOF_VALIDATION"

cargo run -q -p continuitydb-cli -- context-collapse-drill \
  --report-path "$CONTEXT_COLLAPSE" >/dev/null
grep -q '"format": "continuitydb.context_collapse_drill"' "$CONTEXT_COLLAPSE"
grep -q '"valid": true' "$CONTEXT_COLLAPSE"
grep -q '"revision_links_preserved": true' "$CONTEXT_COLLAPSE"
grep -q '"revision_context_preserved": true' "$CONTEXT_COLLAPSE"
grep -q '"frontier_preserved": true' "$CONTEXT_COLLAPSE"

cargo run -q -p continuitydb-cli -- measure-workload \
  --kernel file \
  --store-path "$STORE" \
  --cells 16 \
  --token-budget 600 \
  --artifact-dir "$WORKLOAD_DIR"

cargo run -q -p continuitydb-cli -- validate-workload-bundle \
  --artifact-dir "$WORKLOAD_DIR" \
  --report-path "$WORKLOAD_VALIDATION" >/dev/null
grep -q '"format": "continuitydb.workload.bundle_validation"' "$WORKLOAD_VALIDATION"
grep -q '"valid": true' "$WORKLOAD_VALIDATION"
grep -q '"cells_parseable": true' "$WORKLOAD_VALIDATION"
grep -q '"checkout_request_parseable": true' "$WORKLOAD_VALIDATION"

mkdir -p "$QUERY_DIR"
cat >"$QUERY_DIR/checkout-summary.query" <<'QUERY'
CHECKOUT "alpha-summary" ANSWER "what is the operational state for bench:world cell 0?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.5
  AND token_budget <= 600
RETURN summary_only
QUERY

cargo run -q -p continuitydb-cli -- checkout-query \
  "$STORE" \
  "$QUERY_DIR/checkout-summary.query" >"$QUERY_DIR/checkout-summary.json"
grep -q '"format": "continuitydb.checkout_query.summary"' "$QUERY_DIR/checkout-summary.json"
grep -q '"selected_cell_count": 1' "$QUERY_DIR/checkout-summary.json"
grep -q '"selected_cell_count"' "$QUERY_DIR/checkout-summary.json"
grep -q '"invalidation_condition_count"' "$QUERY_DIR/checkout-summary.json"
grep -q '"bounded_by_token_budget"' "$QUERY_DIR/checkout-summary.json"

cargo run -q -p continuitydb-cli -- validate-checkout-query-summary \
  --report-path "$QUERY_DIR/checkout-summary.json" \
  --validation-report-path "$QUERY_DIR/checkout-summary-validation.json" >/dev/null
grep -q '"format": "continuitydb.checkout_query.summary_validation"' "$QUERY_DIR/checkout-summary-validation.json"
grep -q '"valid": true' "$QUERY_DIR/checkout-summary-validation.json"
grep -q '"selected_cell_count": 1' "$QUERY_DIR/checkout-summary-validation.json"
grep -q '"invalidation_condition_count"' "$QUERY_DIR/checkout-summary-validation.json"
grep -q '"bounded_by_token_budget": true' "$QUERY_DIR/checkout-summary-validation.json"

cargo run -q -p continuitydb-cli -- checkout-query \
  --result-envelope \
  "$STORE" \
  "$QUERY_DIR/checkout-summary.query" >"$QUERY_DIR/checkout-result.json"
grep -q '"format": "continuitydb.checkout_query.result"' "$QUERY_DIR/checkout-result.json"
grep -q '"return_shape": "summary_only"' "$QUERY_DIR/checkout-result.json"
grep -q '"type": "summary"' "$QUERY_DIR/checkout-result.json"
grep -q '"selected_cell_count": 1' "$QUERY_DIR/checkout-result.json"

cargo run -q -p continuitydb-cli -- validate-checkout-query-result \
  --report-path "$QUERY_DIR/checkout-result.json" \
  --validation-report-path "$QUERY_DIR/checkout-result-validation.json" >/dev/null
grep -q '"format": "continuitydb.checkout_query.result_validation"' "$QUERY_DIR/checkout-result-validation.json"
grep -q '"valid": true' "$QUERY_DIR/checkout-result-validation.json"
grep -q '"return_shape": "summary_only"' "$QUERY_DIR/checkout-result-validation.json"
grep -q '"selected_cell_count": 1' "$QUERY_DIR/checkout-result-validation.json"

cat >"$QUERY_DIR/checkout-cells.query" <<'QUERY'
CHECKOUT "alpha-cells" ANSWER "what is the operational state for bench:world cell 0?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.5
  AND token_budget <= 600
RETURN cells_only
QUERY

cargo run -q -p continuitydb-cli -- checkout-query \
  "$STORE" \
  "$QUERY_DIR/checkout-cells.query" >"$QUERY_DIR/checkout-cells.json"
grep -q '"format": "continuitydb.checkout_query.cells"' "$QUERY_DIR/checkout-cells.json"
grep -Fq '"cells": [' "$QUERY_DIR/checkout-cells.json"
grep -q '"payload"' "$QUERY_DIR/checkout-cells.json"

cargo run -q -p continuitydb-cli -- checkout-query \
  --result-envelope \
  "$STORE" \
  "$QUERY_DIR/checkout-cells.query" >"$QUERY_DIR/checkout-cells-result.json"
grep -q '"format": "continuitydb.checkout_query.result"' "$QUERY_DIR/checkout-cells-result.json"
grep -q '"return_shape": "cells_only"' "$QUERY_DIR/checkout-cells-result.json"
grep -q '"type": "cells"' "$QUERY_DIR/checkout-cells-result.json"

cargo run -q -p continuitydb-cli -- validate-checkout-query-result \
  --report-path "$QUERY_DIR/checkout-cells-result.json" \
  --validation-report-path "$QUERY_DIR/checkout-cells-result-validation.json" >/dev/null
grep -q '"format": "continuitydb.checkout_query.result_validation"' "$QUERY_DIR/checkout-cells-result-validation.json"
grep -q '"valid": true' "$QUERY_DIR/checkout-cells-result-validation.json"
grep -q '"return_shape": "cells_only"' "$QUERY_DIR/checkout-cells-result-validation.json"
grep -q '"result_type": "cells"' "$QUERY_DIR/checkout-cells-result-validation.json"
grep -q '"cell_count": 1' "$QUERY_DIR/checkout-cells-result-validation.json"

cat >"$QUERY_DIR/checkout-context-packets.query" <<'QUERY'
CHECKOUT "alpha-context-packets" ANSWER "what is the operational state for bench:world cell 2?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.5
  AND token_budget <= 600
RETURN context_packets_only
QUERY

cargo run -q -p continuitydb-cli -- checkout-query \
  "$STORE" \
  "$QUERY_DIR/checkout-context-packets.query" >"$QUERY_DIR/checkout-context-packets.json"
grep -q '"format": "continuitydb.checkout_query.context_packets"' "$QUERY_DIR/checkout-context-packets.json"
grep -Fq '"context_packets": [' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"source"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"target"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"relation"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"epistemic_action_reasons"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"epistemic_pressure"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"context_gaps"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"invalidation_conditions"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"trajectory_memory"' "$QUERY_DIR/checkout-context-packets.json"
grep -q '"lifecycle_policy"' "$QUERY_DIR/checkout-context-packets.json"

cargo run -q -p continuitydb-cli -- validate-checkout-query-context-packets \
  --report-path "$QUERY_DIR/checkout-context-packets.json" \
  --validation-report-path "$QUERY_DIR/checkout-context-packets-validation.json" >/dev/null
grep -q '"format": "continuitydb.checkout_query.context_packets_validation"' "$QUERY_DIR/checkout-context-packets-validation.json"
grep -q '"valid": true' "$QUERY_DIR/checkout-context-packets-validation.json"
grep -q '"context_packet_count": 1' "$QUERY_DIR/checkout-context-packets-validation.json"

cargo run -q -p continuitydb-cli -- replay-workload \
  --kernel file \
  --store-path "$REPLAY_STORE" \
  --artifact-dir "$WORKLOAD_DIR" \
  --require-manifest \
  --compare-report \
  --fail-on-mismatch \
  --replay-artifact-dir "$REPLAY_DIR"

cargo run -q -p continuitydb-cli -- inspect-kernel \
  "$STORE" \
  --require persistent-indexed-append-log \
  --lookup-query 'CHECKOUT "inspect" ANSWER "what facts are in scope?" WHERE scope = project("continuitydb")' \
  --artifact-dir "$INSPECT_DIR"

cargo run -q -p continuitydb-cli -- validate-inspect-kernel-bundle \
  --artifact-dir "$INSPECT_DIR"

printf 'ContinuityDB alpha workflow artifacts: %s\n' "$WORKDIR"
