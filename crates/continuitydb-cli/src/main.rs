//! ContinuityDB command-line interface.

use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use continuitydb_api::{ContinuityDb, ContinuityError};
use continuitydb_checkout::{checkout, CheckoutRequest};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellPayload, Citation, CommitId, Confidence,
    Evidence, Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
};
use continuitydb_kernel::{
    CommitManifestLookup, KernelCapabilities, KernelDurability, KernelRequirements, StorageKernel,
};
use continuitydb_memory::MemoryKernel;
use std::path::PathBuf;

/// ContinuityDB command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "continuitydb",
    version,
    about = "Embeddable datastore for agent world models"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

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

/// Supported commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the current implementation scope.
    Scope,
    /// Print deterministic demo checkout JSON.
    DemoCheckout,
    /// Compact a JSONL file-backed store into the canonical durable record format.
    CompactFile {
        /// Path to the JSONL file-backed store.
        path: PathBuf,
        /// Skip rewriting when the store is already canonical.
        #[arg(long = "if-needed")]
        if_needed: bool,
    },
    /// Inspect file-backed kernel capabilities and optionally enforce a requirement profile.
    InspectKernel {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Required storage profile.
        #[arg(long = "require")]
        require: Option<RequirementProfile>,
        /// Require the store to already be in canonical durable file format.
        #[arg(long = "require-canonical")]
        require_canonical: bool,
    },
    /// Export all file-backed commit slices to a versioned JSON backup envelope.
    ExportCommits {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to write the versioned JSON commit export envelope.
        output_path: PathBuf,
        /// Exclusive commit cursor to start after.
        #[arg(long = "after")]
        after: Option<CommitId>,
        /// Maximum number of commits to export.
        #[arg(long = "limit")]
        limit: Option<usize>,
    },
    /// Import a versioned JSON commit backup envelope into a file-backed store.
    ImportCommits {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to read the versioned JSON commit export envelope from.
        input_path: PathBuf,
        /// Validate the import without mutating the target store.
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Scope) => {
            println!("core,kernel,memory,revision,checkout,audit");
        }
        Some(Command::DemoCheckout) => {
            let slice = demo_checkout()?;
            println!("{}", serde_json::to_string_pretty(&slice)?);
        }
        Some(Command::InspectKernel {
            store_path,
            require,
            require_canonical,
        }) => {
            let db = if let Some(profile) = require {
                open_file_database_with_profile(&store_path, profile)?
            } else {
                open_file_database(&store_path)?
            };
            if require_canonical {
                db.ensure_file_store_canonical()?;
            }
            let capabilities = db.kernel_capabilities();
            let status = file_status_json(&db)?;
            let health = file_health_json(&db);
            let required = require.map(profile_name);
            let satisfies = require
                .map(|profile| db.kernel_satisfies(requirements_for_profile(profile)))
                .unwrap_or(true);
            let output = serde_json::json!({
                "path": store_path.display().to_string(),
                "capabilities": capabilities_json(capabilities),
                "status": status,
                "health": health,
                "required": required,
                "satisfies": satisfies,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::CompactFile { path, if_needed }) => {
            let mut db = open_file_database(&path)?;
            let output = if if_needed {
                let summary = db.compact_file_store_if_needed()?;
                serde_json::json!({
                    "path": path.display().to_string(),
                    "compacted": summary.compacted,
                    "before": file_health_value(summary.before),
                    "after": file_health_value(summary.after),
                })
            } else {
                db.compact_file_store()?;
                serde_json::json!({
                    "path": path.display().to_string(),
                    "compacted": true,
                })
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ExportCommits {
            store_path,
            output_path,
            after,
            limit,
        }) => {
            let db = open_file_database(&store_path)?;
            let summary =
                db.export_commits_json_file(CommitManifestLookup { after, limit }, &output_path)?;
            let output = serde_json::json!({
                "path": store_path.display().to_string(),
                "output": output_path.display().to_string(),
                "exported_commits": summary.exported_commits,
                "next_after": summary.next_after,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ImportCommits {
            store_path,
            input_path,
            dry_run,
        }) => {
            let mut db = open_file_database(&store_path)?;
            let output = if dry_run {
                let validation = db.validate_commits_json_file(&input_path)?;
                serde_json::json!({
                    "path": store_path.display().to_string(),
                    "input": input_path.display().to_string(),
                    "dry_run": true,
                    "valid_commits": validation.valid_commits,
                })
            } else {
                let summary = db.import_commits_json_file_with_summary(&input_path)?;
                serde_json::json!({
                    "path": store_path.display().to_string(),
                    "input": input_path.display().to_string(),
                    "imported_commits": summary.imported_commits,
                    "next_after": summary.next_after,
                })
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        None => {}
    }
    Ok(())
}

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

fn file_status_json(
    db: &ContinuityDb<continuitydb_kernel::FileKernel>,
) -> Result<serde_json::Value, ContinuityError> {
    let status = db.file_store_status()?;
    Ok(serde_json::json!({
        "cell_count": status.cell_count,
        "commit_count": status.commit_count,
        "file_size_bytes": status.file_size_bytes,
    }))
}

fn file_health_json(db: &ContinuityDb<continuitydb_kernel::FileKernel>) -> serde_json::Value {
    file_health_value(db.file_store_health())
}

fn file_health_value(health: continuitydb_kernel::FileKernelHealth) -> serde_json::Value {
    serde_json::json!({
        "has_header": health.has_header,
        "legacy_raw_cells": health.legacy_raw_cells,
        "checksum_free_records": health.checksum_free_records,
        "canonical_records": health.canonical_records,
        "compaction_recommended": health.compaction_recommended,
    })
}

fn open_file_database(
    path: &PathBuf,
) -> Result<ContinuityDb<continuitydb_kernel::FileKernel>, Box<dyn std::error::Error>> {
    ContinuityDb::open_file(path).map_err(Into::into)
}

fn open_file_database_with_profile(
    path: &PathBuf,
    profile: RequirementProfile,
) -> Result<ContinuityDb<continuitydb_kernel::FileKernel>, Box<dyn std::error::Error>> {
    ContinuityDb::open_file_with_requirements(path, requirements_for_profile(profile))
        .map_err(Into::into)
}

fn demo_checkout() -> Result<continuitydb_checkout::CheckoutSlice, Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let mut frontier = demo_cell("project:continuitydb:frontier", "demo://frontier", 0.95, 10)?;
    frontier.activation = ActivationState::Frontier;
    let alternative = demo_cell(
        "project:continuitydb:alternative",
        "demo://alternative",
        0.90,
        10,
    )?;

    kernel.append_cell(frontier)?;
    kernel.append_cell(alternative)?;

    checkout(
        &kernel,
        CheckoutRequest {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 10,
        },
    )
    .map_err(Into::into)
}

fn demo_cell(
    anchor: &str,
    citation: &str,
    confidence: f32,
    tokens: i64,
) -> Result<StateCell, Box<dyn std::error::Error>> {
    let valid_from = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid demo timestamp"))?;
    StateCell::new(
        StateCellId::new(),
        vec![SemanticAnchor::new(anchor)],
        ValidTimeRange::new(valid_from, None)?,
        Scope::Project("continuitydb".to_string()),
        Answerability::new(vec!["what should the agent know?".to_string()])?,
        vec![Evidence {
            source: SourceId::new("demo"),
            citation: Citation {
                locator: citation.to_string(),
            },
            confidence: Confidence::new(confidence)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text(anchor.to_string()),
        CellCost::new(tokens, 0)?,
    )
    .map_err(Into::into)
}
