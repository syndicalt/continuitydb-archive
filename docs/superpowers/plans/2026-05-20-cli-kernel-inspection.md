# CLI Kernel Inspection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `continuitydb inspect-kernel` so operators and CI can inspect file-kernel capabilities and enforce named storage requirement profiles from the CLI.

**Architecture:** The CLI parses an optional requirement profile, opens the path as a `FileKernel`, wraps it in `ContinuityDb`, and prints deterministic JSON containing capabilities, requested profile, and satisfaction status. Requirement enforcement reuses `ContinuityDb::ensure_kernel_requirements` so CLI behavior matches native API semantics.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli`, `continuitydb-api`, and `continuitydb-kernel` crates, assert_cmd CLI tests.

---

## File Structure

- Modify `crates/continuitydb-cli/src/main.rs`: add command, profile enum, JSON helpers, requirement enforcement.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI behavior tests.
- Modify `README.md`: add CLI kernel inspection to current scope.
- Modify `docs/roadmap.md`: add CLI milestone.

## Task 1: CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add these tests after `cli_compact_file_fails_for_corrupt_store`:

```rust
#[test]
fn cli_inspect_kernel_reports_file_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["path"].as_str(), path.to_str());
    assert!(json["required"].is_null());
    assert_eq!(json["satisfies"].as_bool(), Some(true));
    assert_eq!(json["capabilities"]["durability"].as_str(), Some("append-log"));
    assert_eq!(json["capabilities"]["append_only"].as_bool(), Some(true));
    assert_eq!(json["capabilities"]["derived_indexes"].as_bool(), Some(true));
    assert_eq!(json["capabilities"]["persistent_indexes"].as_bool(), Some(false));
    assert_eq!(
        json["capabilities"]["explicit_commit_records"].as_bool(),
        Some(true)
    );
    assert_eq!(json["capabilities"]["durable_flush"].as_bool(), Some(true));
    assert_eq!(json["capabilities"]["compaction"].as_bool(), Some(true));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_accepts_durable_append_log_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-durable");

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

    assert_eq!(json["required"].as_str(), Some("durable-append-log"));
    assert_eq!(json["satisfies"].as_bool(), Some(true));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_rejects_indexed_embedded_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-indexed");

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require")
        .arg("indexed-embedded")
        .assert()
        .failure()
        .stderr(contains("storage kernel requirements are not met"));

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli inspect_kernel --all-features
```

Expected: FAIL because `inspect-kernel` is not a recognized command.

- [ ] **Step 3: Commit tests after implementation, not before**

No commit in this step. Keep the failing tests staged only after implementation passes.

## Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [ ] **Step 1: Add imports**

Update the kernel import:

```rust
use continuitydb_kernel::{
    CommitManifestLookup, FileKernel, KernelCapabilities, KernelDurability, KernelRequirements,
    StorageKernel,
};
```

- [ ] **Step 2: Add profile enum and command variant**

Add after `struct Cli`:

```rust
/// Named kernel requirement profiles understood by the CLI.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum RequirementProfile {
    /// Allow temporary in-process correctness kernels.
    Ephemeral,
    /// Require durable append-log storage.
    DurableAppendLog,
    /// Require future durable storage with persistent indexes.
    IndexedEmbedded,
}
```

Add to `Command`:

```rust
/// Inspect file-backed kernel capabilities and optionally enforce a requirement profile.
InspectKernel {
    /// Path to the JSONL file-backed store.
    store_path: PathBuf,
    /// Required storage profile.
    #[arg(long = "require")]
    require: Option<RequirementProfile>,
},
```

- [ ] **Step 3: Add profile and JSON helpers**

Add below `main`:

```rust
fn requirements_for_profile(profile: RequirementProfile) -> KernelRequirements {
    match profile {
        RequirementProfile::Ephemeral => KernelRequirements::ephemeral(),
        RequirementProfile::DurableAppendLog => KernelRequirements::durable_append_log(),
        RequirementProfile::IndexedEmbedded => KernelRequirements::indexed_embedded(),
    }
}

fn profile_name(profile: RequirementProfile) -> &'static str {
    match profile {
        RequirementProfile::Ephemeral => "ephemeral",
        RequirementProfile::DurableAppendLog => "durable-append-log",
        RequirementProfile::IndexedEmbedded => "indexed-embedded",
    }
}

fn durability_name(durability: KernelDurability) -> &'static str {
    match durability {
        KernelDurability::Ephemeral => "ephemeral",
        KernelDurability::AppendLog => "append-log",
        KernelDurability::IndexedEmbedded => "indexed-embedded",
    }
}

fn capabilities_json(capabilities: KernelCapabilities) -> serde_json::Value {
    serde_json::json!({
        "durability": durability_name(capabilities.durability),
        "append_only": capabilities.append_only,
        "derived_indexes": capabilities.derived_indexes,
        "persistent_indexes": capabilities.persistent_indexes,
        "explicit_commit_records": capabilities.explicit_commit_records,
        "durable_flush": capabilities.durable_flush,
        "compaction": capabilities.compaction,
    })
}
```

- [ ] **Step 4: Implement command branch**

Add this `match` branch before `CompactFile`:

```rust
Some(Command::InspectKernel {
    store_path,
    require,
}) => {
    let db = ContinuityDb::new(FileKernel::open(&store_path)?);
    let capabilities = db.kernel_capabilities();
    let required = require.map(profile_name);
    let satisfies = require
        .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
        .unwrap_or(true);
    if let Some(profile) = require {
        db.ensure_kernel_requirements(requirements_for_profile(profile))?;
    }
    let output = serde_json::json!({
        "path": store_path.display().to_string(),
        "capabilities": capabilities_json(capabilities),
        "required": required,
        "satisfies": satisfies,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-cli inspect_kernel --all-features
```

Expected: PASS.

- [ ] **Step 6: Commit implementation**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add cli kernel inspection"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near CLI commands:

```markdown
- CLI kernel capability inspection and requirement checks.
```

Add this CLI roadmap milestone after commit backup/restore:

```markdown
4. Expose kernel capability inspection from the CLI. Implemented `continuitydb inspect-kernel <store-path> [--require <profile>]` so operators and CI can inspect file-backed storage guarantees and fail early when a requested profile is not satisfied.
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
git commit -m "docs: record cli kernel inspection"
```
