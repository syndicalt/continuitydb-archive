# CLI Workload Artifact Replay Plan

**Goal:** Let archived workload bundles be re-executed against a selected kernel from their saved fixtures.

**Architecture:** Add a `replay-workload` command, decode and validate `workload-cells.json` and `checkout-request.json`, replay the cells into the selected kernel, execute the archived checkout request, and emit comparable workload and checkout counts plus fixture fingerprints.

**Tech Stack:** Rust, clap, serde_json, existing workload CLI tests.

- [x] Add failing CLI coverage for replaying a workload artifact bundle.
- [x] Add `replay-workload` CLI command.
- [x] Decode versioned workload cells and checkout request artifacts.
- [x] Replay archived cells and request against memory and file kernel paths.
- [x] Emit replay counts and artifact fingerprints.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
