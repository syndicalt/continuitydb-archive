//! ContinuityDB command-line interface.

use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use continuitydb_api::ContinuityDb;
use continuitydb_checkout::{checkout, CheckoutRequest};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
    SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
};
use continuitydb_kernel::{CommitManifestLookup, FileKernel, StorageKernel};
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
    },
    /// Export all file-backed commit slices to a versioned JSON backup envelope.
    ExportCommits {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to write the versioned JSON commit export envelope.
        output_path: PathBuf,
    },
    /// Import a versioned JSON commit backup envelope into a file-backed store.
    ImportCommits {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to read the versioned JSON commit export envelope from.
        input_path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Scope) => {
            println!("core,kernel,memory,revision,checkout,audit");
        }
        Some(Command::DemoCheckout) => {
            let slice = demo_checkout()?;
            println!("{}", serde_json::to_string_pretty(&slice)?);
        }
        Some(Command::CompactFile { path }) => {
            let mut db = ContinuityDb::new(FileKernel::open(&path)?);
            db.compact_file_store()?;
            let output = serde_json::json!({
                "path": path.display().to_string(),
                "compacted": true,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Some(Command::ExportCommits {
            store_path,
            output_path,
        }) => {
            let db = ContinuityDb::new(FileKernel::open(&store_path)?);
            let summary =
                db.export_commits_json_file(CommitManifestLookup::default(), &output_path)?;
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
        }) => {
            let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
            let imported_commits = db.import_commits_json_file(&input_path)?;
            let output = serde_json::json!({
                "path": store_path.display().to_string(),
                "input": input_path.display().to_string(),
                "imported_commits": imported_commits,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        None => {}
    }
    Ok(())
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
