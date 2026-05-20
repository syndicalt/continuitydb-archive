# ContinuityDB Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first production-quality Rust workspace foundation for ContinuityDB: core StateCell types, storage kernel boundary, in-memory backend, basic revision links, deterministic checkout, audit trace, and thin CLI.

**Architecture:** The engine is split into focused crates. `continuitydb-core` owns semantic domain types. `continuitydb-kernel` defines storage contracts. `continuitydb-memory` implements those contracts for correctness tests. `continuitydb-revision`, `continuitydb-checkout`, and `continuitydb-cli` depend inward and do not hide behavior behind command-line-only paths.

**Tech Stack:** Rust 2021 workspace, `thiserror` for typed errors, `serde` for stable data shapes, `chrono` for UTC timestamps, `uuid` for identifiers, `clap` for CLI, `assert_cmd` for CLI tests, standard `cargo test`, `cargo fmt`, and `cargo clippy`.

---

## Files And Responsibilities

- Create `Cargo.toml`: workspace members, shared dependency versions, strict lint profile.
- Create `.gitignore`: Rust build artifacts and local editor/runtime noise.
- Create `README.md`: project identity, architecture, and current development gates.
- Create `crates/continuitydb-core/Cargo.toml`: core crate manifest.
- Create `crates/continuitydb-core/src/lib.rs`: exported modules and crate documentation.
- Create `crates/continuitydb-core/src/cell.rs`: StateCell, identifiers, anchors, scope, activation, payload, costs.
- Create `crates/continuitydb-core/src/evidence.rs`: evidence records, confidence, citations, provenance source.
- Create `crates/continuitydb-core/src/time.rs`: valid-time and system-time types.
- Create `crates/continuitydb-core/src/error.rs`: core validation errors.
- Create `crates/continuitydb-kernel/Cargo.toml`: kernel crate manifest.
- Create `crates/continuitydb-kernel/src/lib.rs`: StorageKernel trait and lookup query/result types.
- Create `crates/continuitydb-memory/Cargo.toml`: memory backend manifest.
- Create `crates/continuitydb-memory/src/lib.rs`: in-memory append-only StorageKernel implementation.
- Create `crates/continuitydb-revision/Cargo.toml`: revision crate manifest.
- Create `crates/continuitydb-revision/src/lib.rs`: explicit revision links and revision service.
- Create `crates/continuitydb-checkout/Cargo.toml`: checkout crate manifest.
- Create `crates/continuitydb-checkout/src/lib.rs`: deterministic checkout constraints and scoring.
- Create `crates/continuitydb-cli/Cargo.toml`: CLI manifest.
- Create `crates/continuitydb-cli/src/main.rs`: thin CLI wrapper over library APIs.

## Task 1: Workspace Foundation

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `README.md`

- [ ] **Step 1: Write workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
]

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/cheapseatsecon/continuitydb"
rust-version = "1.78"

[workspace.dependencies]
assert_cmd = "2.0"
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }
clap = { version = "4.5", features = ["derive"] }
predicates = "3.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
uuid = { version = "1.8", features = ["serde", "v4"] }

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
dbg_macro = "deny"
todo = "deny"
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
```

- [ ] **Step 2: Write repository ignore file**

Create `.gitignore`:

```gitignore
/target/
**/*.rs.bk
.DS_Store
.idea/
.vscode/
```

- [ ] **Step 3: Write README**

Create `README.md`:

```markdown
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
```

- [ ] **Step 4: Run formatting and metadata check**

Run:

```bash
cargo metadata --format-version 1
cargo fmt --all -- --check
```

Expected: both commands fail because Cargo does not treat a virtual workspace with zero members as a valid metadata or formatting target. This is a guardrail: do not commit this intermediate state. Continue to Task 2 and commit the workspace foundation together with the first crate.

- [ ] **Step 5: Defer workspace foundation commit**

Do not commit yet. The first valid commit includes this foundation plus `continuitydb-core` from Task 2.

## Task 2: Core StateCell Domain Model

**Files:**
- Create: `crates/continuitydb-core/Cargo.toml`
- Create: `crates/continuitydb-core/src/lib.rs`
- Create: `crates/continuitydb-core/src/error.rs`
- Create: `crates/continuitydb-core/src/time.rs`
- Create: `crates/continuitydb-core/src/evidence.rs`
- Create: `crates/continuitydb-core/src/cell.rs`

- [ ] **Step 1: Write failing core tests**

Create `crates/continuitydb-core/src/lib.rs`:

```rust
//! Core semantic types for ContinuityDB.

mod cell;
mod error;
mod evidence;
mod time;

pub use cell::{
    ActivationState, Answerability, CellCost, CellPayload, Scope, SemanticAnchor, StateCell,
    StateCellId,
};
pub use error::CoreError;
pub use evidence::{Citation, Confidence, Evidence, SourceId, TrustSignal};
pub use time::{SystemTimeRange, ValidTimeRange};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn state_cell_requires_anchor_and_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let result = StateCell::new(
            StateCellId::new(),
            Vec::new(),
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what is ContinuityDB?".to_string()])?,
            Vec::new(),
            CellPayload::Text("ContinuityDB is a datastore.".to_string()),
            CellCost::new(6, 0)?,
        );

        assert!(matches!(result, Err(CoreError::MissingSemanticAnchor)));
        Ok(())
    }

    #[test]
    fn confidence_is_bounded() {
        assert!(Confidence::new(0.0).is_ok());
        assert!(Confidence::new(1.0).is_ok());
        assert!(matches!(
            Confidence::new(1.1),
            Err(CoreError::ConfidenceOutOfRange { value }) if value == 1.1
        ));
    }

    #[test]
    fn valid_time_contains_as_of_in_half_open_range() -> Result<(), Box<dyn std::error::Error>> {
        let start = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let end = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let range = ValidTimeRange::new(start, Some(end))?;

        assert!(range.contains(start));
        assert!(!range.contains(end));
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-core
```

Expected: FAIL because `crates/continuitydb-core/Cargo.toml` and referenced modules do not exist.

- [ ] **Step 3: Register and add core manifest**

Modify the workspace members in `Cargo.toml`:

```toml
members = [
    "crates/continuitydb-core",
]
```

Create `crates/continuitydb-core/Cargo.toml`:

```toml
[package]
name = "continuitydb-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
serde.workspace = true
thiserror.workspace = true
uuid.workspace = true

[lints]
workspace = true
```

- [ ] **Step 4: Add core errors**

Create `crates/continuitydb-core/src/error.rs`:

```rust
//! Error types for core ContinuityDB validation.

use thiserror::Error;

/// Errors produced when constructing invalid core values.
#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    /// A StateCell must have at least one semantic anchor.
    #[error("state cell requires at least one semantic anchor")]
    MissingSemanticAnchor,
    /// A StateCell must have at least one evidence record.
    #[error("state cell requires at least one evidence record")]
    MissingEvidence,
    /// Confidence values must be between zero and one, inclusive.
    #[error("confidence must be between 0.0 and 1.0, got {value}")]
    ConfidenceOutOfRange {
        /// The invalid confidence value.
        value: f32,
    },
    /// Time ranges must end after they start.
    #[error("time range end must be after start")]
    InvalidTimeRange,
    /// Answerability must include at least one non-empty question.
    #[error("answerability requires at least one non-empty question")]
    EmptyAnswerability,
    /// Token and compute costs must be non-negative.
    #[error("cell costs must be non-negative")]
    InvalidCost,
}
```

- [ ] **Step 5: Add bitemporal range types**

Create `crates/continuitydb-core/src/time.rs`:

```rust
//! Bitemporal range types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Real-world validity range for a StateCell version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidTimeRange {
    from: DateTime<Utc>,
    to: Option<DateTime<Utc>>,
}

impl ValidTimeRange {
    /// Creates a valid-time range. `to` is exclusive when present.
    pub fn new(from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> Result<Self, CoreError> {
        if let Some(to) = to {
            if to <= from {
                return Err(CoreError::InvalidTimeRange);
            }
        }

        Ok(Self { from, to })
    }

    /// Returns true when `as_of` falls inside the half-open valid-time range.
    pub fn contains(&self, as_of: DateTime<Utc>) -> bool {
        as_of >= self.from && self.to.map_or(true, |to| as_of < to)
    }
}

/// System transaction-time range for a StateCell version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemTimeRange {
    from: DateTime<Utc>,
    to: Option<DateTime<Utc>>,
}

impl SystemTimeRange {
    /// Creates a system-time range. `to` is exclusive when present.
    pub fn new(from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> Result<Self, CoreError> {
        if let Some(to) = to {
            if to <= from {
                return Err(CoreError::InvalidTimeRange);
            }
        }

        Ok(Self { from, to })
    }
}
```

- [ ] **Step 6: Add evidence types**

Create `crates/continuitydb-core/src/evidence.rs`:

```rust
//! Evidence and provenance types.

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Stable identifier for an evidence source.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SourceId(String);

impl SourceId {
    /// Creates a source identifier from a non-empty string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Human or machine-readable citation for evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    /// Source-local citation URI, path, or durable locator.
    pub locator: String,
}

/// Confidence score in the inclusive range 0.0..=1.0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Confidence(f32);

impl Confidence {
    /// Creates a bounded confidence value.
    pub fn new(value: f32) -> Result<Self, CoreError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(CoreError::ConfidenceOutOfRange { value });
        }

        Ok(Self(value))
    }

    /// Returns the inner scalar confidence value.
    pub fn value(self) -> f32 {
        self.0
    }
}

/// Signal about source trust or evidence quality.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrustSignal {
    /// Evidence was directly observed.
    DirectObservation,
    /// Evidence was inferred from other evidence.
    Derived,
    /// Evidence was supplied by a human operator.
    HumanSupplied,
}

/// Evidence supporting a StateCell version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Evidence source identity.
    pub source: SourceId,
    /// Citation pointing to the evidence.
    pub citation: Citation,
    /// Confidence assigned to this evidence.
    pub confidence: Confidence,
    /// Trust signals associated with this evidence.
    pub trust: Vec<TrustSignal>,
}
```

- [ ] **Step 7: Add StateCell types**

Create `crates/continuitydb-core/src/cell.rs`:

```rust
//! StateCell domain model.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Evidence, ValidTimeRange};

/// Immutable identifier for a StateCell version.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct StateCellId(Uuid);

impl StateCellId {
    /// Creates a random StateCell identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StateCellId {
    fn default() -> Self {
        Self::new()
    }
}

/// Semantic anchor used to address a StateCell by meaning.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SemanticAnchor(String);

impl SemanticAnchor {
    /// Creates a semantic anchor.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the inner anchor string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Visibility or applicability scope for a StateCell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Scope {
    /// Personal scope.
    Personal(String),
    /// Project scope.
    Project(String),
    /// Team scope.
    Team(String),
    /// Organization scope.
    Organization(String),
    /// Global scope.
    Global,
    /// Task-specific scope.
    Task(String),
}

/// Lifecycle activation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActivationState {
    /// Stored but not actively considered.
    Dormant,
    /// Available for normal checkout.
    Active,
    /// High-impact or uncertain enough to monitor.
    Frontier,
    /// Superseded or intentionally withdrawn from checkout.
    Retired,
}

/// Questions or intents a StateCell can help answer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Answerability {
    questions: Vec<String>,
}

impl Answerability {
    /// Creates answerability from non-empty questions.
    pub fn new(questions: Vec<String>) -> Result<Self, CoreError> {
        let questions: Vec<String> = questions
            .into_iter()
            .map(|question| question.trim().to_string())
            .filter(|question| !question.is_empty())
            .collect();

        if questions.is_empty() {
            return Err(CoreError::EmptyAnswerability);
        }

        Ok(Self { questions })
    }
}

/// Estimated materialization cost for a StateCell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellCost {
    /// Estimated token cost for LLM inclusion.
    pub token_count: i64,
    /// Estimated compute cost units for materialization.
    pub compute_units: i64,
}

impl CellCost {
    /// Creates non-negative cost estimates.
    pub fn new(token_count: i64, compute_units: i64) -> Result<Self, CoreError> {
        if token_count < 0 || compute_units < 0 {
            return Err(CoreError::InvalidCost);
        }

        Ok(Self {
            token_count,
            compute_units,
        })
    }
}

/// Hybrid content payload for a StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellPayload {
    /// Text payload.
    Text(String),
    /// Structured JSON payload.
    Json(serde_json::Value),
    /// External binary/blob reference.
    BlobRef(String),
}

/// Append-only, evidence-backed unit of operational truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateCell {
    /// Immutable cell version ID.
    pub id: StateCellId,
    /// Semantic anchors for lookup by meaning.
    pub anchors: Vec<SemanticAnchor>,
    /// Real-world validity interval.
    pub valid_time: ValidTimeRange,
    /// Scope for visibility and applicability.
    pub scope: Scope,
    /// Questions this cell can help answer.
    pub answerability: Answerability,
    /// Activation state for checkout/frontier selection.
    pub activation: ActivationState,
    /// Evidence supporting this cell version.
    pub evidence: Vec<Evidence>,
    /// Content payload.
    pub payload: CellPayload,
    /// Estimated cost.
    pub cost: CellCost,
}

impl StateCell {
    /// Creates a validated StateCell.
    pub fn new(
        id: StateCellId,
        anchors: Vec<SemanticAnchor>,
        valid_time: ValidTimeRange,
        scope: Scope,
        answerability: Answerability,
        evidence: Vec<Evidence>,
        payload: CellPayload,
        cost: CellCost,
    ) -> Result<Self, CoreError> {
        if anchors.is_empty() {
            return Err(CoreError::MissingSemanticAnchor);
        }

        if evidence.is_empty() {
            return Err(CoreError::MissingEvidence);
        }

        Ok(Self {
            id,
            anchors,
            valid_time,
            scope,
            answerability,
            activation: ActivationState::Active,
            evidence,
            payload,
            cost,
        })
    }
}
```

- [ ] **Step 8: Run core tests**

Run:

```bash
cargo test -p continuitydb-core
```

Expected: PASS for the three core tests.

- [ ] **Step 9: Commit core domain model**

Run:

```bash
git add Cargo.toml Cargo.lock .gitignore README.md crates/continuitydb-core
git commit -m "feat: add core StateCell model"
```

## Task 3: Storage Kernel And In-Memory Backend

**Files:**
- Create: `crates/continuitydb-kernel/Cargo.toml`
- Create: `crates/continuitydb-kernel/src/lib.rs`
- Create: `crates/continuitydb-memory/Cargo.toml`
- Create: `crates/continuitydb-memory/src/lib.rs`

- [ ] **Step 1: Write failing storage contract tests**

Create `crates/continuitydb-memory/src/lib.rs`:

```rust
//! In-memory StorageKernel implementation for correctness tests.

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope, SemanticAnchor,
        SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, StorageKernel};

    use super::MemoryKernel;

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what is stored?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: "test://sample".to_string(),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn append_and_lookup_by_anchor_preserves_cells() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = sample_cell("project:continuitydb:status", 0.9, 12)?;

        kernel.append_cell(cell.clone())?;
        let results = kernel
            .lookup_cells(CellLookup {
                semantic_anchor: Some("project:continuitydb:status".to_string()),
                ..CellLookup::default()
            })?;

        assert_eq!(results, vec![cell]);
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-memory
```

Expected: FAIL because `continuitydb-kernel`, `continuitydb-memory`, and `MemoryKernel` do not exist.

- [ ] **Step 3: Register and add kernel manifest and trait**

Modify the workspace members in `Cargo.toml`:

```toml
members = [
    "crates/continuitydb-core",
    "crates/continuitydb-kernel",
    "crates/continuitydb-memory",
]
```

Create `crates/continuitydb-kernel/Cargo.toml`:

```toml
[package]
name = "continuitydb-kernel"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
continuitydb-core = { path = "../continuitydb-core" }
thiserror.workspace = true

[lints]
workspace = true
```

Create `crates/continuitydb-kernel/src/lib.rs`:

```rust
//! Storage kernel interface for ContinuityDB backends.

use chrono::{DateTime, Utc};
use continuitydb_core::{Scope, StateCell};
use thiserror::Error;

/// Errors produced by storage kernels.
#[derive(Debug, Error, PartialEq)]
pub enum KernelError {
    /// A duplicate immutable StateCell version was appended.
    #[error("state cell already exists")]
    DuplicateCell,
}

/// Query constraints supported by baseline storage kernels.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CellLookup {
    /// Optional semantic anchor filter.
    pub semantic_anchor: Option<String>,
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time as-of filter.
    pub valid_at: Option<DateTime<Utc>>,
}

/// Minimal append and lookup contract required by the first ContinuityDB milestone.
pub trait StorageKernel {
    /// Appends an immutable StateCell version.
    fn append_cell(&mut self, cell: StateCell) -> Result<(), KernelError>;

    /// Looks up StateCells matching deterministic constraints.
    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError>;
}
```

- [ ] **Step 4: Add memory backend manifest and implementation**

Create `crates/continuitydb-memory/Cargo.toml`:

```toml
[package]
name = "continuitydb-memory"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
continuitydb-core = { path = "../continuitydb-core" }
continuitydb-kernel = { path = "../continuitydb-kernel" }

[dev-dependencies]
chrono.workspace = true

[lints]
workspace = true
```

Replace `crates/continuitydb-memory/src/lib.rs` with:

```rust
//! In-memory StorageKernel implementation for correctness tests.

use std::collections::HashSet;

use continuitydb_core::StateCell;
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};

/// Append-only in-memory storage kernel.
#[derive(Default)]
pub struct MemoryKernel {
    cells: Vec<StateCell>,
}

impl StorageKernel for MemoryKernel {
    fn append_cell(&mut self, cell: StateCell) -> Result<(), KernelError> {
        let existing_ids: HashSet<_> = self.cells.iter().map(|stored| stored.id).collect();
        if existing_ids.contains(&cell.id) {
            return Err(KernelError::DuplicateCell);
        }

        self.cells.push(cell);
        Ok(())
    }

    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
        let cells = self
            .cells
            .iter()
            .filter(|cell| {
                lookup.semantic_anchor.as_ref().map_or(true, |anchor| {
                    cell.anchors
                        .iter()
                        .any(|candidate| candidate.as_str() == anchor)
                })
            })
            .filter(|cell| lookup.scope.as_ref().map_or(true, |scope| &cell.scope == scope))
            .filter(|cell| {
                lookup
                    .valid_at
                    .map_or(true, |valid_at| cell.valid_time.contains(valid_at))
            })
            .cloned()
            .collect();

        Ok(cells)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope, SemanticAnchor,
        SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, StorageKernel};

    use super::MemoryKernel;

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what is stored?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: "test://sample".to_string(),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn append_and_lookup_by_anchor_preserves_cells() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = sample_cell("project:continuitydb:status", 0.9, 12)?;

        kernel.append_cell(cell.clone())?;
        let results = kernel
            .lookup_cells(CellLookup {
                semantic_anchor: Some("project:continuitydb:status".to_string()),
                ..CellLookup::default()
            })?;

        assert_eq!(results, vec![cell]);
        Ok(())
    }
}
```

- [ ] **Step 5: Run memory tests**

Run:

```bash
cargo test -p continuitydb-memory
```

Expected: PASS.

- [ ] **Step 6: Commit kernel and memory backend**

Run:

```bash
git add Cargo.toml crates/continuitydb-core/src/cell.rs crates/continuitydb-kernel crates/continuitydb-memory
git commit -m "feat: add storage kernel and memory backend"
```

## Task 4: Revision Links

**Files:**
- Create: `crates/continuitydb-revision/Cargo.toml`
- Create: `crates/continuitydb-revision/src/lib.rs`

- [ ] **Step 1: Write failing revision tests**

Create `crates/continuitydb-revision/src/lib.rs`:

```rust
//! Revision links and revision service.

#[cfg(test)]
mod tests {
    use continuitydb_core::StateCellId;

    use super::{RevisionGraph, RevisionLinkKind};

    #[test]
    fn revision_graph_records_supersession_and_conflict_links() {
        let previous = StateCellId::new();
        let current = StateCellId::new();
        let conflict = StateCellId::new();
        let mut graph = RevisionGraph::default();

        graph.link(current, RevisionLinkKind::Supersedes, previous);
        graph.link(current, RevisionLinkKind::ConflictsWith, conflict);

        assert_eq!(graph.targets(current, RevisionLinkKind::Supersedes), vec![previous]);
        assert_eq!(graph.targets(current, RevisionLinkKind::ConflictsWith), vec![conflict]);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-revision
```

Expected: FAIL because the crate manifest and revision types do not exist.

- [ ] **Step 3: Register and add revision crate**

Modify the workspace members in `Cargo.toml`:

```toml
members = [
    "crates/continuitydb-core",
    "crates/continuitydb-kernel",
    "crates/continuitydb-memory",
    "crates/continuitydb-revision",
]
```

Create `crates/continuitydb-revision/Cargo.toml`:

```toml
[package]
name = "continuitydb-revision"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
continuitydb-core = { path = "../continuitydb-core" }
serde.workspace = true

[lints]
workspace = true
```

Replace `crates/continuitydb-revision/src/lib.rs` with:

```rust
//! Revision links and revision service.

use std::collections::HashMap;

use continuitydb_core::StateCellId;
use serde::{Deserialize, Serialize};

/// Relationship between two StateCell versions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RevisionLinkKind {
    /// Current cell directly follows a previous version.
    Predecessor,
    /// Current cell supersedes the target.
    Supersedes,
    /// Current cell conflicts with the target.
    ConflictsWith,
    /// Current cell was derived from the target.
    DerivesFrom,
}

/// Append-only revision graph for StateCell version relationships.
#[derive(Default)]
pub struct RevisionGraph {
    links: HashMap<(StateCellId, RevisionLinkKind), Vec<StateCellId>>,
}

impl RevisionGraph {
    /// Records a directed revision link.
    pub fn link(&mut self, source: StateCellId, kind: RevisionLinkKind, target: StateCellId) {
        self.links.entry((source, kind)).or_default().push(target);
    }

    /// Returns all targets for a source and link kind.
    pub fn targets(&self, source: StateCellId, kind: RevisionLinkKind) -> Vec<StateCellId> {
        self.links.get(&(source, kind)).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use continuitydb_core::StateCellId;

    use super::{RevisionGraph, RevisionLinkKind};

    #[test]
    fn revision_graph_records_supersession_and_conflict_links() {
        let previous = StateCellId::new();
        let current = StateCellId::new();
        let conflict = StateCellId::new();
        let mut graph = RevisionGraph::default();

        graph.link(current, RevisionLinkKind::Supersedes, previous);
        graph.link(current, RevisionLinkKind::ConflictsWith, conflict);

        assert_eq!(graph.targets(current, RevisionLinkKind::Supersedes), vec![previous]);
        assert_eq!(graph.targets(current, RevisionLinkKind::ConflictsWith), vec![conflict]);
    }
}
```

- [ ] **Step 4: Run revision tests**

Run:

```bash
cargo test -p continuitydb-revision
```

Expected: PASS.

- [ ] **Step 5: Commit revision links**

Run:

```bash
git add Cargo.toml crates/continuitydb-revision
git commit -m "feat: add revision graph"
```

## Task 5: Deterministic Checkout And Audit

**Files:**
- Create: `crates/continuitydb-checkout/Cargo.toml`
- Create: `crates/continuitydb-checkout/src/lib.rs`

- [ ] **Step 1: Write failing checkout tests**

Create `crates/continuitydb-checkout/src/lib.rs`:

```rust
//! Deterministic checkout and audit.

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope, SemanticAnchor,
        SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::StorageKernel;
    use continuitydb_memory::MemoryKernel;

    use super::{checkout, audit, CheckoutRequest};

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what should the agent know?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: format!("test://{anchor}"),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn checkout_respects_token_budget_and_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let high = sample_cell("project:continuitydb:high", 0.95, 10)?;
        let low = sample_cell("project:continuitydb:low", 0.40, 10)?;
        kernel.append_cell(high.clone())?;
        kernel.append_cell(low)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![high]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn audit_includes_cell_id_and_citation_locator() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_cell("project:continuitydb:audit", 0.95, 10)?;
        let trace = audit(&cell);

        assert_eq!(trace.cell_id, cell.id);
        assert_eq!(trace.citations, vec!["test://project:continuitydb:audit".to_string()]);
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-checkout
```

Expected: FAIL because the checkout crate manifest and functions do not exist.

- [ ] **Step 3: Register and add checkout crate**

Modify the workspace members in `Cargo.toml`:

```toml
members = [
    "crates/continuitydb-core",
    "crates/continuitydb-kernel",
    "crates/continuitydb-memory",
    "crates/continuitydb-revision",
    "crates/continuitydb-checkout",
]
```

Create `crates/continuitydb-checkout/Cargo.toml`:

```toml
[package]
name = "continuitydb-checkout"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
continuitydb-core = { path = "../continuitydb-core" }
continuitydb-kernel = { path = "../continuitydb-kernel" }
serde.workspace = true
thiserror.workspace = true

[dev-dependencies]
continuitydb-memory = { path = "../continuitydb-memory" }

[lints]
workspace = true
```

Replace `crates/continuitydb-checkout/src/lib.rs` with:

```rust
//! Deterministic checkout and audit.

use chrono::{DateTime, Utc};
use continuitydb_core::{Confidence, Scope, StateCell, StateCellId};
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by checkout.
#[derive(Debug, Error, PartialEq)]
pub enum CheckoutError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

/// Request constraints for deterministic checkout.
#[derive(Clone, Debug)]
pub struct CheckoutRequest {
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

/// Materialized continuity slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutSlice {
    /// Selected cells.
    pub cells: Vec<StateCell>,
    /// Total estimated tokens.
    pub total_tokens: i64,
}

/// Audit trace for a StateCell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditTrace {
    /// Audited cell identifier.
    pub cell_id: StateCellId,
    /// Citation locators supporting the cell.
    pub citations: Vec<String>,
}

/// Materializes a deterministic continuity slice.
pub fn checkout<K: StorageKernel>(
    kernel: &K,
    request: CheckoutRequest,
) -> Result<CheckoutSlice, CheckoutError> {
    let mut candidates = kernel.lookup_cells(CellLookup {
        semantic_anchor: None,
        scope: request.scope,
        valid_at: request.valid_at,
    })?;

    candidates.retain(|cell| {
        cell.evidence
            .iter()
            .any(|evidence| evidence.confidence.value() >= request.minimum_confidence.value())
    });

    candidates.sort_by(|left, right| {
        let left_confidence = max_confidence(left);
        let right_confidence = max_confidence(right);
        right_confidence
            .partial_cmp(&left_confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut total_tokens = 0;
    let mut cells = Vec::new();
    for cell in candidates {
        let next_total = total_tokens + cell.cost.token_count;
        if next_total <= request.token_budget {
            total_tokens = next_total;
            cells.push(cell);
        }
    }

    Ok(CheckoutSlice {
        cells,
        total_tokens,
    })
}

/// Produces a basic audit trace for a StateCell.
pub fn audit(cell: &StateCell) -> AuditTrace {
    AuditTrace {
        cell_id: cell.id,
        citations: cell
            .evidence
            .iter()
            .map(|evidence| evidence.citation.locator.clone())
            .collect(),
    }
}

fn max_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}
```

- [ ] **Step 4: Run checkout tests**

Run:

```bash
cargo test -p continuitydb-checkout
```

Expected: PASS.

- [ ] **Step 5: Commit checkout and audit**

Run:

```bash
git add Cargo.toml crates/continuitydb-checkout
git commit -m "feat: add deterministic checkout and audit"
```

## Task 6: Thin CLI

**Files:**
- Create: `crates/continuitydb-cli/Cargo.toml`
- Create: `crates/continuitydb-cli/src/main.rs`

- [ ] **Step 1: Write failing CLI smoke test**

Create `crates/continuitydb-cli/src/main.rs`:

```rust
//! ContinuityDB command-line interface.

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::str::contains;

    #[test]
    fn cli_reports_version() -> Result<(), Box<dyn std::error::Error>> {
        let mut command = Command::cargo_bin("continuitydb")?;
        command.arg("--version");
        command.assert().success().stdout(contains("continuitydb"));
        Ok(())
    }
}

fn main() {}
```

- [ ] **Step 2: Run CLI test to verify failure**

Run:

```bash
cargo test -p continuitydb-cli
```

Expected: FAIL because the CLI crate manifest does not exist.

- [ ] **Step 3: Register and add CLI crate**

Modify the workspace members in `Cargo.toml`:

```toml
members = [
    "crates/continuitydb-core",
    "crates/continuitydb-kernel",
    "crates/continuitydb-memory",
    "crates/continuitydb-revision",
    "crates/continuitydb-checkout",
    "crates/continuitydb-cli",
]
```

Create `crates/continuitydb-cli/Cargo.toml`:

```toml
[package]
name = "continuitydb-cli"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[[bin]]
name = "continuitydb"
path = "src/main.rs"

[dependencies]
clap.workspace = true
continuitydb-checkout = { path = "../continuitydb-checkout" }
continuitydb-core = { path = "../continuitydb-core" }
continuitydb-memory = { path = "../continuitydb-memory" }

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true

[lints]
workspace = true
```

Replace `crates/continuitydb-cli/src/main.rs` with:

```rust
//! ContinuityDB command-line interface.

use clap::{Parser, Subcommand};

/// ContinuityDB command-line interface.
#[derive(Debug, Parser)]
#[command(name = "continuitydb", version, about = "Embeddable datastore for agent world models")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// Supported commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the current implementation scope.
    Scope,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Scope) => {
            println!("core,kernel,memory,revision,checkout,audit");
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::str::contains;

    #[test]
    fn cli_reports_version() -> Result<(), Box<dyn std::error::Error>> {
        let mut command = Command::cargo_bin("continuitydb")?;
        command.arg("--version");
        command.assert().success().stdout(contains("continuitydb"));
        Ok(())
    }
}
```

- [ ] **Step 4: Run CLI tests**

Run:

```bash
cargo test -p continuitydb-cli
```

Expected: PASS.

- [ ] **Step 5: Commit CLI**

Run:

```bash
git add Cargo.toml crates/continuitydb-cli
git commit -m "feat: add continuitydb CLI shell"
```

## Task 7: Whole-Workspace Verification

**Files:**
- Modify only if verification exposes concrete defects.

- [ ] **Step 1: Run formatter**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 2: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS with zero warnings.

- [ ] **Step 3: Run tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS for all crate tests.

- [ ] **Step 4: Inspect worktree**

Run:

```bash
git status --short --branch
```

Expected: only intentional untracked local runtime files, such as `.eventloom/`, remain outside committed project files.

- [ ] **Step 5: Commit verification fixes if needed**

If Steps 1-3 require fixes, commit the focused fixes:

```bash
git add Cargo.toml README.md .gitignore crates
git commit -m "chore: pass workspace verification"
```

If Steps 1-3 pass without changes, do not create an empty commit.

## Self-Review Against Spec

- StateCell model: covered by Task 2.
- Storage kernel boundary: covered by Task 3.
- In-memory backend: covered by Task 3.
- Append-only ingest path: covered by `StorageKernel::append_cell` in Task 3.
- Revision links: covered by Task 4.
- Basic deterministic checkout: covered by Task 5.
- Audit trace: covered by Task 5.
- CLI as thin wrapper: covered by Task 6.
- Test-first development: every behavior task starts with a failing test and a failure verification command.
- Production standard: workspace lints forbid unsafe code, `todo!`, `unwrap`, `expect`, and direct panics in production code.

## Execution Notes

The plan intentionally avoids LatticeDB integration, native query syntax, distributed storage, ML utility prediction, vector search, and C ABI. Those are separate milestones after the foundation proves the StateCell and storage-kernel boundaries.
