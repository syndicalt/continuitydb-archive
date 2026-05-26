# ContinuityDB

> Project status: archived failed research prototype.
>
> The original ContinuityDB thesis did not survive falsification. A specialized
> StateCell datastore was not shown to be necessary for strong agent memory, and
> the strongest evidence now points to behavior, control, retrieval policy, and
> runtime-level memory as the real bottlenecks. This repository is retained for
> audit, salvageable benchmark patterns, and frontier-memory research notes; it
> should not be treated as an active production database roadmap.

See [ARCHIVED.md](ARCHIVED.md) and [Project Postmortem](docs/project-postmortem.md).

## What This Was

ContinuityDB was a Rust-native embeddable datastore experiment for continuity
state. Its intended product model was:

```text
StateCell -> checkout(task, budget) -> ContextPacket
```

The bet was that agent memory needed a specialized database primitive for
beliefs, evidence, uncertainty, revision history, lifecycle policy, and
operational outcomes. The retained evidence did not justify that bet.

## Archive Notes

- Do not treat the roadmap as current execution guidance.
- Do not build a production storage engine from this thesis.
- Do not cite historical internal benchmark scores as validation.
- Salvage benchmark cases, failure analysis, and falsification discipline into
  future memory-evaluation work if useful.

## Verification For Historical Changes

This repository may contain unmerged experimental changes. If inspecting or
salvaging code, use normal Rust verification commands before trusting a slice:

```bash
cargo fmt -- --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Documentation

- [Archive Status](ARCHIVED.md)
- [Project Postmortem](docs/project-postmortem.md)
- [Thesis](docs/thesis.md)
- [Context Packet Architecture](docs/context-packet-architecture.md)
- [Benchmark Regime](docs/benchmark-regime.md)
- [Roadmap](docs/roadmap.md)

Historical benchmark artifacts have been removed from the repository front door. New evidence should measure whether checkout improves real agent behavior over long horizons, not whether old internal rubrics favor ContinuityDB.
