# Default Steward Evaluation Conflict Case Design

## Goal

Expand the default local Steward model evaluation suite with a deterministic conflict-classification case.

## Motivation

The roadmap acceptance-test bar calls out correct conflict versus supersession classification. The default benchmark suite currently exercises uncertainty and verification behavior, but it does not require any revision-link classification. Before collecting real local model artifacts, the fixed suite should cover a second core Steward behavior: preserving evidence and emitting a `ConflictsWith` revision proposal for contradictory claims.

## Design

Add a second case to `default_steward_evaluation_suite()`:

- name: `conflict classification`
- task: classify contradictory release-status claims
- evidence locator: `continuitydb://evaluation/conflict-evidence`
- expected action: `StewardAction::LinkRevision { source: StateCellId::from_u128(1), kind: RevisionLinkKind::ConflictsWith, target: StateCellId::from_u128(2) }`
- required citation: `continuitydb://evaluation/conflict-evidence`
- forbidden rationale marker: `verified in production`

The existing uncertainty case remains unchanged.

## CLI Fixture

The `benchmark-local-model` CLI test fixture should emit both required proposals so the default benchmark still passes and exposes two case reports.

## Non-Goals

- Do not add real model dependencies.
- Do not change policy validation semantics.
- Do not add weighted scoring.
- Do not change the local model response schema.
