# ContinuityDB

ContinuityDB is a Rust-native embeddable datastore for agent world models: context, beliefs, knowledge, evidence, uncertainty, and operational truth.

The primitive unit is the `StateCell`, an append-only, evidence-backed, temporally-aware unit of operational truth. ContinuityDB owns StateCell semantics, belief revision, supersession, deterministic checkout, and auditability. Storage engines are substrates behind a kernel interface, not the identity of the database.

## Current Scope

The first milestone builds:

- Core StateCell domain types.
- A pluggable storage kernel trait.
- An in-memory kernel for correctness tests.
- Basic revision links.
- Deterministic checkout.
- Commit-scoped checkout.
- Audit traces.
- Commit-aware audit traces.
- Deterministic Steward proposal substrate.
- Pluggable proposal audit ledger store contract.
- JSONL file-backed proposal audit store.
- StorageKernel-backed proposal audit store adapter.
- Deterministic mock Steward for test-first development.
- Feature-gated local model Steward boundary.
- Local executable Steward model runner.
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
- Line-addressed JSONL file-kernel corruption diagnostics.
- Headered JSONL file-kernel partial-commit detection.
- Durable filesystem flush boundaries for JSONL file-kernel writes.
- JSONL file-kernel compaction into the canonical durable record format.
- Native file-backed compaction API.
- CLI file-backed compaction command.
- Native commit cell materialization API.
- Native commit slice materialization API for cursor-selected commit replay.
- Native commit export batch API for backup and sync flows.
- Native typed operation API for ingest, checkout, and audit.
- Native typed utility feedback revision API.
- Native read-only conflict analysis API.
- Native read-only batch conflict analysis API.
- A thin CLI over library APIs.

## Roadmap

The frontier roadmap is tracked in [`docs/roadmap.md`](docs/roadmap.md). The next major research item is an embedded database Steward model: a local, proposal-only model layer that helps maintain StateCells, conflicts, answerability, frontier priorities, and audit explanations without directly mutating committed truth.

## Engineering Standard

Development is test-first. No demo-only behavior, unchecked library panics, or shortcuts are accepted.
