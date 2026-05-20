# ContinuityDB Roadmap

This roadmap tracks frontier items that extend ContinuityDB beyond the initial deterministic Rust foundation.

## Roadmap Principles

- Preserve ContinuityDB as a datastore, not an agent runtime.
- Keep deterministic database semantics at the commit boundary.
- Treat model outputs as proposals, not hidden truth mutations.
- Require every accepted model-assisted change to be auditable as evidence.
- Prefer small, embeddable, open-source models before hosted model dependencies.

## Frontier Item: Embedded Database Steward

ContinuityDB should eventually include an embedded agentic model layer that acts as a steward for the database itself. This is not a general agent orchestrator and must not run application workflows. Its job is to maintain the epistemic health of the datastore.

The Steward may propose:

- StateCell creation from new evidence.
- Supersession and conflict links.
- Confidence adjustments.
- Answerability labels.
- Frontier priority changes.
- Checkout summaries and uncertainty explanations.
- Verification or refresh tasks for stale or high-impact cells.

The Steward must not directly mutate committed truth. It emits structured proposals. A deterministic policy layer validates, rejects, or accepts those proposals. Accepted proposals become append-only StateCells or revision links with provenance showing the source evidence, model identity, prompt/profile, policy decision, and resulting database mutation.

Boundary rule:

> ContinuityDB may use an embedded model to propose what state needs attention; deterministic database policy decides what becomes committed truth.

## Steward Architecture Track

```text
External apps and agents
  ingest evidence, request checkout, provide feedback

ContinuityDB API
  deterministic request boundary

Steward model layer
  proposes revisions, conflicts, answerability, summaries, frontier work

Policy and validation layer
  validates proposal schema, confidence, permissions, and audit requirements

Semantic engine
  StateCells, evidence, bitemporality, revision, checkout, audit

Storage kernel
  append log, graph, temporal, text, vector, payload storage
```

## Storage Kernel Milestones

1. Define the minimal `StorageKernel` append and lookup contract. Implemented in `continuitydb-kernel`.
2. Provide an in-memory correctness kernel for deterministic tests. Implemented in `continuitydb-memory`.
3. Add the first durable embedded kernel. Implemented as an append-only JSONL `FileKernel` in `continuitydb-kernel` with nested database directory creation; indexed production storage remains future work.
4. Add first-class StateCell activation filtering. Implemented in `CellLookup` across memory and file kernels.
5. Add first-class StateCell answerability filtering. Implemented in `CellLookup` across memory and file kernels.
6. Add first-class evidence-source and minimum-confidence filtering. Implemented in `CellLookup` across memory and file kernels.
7. Add deterministic in-process indexes for the durable file kernel. Implemented ID and semantic-anchor indexes rebuilt from the JSONL log on open and maintained after successful append, giving the first durable kernel a real indexing boundary while preserving the append log as source of truth.
8. Add first-class system transaction-time stamping and lookup. Implemented `StateCell.system_time`, deterministic `StorageKernel::append_cell_at`, default append-time stamping, and `CellLookup.system_at` filtering across memory and file kernels.
9. Add atomic StateCell write batches. Implemented `StorageKernel::append_cells` and `append_cells_at` across memory and file kernels so related cells can commit with one shared system transaction time and duplicate batches reject without partial visibility.
10. Add first-class commit identifiers. Implemented `CommitId`, `StateCell.commit_id`, explicit commit-stamped append APIs, and `CellLookup.commit_id` filtering across memory and file kernels so transaction-scoped continuity slices have stable audit IDs.
11. Add first-class commit manifests. Implemented `CommitManifest` and kernel/API manifest lookup so a commit boundary can expose its commit time and ordered StateCell IDs without reconstructing from checkout results.
12. Add commit manifest timeline listing. Implemented ordered `list_commit_manifests` support across the storage kernel, memory/file kernels, and native API so audit and sync callers can discover commit boundaries deterministically.
13. Add cursor-based commit manifest listing. Implemented `CommitManifestLookup` with exclusive commit cursors and limits across memory/file kernels and the native API for incremental audit, backup, and future sync reads.
14. Add explicit durable commit records to the JSONL file kernel. Implemented backward-compatible `cell` and `commit` file records so new writes persist commit manifests directly while legacy raw StateCell logs remain readable.
15. Add versioned JSONL file-kernel format headers. Implemented a supported `header` record for new stores while preserving legacy non-header logs and rejecting unsupported or misplaced headers.
16. Add per-record JSONL file-kernel checksums. Implemented deterministic checksum fields for new cell and commit records, validation on reopen, and compatibility with checksum-free legacy envelope records.
17. Add JSONL file-kernel compaction. Implemented `FileKernel::compact` to rewrite stored cells and commit manifests into the latest header plus checksummed cell and commit record format while preserving lookup and manifest listing behavior.
18. Add line-addressed JSONL file-kernel corruption diagnostics. Implemented `KernelError::StoreCorruptRecord { line }` for decode-time header, checksum, and malformed JSONL failures so operators can locate damaged durable records.
19. Add headered JSONL file-kernel partial-commit detection. Implemented explicit-manifest enforcement for current-format headered logs so crash-truncated cell records are rejected instead of reconstructed as committed truth, while headerless legacy raw logs remain readable.
20. Add durable filesystem flush boundaries for JSONL file-kernel writes. Implemented internal durable write helpers using flush plus `sync_all` for headers, append batches, and compaction temp files, with parent-directory sync after compaction rename.
21. Add file-kernel secondary indexes for answerability and evidence source lookups. Implemented derived in-process indexes rebuilt from the JSONL log and maintained after append so common context retrieval filters can start from indexed candidates while preserving append-order results and the canonical log as source of truth.
22. Add file-kernel secondary indexes for activation and dependency lookups. Implemented derived in-process indexes for activation states, dependency targets, and dependency target/kind pairs so frontier and causality filters can start from indexed candidates while preserving append-order results.
23. Add typed storage-kernel capability introspection. Implemented `KernelDurability`, `KernelCapabilities`, `StorageKernel::capabilities`, and native API exposure so embedders can distinguish ephemeral, append-log, and future indexed embedded kernels without depending on concrete kernel types.
24. Add typed storage-kernel requirement matching. Implemented `KernelRequirements` and `KernelCapabilities::satisfies` so embedders can express ephemeral, durable append-log, and future indexed embedded storage requirements without duplicating capability comparison logic.
25. Add file-backed store status. Implemented `FileKernelStatus` and `FileKernel::status` so operators and embedders can inspect visible cell counts, commit counts, revision-link counts, and durable file size after open-time validation.
26. Add file-backed store health reporting. Implemented `FileKernelHealth` and `FileKernel::health` so validated readable stores report whether they are canonical, legacy raw, checksum-free, or compaction-worthy.
27. Add native revision-link record storage. Implemented `RevisionLinkLookup`, append/list storage-kernel methods, in-memory revision-link storage, and checksummed JSONL file-kernel records that survive reopen and compaction.
28. Add file-kernel secondary indexes for revision-link lookups. Implemented derived in-process indexes for revision-link source, target, kind, source-kind, target-kind, source-target, and source-target-kind filters while preserving append-order results and the canonical log as source of truth.
29. Add revision-link-aware file-store status. Extended `FileKernelStatus` and inspection surfaces with visible revision-link counts so native revision graph records are operationally visible alongside cells and commits.
30. Add duplicate revision-link append rejection. Implemented exact duplicate `RevisionLinkRecord` rejection across memory and file kernels, including pre-write file append checks and duplicate durable-log detection during file-kernel reopen.

## Checkout Milestones

1. Push deterministic checkout constraints into storage lookup. Implemented for scope, valid time, answerability question, evidence source, and minimum confidence in `continuitydb-checkout`.
2. Add selected-cell metadata to checkout slices. Implemented audit traces, uncertainty entries, and frontier recommendations in `continuitydb-checkout`.
3. Add deterministic checkout alternatives. Implemented token-budget omission metadata with reason, citations, and confidence in `continuitydb-checkout`.
4. Rank checkout candidates by deterministic utility-aware score. Implemented by combining max evidence confidence with `StateCell` utility feedback before token-budget packing in `continuitydb-checkout`.
5. Add dependency-aware checkout constraints. Implemented dependency target and kind filters in `CheckoutRequest` with pushdown into `CellLookup`.
6. Add activation-aware checkout constraints. Implemented `CheckoutRequest.activation` with pushdown into `CellLookup.activation` so callers can materialize dormant, active, frontier, or retired StateCells through deterministic checkout.
7. Add semantic-anchor checkout constraints. Implemented `CheckoutRequest.semantic_anchor` with pushdown into `CellLookup.semantic_anchor` so callers can materialize StateCells by stable semantic identity through deterministic checkout.
8. Add system-time checkout constraints. Implemented `CheckoutRequest.system_at` with pushdown into `CellLookup.system_at`, allowing continuity slices to be materialized as of a database transaction time.
9. Add dependency-aware audit traces. Implemented `AuditDependency` metadata in checkout audit traces so selected cells explain dependency and causality links alongside citations.
10. Add structured evidence audit traces. Implemented `AuditEvidence` metadata in checkout audit traces so selected cells expose source IDs, citation locators, confidence scores, and trust signals.
11. Add commit-scoped checkout constraints. Implemented `CheckoutRequest.commit_id` with pushdown into `CellLookup.commit_id`, allowing continuity slices to be materialized for one explicit database commit boundary.
12. Add commit-aware audit traces. Implemented `AuditTrace.commit_id` so direct audit and checkout-selected audit traces expose the database commit boundary that wrote each cell.
13. Add revision-link-aware audit traces. Implemented `AuditTrace.revision_links` and direct native API audit enrichment so callers can see native supersession, predecessor, conflict, and derivation links where an audited StateCell participates.
14. Add checkout-selected revision-link audit traces. Implemented kernel-backed enrichment for checkout-selected audit traces so materialized continuity slices carry native revision-link context without changing selection, ranking, alternatives, uncertainty, or frontier metadata.

## Query Language Milestones

1. Add a typed Continuity Query AST. Implemented `continuitydb-query` with structured checkout query types and compilation into `CheckoutRequest`, establishing the semantic target for future text syntax and API bindings.
2. Execute typed queries through the native API. Implemented `ContinuityDb::checkout_query` and `checkout_continuity_query` so embedders can materialize typed Continuity queries without manually compiling them into checkout requests.
3. Add portable typed query serialization. Implemented serde support for `continuitydb-query` AST values with stable snake-case enum tags so future CLI query files, bindings, and agent APIs can exchange typed queries without a text parser.
4. Execute serialized typed query files from the CLI. Implemented `continuitydb checkout-query <store-path> <query-path>` so saved `ContinuityQuery` JSON can materialize file-backed checkout slices through the native typed API before text query syntax exists.
5. Add a versioned typed query JSON envelope. Implemented `QueryEnvelope`, `encode_query_json`, and `decode_query_json` in `continuitydb-query` so saved query files and bindings can validate format and version before executing raw typed query content.
6. Add read-only typed query AST introspection. Implemented `CheckoutQuery` accessors for task, requirements, return shape, and optimization so embedders and bindings can inspect decoded query files without exposing internal fields.
7. Add first strict text parser for checkout queries. Implemented `parse_query_text` for a minimal `CHECKOUT "task" ANSWER "question"` syntax with deterministic `WHERE` constraints for scope, minimum confidence, token budget, and evidence source.
8. Add temporal and commit constraints to text checkout queries. Implemented strict `valid_at`, `system_at`, and `commit_id` constraints so text `CHECKOUT` syntax can express bitemporal and commit-scoped materialization already available in the typed AST.
9. Add dependency constraints to text checkout queries. Implemented `dependency_target` and `dependency_kind` constraints, backed by `StateCellId` text parsing, so strict text `CHECKOUT` can express causality-aware materialization already available in the typed AST.
10. Add activation constraints to typed and text checkout queries. Implemented `QueryRequirements.activation` and strict text `activation = ...` parsing so query files can materialize activation-scoped continuity slices.
11. Add semantic-anchor constraints to typed and text checkout queries. Implemented `QueryRequirements.semantic_anchor` and strict text `semantic_anchor = "..."` parsing so query files can materialize anchor-scoped continuity slices.

## Utility Feedback Milestones

1. Add first-class StateCell utility feedback primitives. Implemented as bounded relevance, recency, and decision-impact scores in `continuitydb-core`, with neutral defaults for new and previously serialized cells.
2. Add append-only utility feedback revision. Implemented in `continuitydb-revision` as a deterministic helper that creates a successor `StateCell` version and records supersession/predecessor links to the prior version.

## Dependency and Causality Milestones

1. Add first-class StateCell dependency references. Implemented in `continuitydb-core` as typed links to target `StateCellId` values with dependency kind and rationale, defaulting to an empty list for new and previously serialized cells.
2. Add dependency-aware storage lookup. Implemented target and kind filters in `CellLookup` across memory and file kernels.
3. Add core native revision-link records. Implemented `RevisionLinkKind` and `RevisionLinkRecord` in `continuitydb-core`, with compatibility re-export from `continuitydb-revision`, establishing the semantic record shape needed for storage-native revision links.

## Conflict Detection Milestones

1. Add deterministic StateCell conflict detection. Implemented same-anchor, overlapping-valid-time, different-payload detection in `continuitydb-revision`, backed by half-open valid-time overlap semantics in `continuitydb-core`.
2. Add deterministic candidate-set conflict scanning. Implemented unordered pairwise scanning in input order with aggregate reciprocal `ConflictsWith` revision links in `continuitydb-revision`.
3. Add deterministic conflict resolution recommendations. Implemented non-mutating recommendations for confidence-gap supersession, latest-valid-time wins, and human review in `continuitydb-revision`.
4. Add deterministic batch conflict resolution recommendations. Implemented candidate-set recommendation scanning with aggregate conflict links and proposed supersession links in `continuitydb-revision`.

## CLI Milestones

1. Expose deterministic checkout JSON from the CLI. Implemented as `continuitydb demo-checkout`, showing selected cells, audit traces, uncertainty, frontier recommendations, and alternatives.
2. Expose JSONL file-store compaction from the CLI. Implemented as `continuitydb compact-file <path>` so operators can compact a file-backed store into the current canonical durable record format.
3. Expose commit backup and restore from the CLI. Implemented `continuitydb export-commits <store-path> <output-path>` and `continuitydb import-commits <store-path> <input-path>` over the versioned commit export envelope so file-backed stores can be copied through a validated portable backup file.
4. Expose kernel capability inspection from the CLI. Implemented `continuitydb inspect-kernel <store-path> [--require <profile>]` so operators and CI can inspect file-backed storage guarantees and fail early when a requested profile is not satisfied.
5. Route file-backed CLI commands through native open helpers. Refactored inspection, compaction, export, and import commands to use `ContinuityDb<FileKernel>::open_file` or `open_file_with_requirements`, keeping operational tooling aligned with the embeddable API boundary.
6. Include file-store status in kernel inspection. Extended `continuitydb inspect-kernel` JSON with visible cell count, commit count, revision-link count, and durable file size.
7. Include file-store health in kernel inspection. Extended `continuitydb inspect-kernel` JSON with file-format health and compaction recommendation metadata.
8. Add canonical file-store inspection gate. Implemented `continuitydb inspect-kernel --require-canonical` so CI and operators can fail early when a readable file store needs compaction.
9. Add conditional file-store compaction. Implemented `continuitydb compact-file --if-needed` so operators can compact only when health recommends it.
10. Add commit import dry-run validation. Implemented `continuitydb import-commits --dry-run` so operators can validate backup files against a target store before mutation.
11. Add incremental commit export. Implemented `continuitydb export-commits --after --limit` so operators can page commit backups through the same cursor semantics exposed by the native API.
12. Report commit import cursors. Extended `continuitydb import-commits` output with `next_after` so operators can checkpoint imported backup pages.
13. Add direct commit copy. Implemented `continuitydb copy-commits` so operators can copy cursor-selected commit pages between local file-backed stores without writing an intermediate backup file.
14. Execute saved query files from the CLI. Extended `continuitydb checkout-query` to accept raw typed query JSON, versioned `continuitydb.query` envelopes, and strict text `CHECKOUT` query files through the native query-file API.
15. Preserve native revision links in backup and copy flows. Extended CLI commit export/import and direct copy behavior to round-trip source-owned native revision-link records through the versioned commit export envelope.

## Benchmark and Workload Milestones

1. Add deterministic world-model workload generation. Implemented `continuitydb-workload` with stable StateCell IDs, semantic anchors, scoped evidence, frontier activation cadence, dependency edges, utility signals, and workload summaries so storage engines and checkout algorithms can be compared against repeatable corpora.
2. Add deterministic workload measurement harness. Implemented storage-kernel-generic ingest and checkout measurement over generated workloads, reporting operation counts, selected and alternative checkout counts, frontier counts, token totals, and observational elapsed durations without making benchmark superiority claims.
3. Add CLI workload measurement. Implemented `continuitydb measure-workload` for memory and file-backed kernels so deterministic workload ingest and checkout measurements can be collected as JSON from operator and CI workflows.
4. Add durable workload measurement baselines. Implemented JSONL baseline records for workload measurement snapshots so historical ingest and checkout evidence can be retained before adding regression gates.
5. Add CLI workload baseline recording. Extended `continuitydb measure-workload` with `--baseline-path` and `--label` so operator and CI runs can append durable JSONL baseline records while still printing current measurement JSON.
6. Add deterministic workload baseline regression comparison. Implemented latest matching baseline lookup and count/timing comparison reports so future storage-engine changes can be checked against durable workload evidence before adding CI gates.
7. Add CLI workload baseline regression gate. Extended `continuitydb measure-workload` with baseline comparison, elapsed-growth tolerance, JSON comparison reporting, and optional non-zero exits for deterministic regressions.

## Native API Milestones

1. Add typed embeddable operations for ingest, checkout, and audit. Implemented in `continuitydb-api` as `ContinuityDb<K>` over any `StorageKernel`, backed by first-class `CellLookup.cell_id` support in memory and file kernels.
2. Add native typed query execution. Implemented checkout execution for `continuitydb-query` AST values through `ContinuityDb<K>`, preserving typed query errors and delegating materialization to the existing checkout engine.
3. Add native versioned query-envelope execution. Implemented `ContinuityDb::checkout_query_json` so embedders can execute `continuitydb.query` envelope bytes directly while preserving distinct query compilation and envelope validation errors.
4. Add native saved query-file execution. Implemented `ContinuityDb::checkout_query_file` so embedders can execute either raw `ContinuityQuery` JSON files or versioned `continuitydb.query` envelope files while preserving native query, envelope, JSON, and file I/O error boundaries.
5. Add native saved text query-file execution. Extended `ContinuityDb::checkout_query_file` to recognize strict text `CHECKOUT` query files and route them through `parse_query_text` before normal typed query execution.
6. Add native strict text query execution. Implemented `ContinuityDb::checkout_query_text` so embedders can execute strict `CHECKOUT` text directly without creating saved query files, preserving text parser and query compilation error boundaries.
7. Add typed utility feedback revision operations. Implemented `ContinuityDb::record_utility_feedback` and `record_utility_feedback_at` so applications can record outcome feedback as append-only successor StateCells through the native API.
8. Add typed read-only conflict analysis operations. Implemented `ContinuityDb::detect_conflict` and `recommend_conflict_resolution` so applications can inspect deterministic StateCell conflicts and non-mutating resolution recommendations through the native API.
9. Add typed read-only batch conflict analysis operations. Implemented `ContinuityDb::detect_conflicts` and `recommend_conflict_resolutions` so applications can analyze deterministic conflict frontiers across ordered stored cell sets through the native API.
10. Add ordered commit cell materialization. Implemented `ContinuityDb::commit_cells` so callers can hydrate the StateCells written by one commit in manifest order, with unknown commits reported as `CommitNotFound`.
11. Add cursor-based commit slice materialization. Implemented `CommitSlice` and `ContinuityDb::commit_slices` so callers can materialize cursor-selected commit manifests with their ordered StateCells for audit, backup, sync, and replay flows.
12. Add native file-backed compaction API. Implemented `ContinuityDb<FileKernel>::compact_file_store` so embedders can run JSONL store compaction through the native API without expanding the generic storage-kernel trait.
13. Add native commit export batches. Implemented `CommitExportBatch` and `ContinuityDb::export_commits` so embedders can page commit slices with a deterministic next cursor for backup, sync, and replay flows.
14. Add native validated commit import batches. Implemented `ContinuityDb::import_commit_batch` so exported commit batches can be validated and replayed into another store while preserving commit IDs, commit times, cell IDs, and manifest ordering.
15. Add versioned JSON commit export envelopes. Implemented `CommitExportEnvelope` plus JSON encode/decode helpers so native commit export batches can be written to files or sync channels with explicit format/version validation.
16. Add native commit backup and restore file helpers. Implemented `ContinuityDb<FileKernel>::export_commits_json_file` and `import_commits_json_file` so embedders can write and read versioned commit export envelope files without duplicating CLI file I/O orchestration.
17. Add native kernel requirement enforcement. Implemented `ContinuityDb::kernel_satisfies` and `ensure_kernel_requirements` with a typed `KernelRequirementsNotMet` error so embedders can fail early when a backing kernel lacks required production guarantees.
18. Add file-backed open helpers with requirement enforcement. Implemented `ContinuityDb<FileKernel>::open_file` and `open_file_with_requirements` so embedders can enforce storage profiles before receiving a usable file-backed database handle.
19. Add native file-store status. Implemented `ContinuityDb<FileKernel>::file_store_status` so embedders can inspect file-backed store shape, including native revision-link counts, without depending on kernel internals.
20. Add native file-store health reporting. Implemented `ContinuityDb<FileKernel>::file_store_health` so embedders can inspect file format health without depending on kernel internals.
21. Add canonical file-store requirement gate. Implemented `ensure_file_store_canonical` and `open_canonical_file` so embedders can reject readable but compaction-worthy file stores without automatic mutation.
22. Add conditional file-store compaction. Implemented `compact_file_store_if_needed` with before/after health summaries so embedders can automate explicit maintenance without rewriting canonical stores.
23. Add commit import dry-run validation. Implemented `validate_commit_import` and `validate_commits_json_file` so embedders can validate replay batches and backup files without mutating target stores.
24. Add commit import summaries. Implemented summary-returning import APIs so embedders can retrieve imported counts and backup cursors without decoding envelopes separately.
25. Add native direct commit copy. Implemented `copy_commits_from` so embedders can replay cursor-selected commit pages between open databases without JSON file envelopes.
26. Add native Steward proposal audit operations. Implemented optional `steward` feature methods on `ContinuityDb<K>` for policy-evaluated proposal audit recording, listing, and lookup through the backing `StorageKernel`.
27. Add native Steward conflict-resolution audit operations. Implemented optional `steward` feature method `audit_conflict_resolutions_with_steward` so embedders can analyze stored conflicts, emit deterministic Steward proposals, and record policy-evaluated proposal audits through the backing `StorageKernel`.
28. Add native Steward frontier/watch audit operations. Implemented optional `steward` feature method `audit_frontier_watch_with_steward` so embedders can run subscribed frontier watch events, emit deterministic Steward proposals, and record policy-evaluated proposal audits through the backing `StorageKernel`.
29. Add native accepted Steward `MarkFrontier` application. Implemented optional `steward` feature method `apply_accepted_mark_frontier_proposal_at` so embedders can deterministically apply accepted frontier proposals as append-only StateCell successors while rejected and unsupported proposals do not mutate committed truth.
30. Add native accepted Steward `LabelAnswerability` application. Implemented optional `steward` feature method `apply_accepted_label_answerability_proposal_at` so embedders can deterministically apply accepted answerability-label proposals as append-only StateCell successors while rejected and unsupported proposals do not mutate committed truth.
31. Add native accepted Steward `AdjustConfidence` application. Implemented optional `steward` feature method `apply_accepted_adjust_confidence_proposal_at` so embedders can deterministically apply accepted confidence proposals as append-only StateCell successors while rejected and unsupported proposals do not mutate committed truth.
32. Add native accepted Steward `RequestVerification` application. Implemented optional `steward` feature method `apply_accepted_request_verification_proposal_at` so embedders can deterministically materialize accepted verification work as append-only operational StateCells while preserving the model-as-proposer boundary.
33. Add native accepted Steward `LinkRevision` application. Implemented optional `steward` feature method `apply_accepted_link_revision_proposal_at` so embedders can deterministically materialize accepted revision-link assertions as append-only operational StateCells until the storage kernel grows native revision-link records.
34. Add native accepted Steward `CreateCellDraft` application. Implemented optional `steward` feature method `apply_accepted_create_cell_draft_proposal_at` so embedders can deterministically promote accepted draft content into append-only StateCells with proposal-derived evidence.
35. Add native accepted Steward proposal dispatch. Implemented optional `steward` feature method `apply_accepted_steward_proposal_at` so embedders can apply any current accepted Steward action through one deterministic policy-to-commit boundary.
36. Add native revision-link record operations. Implemented endpoint-validating native API append/list methods for `RevisionLinkRecord` plus a Steward `LinkRevision` application path that writes native revision-link records without adding an operational link StateCell.
37. Add typed accepted Steward proposal dispatch. Implemented optional `steward` feature enum `StewardApplicationResult` and `apply_accepted_steward_proposal_typed_at` so unified application can return either committed StateCell IDs or native revision-link records while preserving the legacy `StateCellId` dispatcher.
38. Add revision-link-aware direct audit. Implemented `ContinuityDb::audit_cell` enrichment over native revision-link records so direct cell audit exposes source-side and target-side revision relationships while preserving missing-cell error behavior.
39. Add revision-link-aware commit export/import/copy. Extended `CommitExportBatch` with source-owned native revision links, validated imported link endpoints against existing or imported StateCells, and restored links after commit cell import.
40. Add duplicate-safe revision-link import validation. Extended commit export batch validation to reject duplicate incoming revision-link records and revision links already visible in the target before any cells or links are imported.
41. Add duplicate-safe native revision-link append validation. Native revision-link record operations now surface `KernelError::DuplicateRevisionLink` when a caller attempts to append the same source, target, kind, and recorded time twice.
42. Add native conflict-resolution Steward audit/application. Implemented `resolve_conflicts_with_steward_at` so embedders can run deterministic conflict stewardship, persist proposal audit records, and apply accepted results through the typed dispatcher in proposal order.
43. Add native frontier/watch Steward audit/application. Implemented `resolve_frontier_watch_with_steward_at` so embedders can run subscribed frontier stewardship, persist proposal audit records, and apply accepted verification or frontier-maintenance results through the typed dispatcher.

## Steward Milestones

1. Define `StewardProposal` types without invoking any model. Implemented in `continuitydb-steward`.
2. Add policy validation for accepting and rejecting proposals. Implemented in `continuitydb-steward`.
3. Persist accepted and rejected proposals for audit. Implemented as an in-memory append-only ledger, a pluggable `ProposalLedgerStore` contract, a JSONL `FileProposalStore`, and a generic `StorageKernel`-backed proposal audit adapter in `continuitydb-steward`; specialized production-engine adapters remain future work.
4. Build a deterministic mock steward for test-first development. Implemented in `continuitydb-steward`.
5. Add local model inference behind a feature flag. Implemented as a `local-model` backend boundary, local executable runner, and deterministic llama.cpp/mistral.rs runner profiles in `continuitydb-steward`; real runtime execution remains future work.
6. Publish the local model response contract. Implemented feature-gated JSON Schema and GBNF grammar accessors plus runtime-profile helpers so embedders can constrain real model output before deterministic proposal decoding and policy validation.
7. Evaluate small open-source steward models against fixed proposal-quality tests. Implemented as a `local-model` evaluation harness with candidate metadata, deterministic pass/fail reasons, an executable runner benchmark fixture, durable JSONL benchmark baseline records, a recorder API for configured real-runtime baseline collection, latest-baseline lookup for candidate regression gates, deterministic baseline regression comparison, and a record-and-compare gate report; environment-specific real model baseline artifacts remain future work.
8. Add frontier/watch integration so the Steward can propose refresh and verification work. Implemented as deterministic frontier watch events that emit `RequestVerification` and `MarkFrontier` proposals, durable frontier subscription records with in-memory and JSONL file-backed stores, and a subscription runner that filters incoming watch events through stored subscriptions in `continuitydb-steward`.
9. Add conflict-resolution proposal integration. Implemented `ConflictResolutionSteward` to convert deterministic revision conflict recommendations into auditable `LinkRevision` or `RequestVerification` proposals without mutating committed truth.
10. Add durable conflict-resolution proposal audit. Implemented `ConflictResolutionSteward::propose_and_record` so deterministic conflict-resolution proposals are policy-evaluated and persisted through any existing `StoredProposalLedger`.
11. Add kernel-backed conflict-resolution audit. Implemented `ConflictResolutionSteward::propose_and_record_to_kernel` so deterministic conflict-resolution proposal audits can be persisted as StateCells through any `StorageKernel`.
12. Add borrowed kernel proposal audit adapter. Implemented `BorrowedKernelProposalStore` so embedders can record Steward proposal audits through an existing `ContinuityDb`-owned kernel without moving ownership.
13. Add native API conflict-resolution stewardship. Implemented `ContinuityDb::audit_conflict_resolutions_with_steward` so stored conflict sets can become audited Steward proposal decisions without leaving the embeddable database API.
14. Add native API frontier/watch stewardship. Implemented `ContinuityDb::audit_frontier_watch_with_steward` so subscribed frontier watch events can become audited Steward proposal decisions without leaving the embeddable database API.
15. Add accepted `MarkFrontier` proposal application. Implemented the first deterministic proposal-to-state mutation path: accepted frontier proposals append successor StateCells with `Frontier` activation through the native API, while models remain proposal-only.
16. Add accepted `LabelAnswerability` proposal application. Implemented deterministic answerability-label proposal application through the native API, preserving the model-as-proposer boundary while allowing accepted labels to become committed append-only StateCell revisions.
17. Add accepted `AdjustConfidence` proposal application. Implemented deterministic confidence proposal application through the native API by appending successor StateCells with revised evidence confidence while preserving the model-as-proposer boundary.
18. Add accepted `RequestVerification` proposal application. Implemented deterministic verification-work materialization through the native API by appending operational StateCells with derived evidence, target dependencies when present, and full proposal audit payloads.
19. Add accepted `LinkRevision` proposal application. Implemented deterministic revision-link assertion materialization through the native API by appending operational StateCells with derived evidence, endpoint dependencies, and full proposal audit payloads.
20. Add accepted `CreateCellDraft` proposal application. Implemented deterministic draft promotion through the native API by appending StateCells with proposed anchors, proposed text payloads, proposal-derived evidence, and deterministic commit time.
21. Add unified accepted proposal application. Implemented deterministic dispatch from a policy-evaluated proposal audit record to the correct action-specific application path so embedders no longer need to duplicate Steward action matching.
22. Add native conflict-resolution application workflow. Implemented a composed native API path that records conflict-resolution proposal audits and applies accepted `LinkRevision` proposals as native revision-link records through the typed dispatcher.
23. Add native frontier/watch application workflow. Implemented a composed native API path that records subscribed frontier/watch proposal audits and applies accepted `RequestVerification` and `MarkFrontier` proposals through the typed dispatcher.
24. Add local model runtime manifests. Implemented `LocalModelRuntimeManifest` so benchmark reports and durable baselines preserve the executable path and deterministic arguments used for a local model evaluation, while legacy baseline JSON remains readable.
25. Add CLI local-model benchmark recording. Implemented a feature-gated `benchmark-local-model` command that runs the fixed Steward evaluation suite against a configured local executable, appends a JSONL baseline, and prints structured candidate, runtime, pass-count, and regression metadata.
26. Add CLI local-model contract export. Implemented a feature-gated `local-model-contract` command that writes the Steward response JSON Schema and GBNF grammar to operator-selected files for grammar-constrained local model runs.
27. Add local-model benchmark contract versioning. Implemented response schema version metadata on benchmark reports, durable baselines, legacy baseline decoding, and CLI benchmark summaries so future contract evolution remains auditable.
28. Add compatible local-model regression gates. Updated record-and-compare baseline gating to compare only previous baselines with matching candidate identity, response schema version, and runtime manifest, preventing invalid regressions across runtime or contract changes.
29. Add public compatible local-model baseline lookup. Implemented a feature-gated API that returns the newest stored baseline matching candidate identity, response schema version, and runtime manifest without recording a new benchmark run.
30. Add local-model evaluation summary metrics. Implemented serializable deterministic evaluation summaries for reports and baselines so embedders can inspect total, passed, failed, and pass-rate metrics without duplicating regression internals.
31. Add CLI local-model evaluation summary output. Routed `benchmark-local-model` JSON through the public evaluation summary API and exposed failed-case and pass-rate metrics alongside existing pass counts.
32. Add CLI local-model per-case evaluation detail output. Embedded serialized Steward evaluation reports in `benchmark-local-model` JSON so operator artifacts carry case names and deterministic failure reasons alongside aggregate metrics.
33. Add default local-model conflict-classification evaluation. Expanded the fixed Steward benchmark suite with a deterministic `ConflictsWith` revision-link case so local model baselines test classification behavior beyond thin-evidence verification.
34. Add local-model evaluation suite introspection. Implemented read-only accessors for fixed evaluation cases, inputs, expectations, required citations, and rationale constraints so embedders can inspect benchmark contracts before running local models.
35. Add CLI local-model evaluation suite export. Implemented a feature-gated `local-model-evaluation-suite` command that prints the fixed benchmark case contracts as JSON using the public evaluation introspection API.
36. Add local-model evaluation suite fingerprints. Persisted deterministic evaluation-suite fingerprints on benchmark reports and baselines, exposed them in CLI benchmark JSON, and required matching fingerprints for compatible baseline regression gates.
37. Add CLI local-model evaluation suite fingerprint output. Included the deterministic suite fingerprint in the feature-gated `local-model-evaluation-suite` JSON so operators can match inspected benchmark contracts to recorded baselines.
38. Add CLI small-model candidate registry output. Exposed the fixed local Steward candidate registry through `local-model-candidates` JSON so operators can inspect supported model IDs and roles before benchmark runs.
39. Add CLI local-model benchmark dry-run preflight output. Added `benchmark-local-model --dry-run` so operators can inspect candidate, runtime, schema, suite fingerprint, and baseline target metadata without executing a model or mutating baseline records.
40. Add CLI local-model contract fingerprints. Added deterministic schema and grammar fingerprints to contract export and benchmark dry-run JSON so operators can connect archived constraint files to preflight artifacts.
41. Add durable local-model contract fingerprints. Persisted schema and grammar fingerprints on benchmark reports and baselines, exposed them in CLI benchmark JSON, and required matching fingerprints for compatible baseline regression gates.
42. Add small-model candidate runtime metadata. Extended the local Steward candidate registry with recommended runtime, artifact format, temperature, grammar requirement, and operational notes, and exposed the metadata through CLI candidate JSON.
43. Add small-model benchmark argument templates. Added candidate-recommended runner configuration helpers and exposed deterministic benchmark argument vectors through CLI candidate JSON.
44. Add CLI local-model benchmark candidate defaults. Added `benchmark-local-model --candidate-defaults` so benchmark runs and dry-runs can materialize candidate-recommended runner arguments before operator-supplied extra arguments.
45. Add CLI local-model benchmark grammar path support. Added `benchmark-local-model --grammar-path` so dry-runs and benchmark runs can pass generated GBNF grammar artifacts as first-class runtime arguments.
46. Add CLI local-model candidate requirement enforcement. Added `benchmark-local-model --enforce-candidate-requirements` so strict dry-runs and benchmark runs reject grammar-required candidates when no grammar artifact path is supplied.
47. Add CLI local-model benchmark contract artifact directory. Added `benchmark-local-model --contract-dir` so dry-runs and benchmark runs can materialize schema and grammar artifacts, report their fingerprints, and use the generated grammar path for strict grammar-required candidates.
48. Add CLI local-model benchmark prompt artifact directory. Added `benchmark-local-model --prompt-dir` so dry-runs and benchmark runs can write deterministic per-case prompt artifacts and report their fingerprints for reproducible local Steward model trials.
49. Add durable local-model prompt fingerprints. Persisted deterministic prompt-rendering fingerprints on benchmark reports and baselines, exposed them in CLI benchmark JSON, and required matching fingerprints for compatible baseline regression gates.
50. Add CLI local-model dry-run baseline compatibility preflight. Added read-only `benchmark-local-model --dry-run --compare-baseline` baseline inspection so operators can see whether the current runtime and contract metadata have a compatible previous baseline before executing a model.
51. Add default local-model unsupported-claim evaluation. Expanded the fixed Steward benchmark suite with a deterministic unsupported-claim boundary case so local model baselines test whether models request verification instead of inventing deployment certainty beyond the cited evidence.
52. Add default local-model citation preservation evaluation. Expanded the fixed Steward benchmark suite with a deterministic multi-source citation preservation case so local model baselines test whether models preserve all required evidence locators for frontier decisions.
53. Add local-model benchmark stability reporting. Added a public repeated-run stability report API for local Steward benchmarks so embedders can detect output drift across low-temperature trials even when each individual run still passes deterministic evaluation.
54. Add CLI local-model benchmark stability reporting. Added `benchmark-local-model --stability-trials` so operators can request repeated-run stability reports from the existing benchmark command, with dry-run preflight metadata and real-run JSON output that does not persist stability as a baseline.
55. Add CLI local-model instability failure gating. Added `benchmark-local-model --fail-on-unstable` so repeated-run stability drift can fail before baseline recording, making low-temperature output instability usable as a CI gate for Steward model candidates.
56. Add default local-model policy-rejection evaluation. Expanded the fixed Steward benchmark suite with a deterministic policy-rejection avoidance case so local model baselines test whether models request clarification instead of emitting policy-invalid answerability labels.
57. Add CLI local-model fixed evaluation failure gating. Added `benchmark-local-model --fail-on-failed-cases` so current fixed-suite failures can exit non-zero before baseline recording, making proposal-quality failures usable as a CI gate separately from baseline regression checks.
58. Add CLI local-model fixed evaluation failure report artifacts. Added `benchmark-local-model --failure-report-path` so fixed evaluation gate failures can write structured benchmark JSON without recording a baseline.
59. Add default local-model supersession classification evaluation. Expanded the fixed Steward benchmark suite with a deterministic `Supersedes` revision-link case so local model baselines test whether models distinguish newer replacement evidence from direct conflicts.
60. Add default local-model confidence-adjustment evaluation. Expanded the fixed Steward benchmark suite with a deterministic `AdjustConfidence` case so local model baselines test belief confidence revision proposals.
61. Add default local-model targeted-verification evaluation. Expanded the fixed Steward benchmark suite with a deterministic targeted `RequestVerification` case so local model baselines test whether refresh requests preserve the specific stale high-impact StateCell identity.
62. Add default local-model create-cell-draft evaluation. Expanded the fixed Steward benchmark suite with a deterministic `CreateCellDraft` case so local model baselines test whether new evidence becomes a draft proposal rather than an un-audited truth mutation.
63. Add CLI local-model benchmark report artifacts. Added `benchmark-local-model --report-path` so successful real runs and dry-run preflights can write the same structured JSON printed to stdout into a durable artifact for CI and reproducible local model trials.
64. Add CLI local-model benchmark response artifacts. Added `benchmark-local-model --response-dir` so real benchmark runs can persist raw per-case model stdout and report captured response artifact metadata for debugging local Steward model failures.
65. Add durable local-model response fingerprints. Persisted per-case raw response fingerprints and byte counts in benchmark reports and durable baselines so real model trials can be audited without storing raw model stdout in baseline records.
66. Add CLI local-model response artifact manifests. Added `local-model-responses.manifest.json` inside `benchmark-local-model --response-dir` output so archived raw response directories remain self-describing without separate stdout capture.
67. Add CLI local-model benchmark artifact bundles. Added `benchmark-local-model --artifact-dir` so real runs and dry-runs can materialize contracts, prompts, responses where applicable, response manifests, and benchmark reports under one archiveable directory while preserving explicit per-artifact path overrides.
68. Add CLI local-model benchmark bundle manifests. Added `local-model-benchmark.manifest.json` inside `benchmark-local-model --artifact-dir` output so archived benchmark bundles expose one versioned entry point with report, contract, prompt, response, and nested response-manifest references.
69. Add CLI local-model fixed-failure artifact bundles. Extended `benchmark-local-model --artifact-dir --fail-on-failed-cases` so failed fixed-suite runs still write contracts, prompts, responses, nested response manifests, `benchmark-report.json`, and the root bundle manifest before exiting non-zero without recording a baseline.

## Small Embeddable Model Track

The smallest feasible first candidate is **Qwen2.5-0.5B-Instruct**. It is an Apache-2.0 instruction-tuned model with about 0.49B parameters, long context, and explicit model-card claims around instruction following and structured JSON generation. Those traits matter more for a database steward than general chat quality because the Steward must emit constrained proposal objects.

Evaluation candidates:

- **Default feasibility candidate:** `Qwen/Qwen2.5-0.5B-Instruct`
  - Why: smallest current candidate that is explicitly instruction-tuned and claims improved structured output behavior.
  - Use for: first real Steward proposal experiments.
- **Current reasoning candidate:** `Qwen/Qwen3-0.6B`
  - Why: slightly larger, newer Qwen line with configurable thinking behavior.
  - Use for: compare proposal quality against Qwen2.5-0.5B.
- **Ultra-small experimental candidate:** `HuggingFaceTB/SmolLM2-360M-Instruct`
  - Why: smaller on-device instruction model.
  - Use for: measure the lower bound; do not assume it is reliable enough for default stewardship.
- **Smoke-test-only candidate:** `HuggingFaceTB/SmolLM2-135M-Instruct`
  - Why: extremely small, useful for CI or toy constrained-output tests if it can follow the schema.
  - Use for: optional experiments, not default stewardship.

Recommended inference path:

- Prefer a local feature-gated backend.
- Evaluate `llama.cpp`/GGUF first for portability and grammar-constrained JSON output.
- Evaluate `mistral.rs` as the Rust-native integration path for GGUF and future in-process inference.
- Keep hosted model APIs outside the core engine.

## Steward Acceptance Tests

Before adding a real model dependency, create fixed test corpora and score proposals for:

- Valid JSON/schema conformance.
- Correct conflict versus supersession classification.
- Evidence citation preservation.
- No unsupported claims beyond source evidence.
- Stable output under low temperature.
- Explicit uncertainty when evidence is insufficient; scoreable through required rationale terms.
- Deterministic policy rejection of invalid proposals.

## Research Sources

- Qwen2.5-0.5B-Instruct model card: <https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct>
- Qwen3-0.6B model card: <https://huggingface.co/Qwen/Qwen3-0.6B>
- SmolLM2-360M-Instruct model files/card: <https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct>
- llama.cpp GGUF and grammar-constrained inference: <https://github.com/ggml-org/llama.cpp>
- mistral.rs Rust inference engine: <https://docs.rs/crate/mistralrs/latest>
