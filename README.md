# ContinuityDB

ContinuityDB is a Rust-native embeddable datastore for agent world models: context, beliefs, knowledge, evidence, uncertainty, and operational truth.

The primitive unit is the `StateCell`, an append-only, evidence-backed, temporally-aware unit of operational truth. ContinuityDB owns StateCell semantics, belief revision, supersession, deterministic checkout, and auditability. Storage engines are substrates behind a kernel interface, not the identity of the database.

## Current Scope

The first milestone builds:

- Core StateCell domain types.
- Deterministic StateCell workload generation for benchmark and storage-engine comparisons.
- Storage-kernel-generic deterministic workload measurement for ingest and checkout.
- CLI workload measurement for memory and file-backed kernels.
- JSONL workload measurement baseline recording.
- CLI workload measurement baseline recording.
- Deterministic workload baseline regression comparison.
- CLI workload baseline regression comparison and failure gating.
- A pluggable storage kernel trait.
- An in-memory kernel for correctness tests.
- Basic revision links.
- Core native revision-link records.
- Storage-kernel-native revision-link records.
- Deterministic checkout.
- Semantic-anchor scoped checkout and strict text query constraints.
- Activation-aware checkout and strict text query constraints.
- Typed Continuity Query AST compiling checkout semantics into native requests.
- First strict text parser for `CHECKOUT` queries.
- Bitemporal and commit-scoped constraints in strict text `CHECKOUT` queries.
- Dependency-aware constraints in strict text `CHECKOUT` queries.
- Portable typed query serialization for bindings and future query files.
- Versioned JSON envelopes for portable typed query files.
- Read-only typed query AST introspection for embedders and bindings.
- Native typed query execution through the embeddable API.
- Native API execution for versioned typed query envelopes.
- Native API execution for saved typed query files.
- Native and CLI execution for saved text `CHECKOUT` query files.
- Native API execution for strict text `CHECKOUT` query strings.
- CLI execution for serialized typed query files.
- CLI execution for versioned typed query envelopes.
- Commit-scoped checkout.
- Audit traces.
- Commit-aware audit traces.
- Deterministic Steward proposal substrate.
- Pluggable proposal audit ledger store contract.
- JSONL file-backed proposal audit store.
- StorageKernel-backed proposal audit store adapter.
- Native feature-gated Steward proposal audit API.
- Native feature-gated Steward conflict-resolution audit API.
- Native feature-gated Steward frontier/watch audit API.
- Native feature-gated Steward `MarkFrontier` application API.
- Native feature-gated Steward `LabelAnswerability` application API.
- Native feature-gated Steward `AdjustConfidence` application API.
- Native feature-gated Steward `RequestVerification` application API.
- Native feature-gated Steward `LinkRevision` application API.
- Native feature-gated Steward `CreateCellDraft` application API.
- Native feature-gated Steward accepted-proposal dispatch API.
- Deterministic mock Steward for test-first development.
- Feature-gated local model Steward boundary.
- Local executable Steward model runner.
- Stable local model Steward response schema and grammar contract.
- Deterministic llama.cpp and mistral.rs runtime profiles.
- Fixed Steward proposal-quality evaluation harness.
- Executable local model benchmark fixture.
- Durable local model benchmark baseline store.
- Deterministic frontier/watch Steward proposal integration.
- Durable frontier/watch subscription stores.
- Deterministic frontier subscription runner.
- Atomic kernel-level StateCell write batches.
- First-class commit identifiers for transaction-scoped lookup.
- First-class commit manifests for transaction-boundary inspection.
- Commit manifest timeline listing.
- Cursor-based commit manifest listing for incremental audit and sync reads.
- Explicit durable commit records in the JSONL file kernel.
- Versioned JSONL file-kernel format headers.
- Per-record JSONL file-kernel checksums for cell and commit records.
- File-kernel secondary indexes for answerability questions and evidence sources.
- File-kernel secondary indexes for activation states and dependency filters.
- Typed storage-kernel capability introspection for embedders.
- Typed storage-kernel requirement checks for production readiness gates.
- File-backed store status for visible cell/commit counts and durable file size.
- File-backed store health reporting for canonical and compaction-worthy logs.
- Canonical file-store requirement gate for production open/inspection paths.
- Line-addressed JSONL file-kernel corruption diagnostics.
- Headered JSONL file-kernel partial-commit detection.
- Durable filesystem flush boundaries for JSONL file-kernel writes.
- JSONL file-kernel compaction into the canonical durable record format.
- Conditional file-store compaction for explicit maintenance automation.
- Native file-backed compaction API.
- Native file-backed open helpers with requirement enforcement.
- CLI file-backed compaction command.
- Native commit cell materialization API.
- Native commit slice materialization API for cursor-selected commit replay.
- Native commit export batch API for backup and sync flows.
- Native validated commit import batch API for replay flows.
- Native direct commit copy between open databases for local sync.
- Commit import dry-run validation for backup and sync workflows.
- Commit import summaries with cursor metadata for checkpointed sync.
- Versioned JSON commit export envelope for backup and sync files.
- Native commit backup and restore file helper API.
- Native typed operation API for ingest, checkout, and audit.
- Native typed utility feedback revision API.
- Native read-only conflict analysis API.
- Native read-only batch conflict analysis API.
- CLI kernel capability inspection and requirement checks.
- CLI file-backed commands routed through native open helpers.
- CLI commit backup and restore commands over versioned export envelopes.
- CLI incremental commit export for cursor-based backup and sync workflows.
- CLI direct commit copy for local file-backed sync.
- A thin CLI over library APIs.

## Roadmap

The frontier roadmap is tracked in [`docs/roadmap.md`](docs/roadmap.md). The next major research item is an embedded database Steward model: a local, proposal-only model layer that helps maintain StateCells, conflicts, answerability, frontier priorities, and audit explanations without directly mutating committed truth.

## Engineering Standard

Development is test-first. No demo-only behavior, unchecked library panics, or shortcuts are accepted.
