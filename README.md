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
- Audit traces.
- A thin CLI over library APIs.

## Engineering Standard

Development is test-first. No demo-only behavior, unchecked library panics, or shortcuts are accepted.
