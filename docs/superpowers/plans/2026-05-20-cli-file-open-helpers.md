# CLI File Open Helpers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor file-backed CLI commands to use the native `ContinuityDb<FileKernel>` open helpers, including requirement-enforced construction for `inspect-kernel --require`.

**Architecture:** Add small CLI helper functions that open file-backed databases through `ContinuityDb::open_file` or `open_file_with_requirements`. Existing commands keep their JSON output and behavior while sharing the native API construction boundary.

**Tech Stack:** Rust, clap, assert_cmd, existing `continuitydb-cli`, `continuitydb-api`, and `continuitydb-kernel` crates.

---

## File Structure

- Modify `crates/continuitydb-cli/src/main.rs`: replace direct `FileKernel::open` usage with API open helpers and add CLI-local helper functions.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add a behavior-preserving test that exercises requirement-enforced open through CLI inspection output.
- Modify `README.md`: note that CLI file commands use native open helpers.
- Modify `docs/roadmap.md`: add CLI milestone.

## Task 1: Test the CLI Requirement Construction Path

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write the failing/guarding CLI test**

Add this test after `cli_inspect_kernel_accepts_durable_append_log_requirement`:

```rust
#[test]
fn cli_inspect_kernel_requirement_output_matches_native_gate(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-native-gate");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("durable-append-log")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let db = ContinuityDb::open_file_with_requirements(
        &path,
        continuitydb_kernel::KernelRequirements::durable_append_log(),
    )?;

    assert_eq!(json["required"].as_str(), Some("durable-append-log"));
    assert_eq!(json["satisfies"].as_bool(), Some(true));
    assert_eq!(db.kernel().path(), path.as_path());

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run test to verify current behavior**

Run:

```bash
cargo test -p continuitydb-cli native_gate --all-features
```

Expected before implementation: PASS, because this is a guard test proving CLI output is compatible with the native gate before refactoring. This slice is a refactor with a behavior-preserving regression test; the code change is still required because current CLI implementation does not use the native open helpers.

## Task 2: Refactor CLI Construction

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [ ] **Step 1: Remove direct FileKernel import**

Change:

```rust
use continuitydb_kernel::{
    CommitManifestLookup, FileKernel, KernelCapabilities, KernelDurability, KernelRequirements,
    StorageKernel,
};
```

to:

```rust
use continuitydb_kernel::{
    CommitManifestLookup, KernelCapabilities, KernelDurability, KernelRequirements, StorageKernel,
};
```

- [ ] **Step 2: Add open helpers below `capabilities_json`**

```rust
fn open_file_database(path: &PathBuf) -> Result<ContinuityDb<continuitydb_kernel::FileKernel>, Box<dyn std::error::Error>> {
    ContinuityDb::open_file(path).map_err(Into::into)
}

fn open_file_database_with_profile(
    path: &PathBuf,
    profile: RequirementProfile,
) -> Result<ContinuityDb<continuitydb_kernel::FileKernel>, Box<dyn std::error::Error>> {
    ContinuityDb::open_file_with_requirements(path, requirements_for_profile(profile))
        .map_err(Into::into)
}
```

- [ ] **Step 3: Refactor `inspect-kernel` branch**

Replace:

```rust
let db = ContinuityDb::new(FileKernel::open(&store_path)?);
let capabilities = db.kernel_capabilities();
let required = require.map(profile_name);
let satisfies = require
    .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
    .unwrap_or(true);
if let Some(profile) = require {
    db.ensure_kernel_requirements(requirements_for_profile(profile))?;
}
```

with:

```rust
let db = if let Some(profile) = require {
    open_file_database_with_profile(&store_path, profile)?
} else {
    open_file_database(&store_path)?
};
let capabilities = db.kernel_capabilities();
let required = require.map(profile_name);
let satisfies = require
    .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
    .unwrap_or(true);
```

- [ ] **Step 4: Refactor remaining file-backed branches**

Change compaction:

```rust
let mut db = ContinuityDb::new(FileKernel::open(&path)?);
```

to:

```rust
let mut db = open_file_database(&path)?;
```

Change export:

```rust
let db = ContinuityDb::new(FileKernel::open(&store_path)?);
```

to:

```rust
let db = open_file_database(&store_path)?;
```

Change import:

```rust
let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
```

to:

```rust
let mut db = open_file_database(&store_path)?;
```

- [ ] **Step 5: Run CLI tests**

Run:

```bash
cargo test -p continuitydb-cli --all-features
```

Expected: PASS.

- [ ] **Step 6: Commit implementation**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "refactor: use api file open helpers in cli"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near CLI kernel inspection:

```markdown
- CLI file-backed commands routed through native open helpers.
```

Add this CLI roadmap milestone after kernel capability inspection:

```markdown
5. Route file-backed CLI commands through native open helpers. Refactored inspection, compaction, export, and import commands to use `ContinuityDb<FileKernel>::open_file` or `open_file_with_requirements`, keeping operational tooling aligned with the embeddable API boundary.
```

- [ ] **Step 2: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit docs**

```bash
git add README.md docs/roadmap.md
git commit -m "docs: record cli open helper routing"
```
